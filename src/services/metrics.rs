use std::path::Path;

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS request_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    latency_ms INTEGER NOT NULL,
    success INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_request_log_timestamp ON request_log(timestamp);
"#;

pub struct MetricsStore {
    conn: Connection,
}

impl MetricsStore {
    pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "auto_vacuum", "INCREMENTAL")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    pub fn record(&self, latency_ms: u64, success: bool) {
        let _ = self.conn.execute(
            "INSERT INTO request_log (timestamp, latency_ms, success)
             VALUES (datetime('now'), ?1, ?2)",
            params![latency_ms as i64, success as i32],
        );
    }

    pub fn get_summary(&self, retention_days: u32) -> MetricsSummary {
        let query = format!(
            r#"
            SELECT
                COUNT(*) as total_requests,
                COALESCE(AVG(latency_ms), 0) as avg_latency_ms,
                COALESCE(SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END) * 100.0 / NULLIF(COUNT(*), 0), 0) as error_rate
            FROM request_log
            WHERE timestamp >= datetime('now', '-{} days')
            "#,
            retention_days
        );

        self.conn
            .query_row(&query, [], |row| {
                Ok(MetricsSummary {
                    total_requests: row.get::<_, i64>(0)? as u64,
                    avg_latency_ms: row.get::<_, f64>(1)? as u64,
                    error_rate: row.get::<_, f64>(2)? as f32,
                })
            })
            .unwrap_or_default()
    }

    pub fn cleanup(&self, retention_days: u32) {
        let query = format!(
            "DELETE FROM request_log WHERE timestamp < datetime('now', '-{} days')",
            retention_days
        );
        let _ = self.conn.execute(&query, []);
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricsSummary {
    pub total_requests: u64,
    pub avg_latency_ms: u64,
    pub error_rate: f32,
}
