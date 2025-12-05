//! Retry utilities with exponential backoff.

use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;

/// Configuration for retry behavior.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Initial delay between retries.
    pub initial_delay: Duration,
    /// Maximum delay between retries.
    pub max_delay: Duration,
    /// Backoff multiplier (delay *= multiplier after each retry).
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    /// Create a new retry configuration.
    #[must_use]
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            ..Default::default()
        }
    }

    /// Set the initial delay.
    #[must_use]
    pub fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    /// Set the maximum delay.
    #[must_use]
    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Set the backoff multiplier.
    #[must_use]
    pub fn with_multiplier(mut self, multiplier: f64) -> Self {
        self.multiplier = multiplier;
        self
    }
}

/// Retry result indicating what happened.
#[derive(Debug)]
pub enum RetryResult<T, E> {
    /// Operation succeeded.
    Success(T),
    /// Operation failed after all retries.
    Failed { last_error: E, attempts: u32 },
}

impl<T, E> RetryResult<T, E> {
    /// Convert to a Result, discarding retry information.
    pub fn into_result(self) -> Result<T, E> {
        match self {
            RetryResult::Success(value) => Ok(value),
            RetryResult::Failed { last_error, .. } => Err(last_error),
        }
    }
}

/// Determines if an error is retryable.
pub trait Retryable {
    /// Returns true if the operation should be retried.
    fn is_retryable(&self) -> bool;
}

// Default implementation for anyhow::Error
impl Retryable for anyhow::Error {
    fn is_retryable(&self) -> bool {
        // By default, retry on common transient errors
        let msg = self.to_string().to_lowercase();
        msg.contains("timeout")
            || msg.contains("connection refused")
            || msg.contains("connection reset")
            || msg.contains("temporarily unavailable")
            || msg.contains("service unavailable")
            || msg.contains("too many requests")
    }
}

/// Execute an async operation with exponential backoff retry.
pub async fn with_retry<T, E, F, Fut>(config: &RetryConfig, mut operation: F) -> RetryResult<T, E>
where
    E: Retryable + std::fmt::Debug,
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let mut attempts = 0;
    let mut delay = config.initial_delay;

    loop {
        attempts += 1;

        match operation().await {
            Ok(value) => return RetryResult::Success(value),
            Err(error) => {
                if attempts >= config.max_retries || !error.is_retryable() {
                    return RetryResult::Failed {
                        last_error: error,
                        attempts,
                    };
                }

                // Add some jitter to avoid thundering herd
                let jitter_ms = rand_jitter(delay.as_millis() as u64 / 4);
                let actual_delay = delay + Duration::from_millis(jitter_ms);

                sleep(actual_delay).await;

                // Increase delay for next attempt
                delay = Duration::from_secs_f64(delay.as_secs_f64() * config.multiplier)
                    .min(config.max_delay);
            }
        }
    }
}

/// Generate a random jitter value.
fn rand_jitter(max: u64) -> u64 {
    if max == 0 {
        return 0;
    }
    // Simple linear congruential generator for jitter
    // This is not cryptographically secure, but fine for jitter
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    seed % max
}

/// Execute an async operation with default retry configuration.
pub async fn retry<T, E, F, Fut>(operation: F) -> Result<T, E>
where
    E: Retryable + std::fmt::Debug,
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    with_retry(&RetryConfig::default(), operation)
        .await
        .into_result()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[derive(Debug)]
    struct RetryableError(String);

    impl Retryable for RetryableError {
        fn is_retryable(&self) -> bool {
            self.0.contains("transient")
        }
    }

    #[tokio::test]
    async fn test_retry_success_first_attempt() {
        let counter = AtomicU32::new(0);
        let result = with_retry(&RetryConfig::new(3), || async {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok::<_, RetryableError>("success")
        })
        .await;

        match result {
            RetryResult::Success(v) => assert_eq!(v, "success"),
            _ => panic!("expected success"),
        }
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_success_after_retries() {
        let counter = AtomicU32::new(0);
        let result = with_retry(
            &RetryConfig::new(3).with_initial_delay(Duration::from_millis(10)),
            || async {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(RetryableError("transient error".to_string()))
                } else {
                    Ok("success")
                }
            },
        )
        .await;

        match result {
            RetryResult::Success(v) => assert_eq!(v, "success"),
            _ => panic!("expected success"),
        }
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_non_retryable_error() {
        let counter = AtomicU32::new(0);
        let result = with_retry(&RetryConfig::new(3), || async {
            counter.fetch_add(1, Ordering::SeqCst);
            Err::<(), _>(RetryableError("permanent error".to_string()))
        })
        .await;

        match result {
            RetryResult::Failed { attempts, .. } => assert_eq!(attempts, 1),
            _ => panic!("expected failure"),
        }
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_exhausted() {
        let counter = AtomicU32::new(0);
        let result = with_retry(
            &RetryConfig::new(3).with_initial_delay(Duration::from_millis(10)),
            || async {
                counter.fetch_add(1, Ordering::SeqCst);
                Err::<(), _>(RetryableError("transient error".to_string()))
            },
        )
        .await;

        match result {
            RetryResult::Failed { attempts, .. } => assert_eq!(attempts, 3),
            _ => panic!("expected failure"),
        }
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }
}
