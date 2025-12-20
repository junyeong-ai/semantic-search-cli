use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::error::DaemonError;
use crate::models::Config;
use crate::server::protocol::{
    EmbedRequest, Request, Response, StatusResponse, decode_length, encode_message,
};

pub struct DaemonClient {
    socket_path: PathBuf,
    auto_start: bool,
}

impl DaemonClient {
    pub fn new(config: &Config) -> Self {
        Self {
            socket_path: config.socket_path(),
            auto_start: config.daemon.auto_start,
        }
    }

    pub fn is_running(&self) -> bool {
        self.socket_path.exists()
            && std::os::unix::net::UnixStream::connect(&self.socket_path).is_ok()
    }

    pub async fn ensure_running(&self) -> Result<(), DaemonError> {
        if self.is_running() {
            return Ok(());
        }

        if !self.auto_start {
            return Err(DaemonError::NotRunning);
        }

        self.spawn_daemon()?;
        self.wait_for_ready().await
    }

    fn spawn_daemon(&self) -> Result<(), DaemonError> {
        let exe = std::env::current_exe().map_err(|e| DaemonError::SpawnError(e.to_string()))?;

        Command::new(&exe)
            .args(["serve", "--daemon"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .map_err(|e| DaemonError::SpawnError(e.to_string()))?;

        Ok(())
    }

    async fn wait_for_ready(&self) -> Result<(), DaemonError> {
        let max_wait = Duration::from_secs(60);
        let check_interval = Duration::from_millis(100);
        let start = std::time::Instant::now();

        while start.elapsed() < max_wait {
            if self.is_running() && self.ping().await.is_ok() {
                return Ok(());
            }
            tokio::time::sleep(check_interval).await;
        }

        Err(DaemonError::Timeout)
    }

    async fn connect(&self) -> Result<UnixStream, DaemonError> {
        UnixStream::connect(&self.socket_path)
            .await
            .map_err(|e| DaemonError::ConnectionFailed(e.to_string()))
    }

    async fn send_request(&self, request: Request) -> Result<Response, DaemonError> {
        let mut stream = self.connect().await?;

        let encoded =
            encode_message(&request).map_err(|e| DaemonError::ProtocolError(e.to_string()))?;

        stream
            .write_all(&encoded)
            .await
            .map_err(|e| DaemonError::SocketError(e.to_string()))?;

        let mut len_buf = [0u8; 4];
        stream
            .read_exact(&mut len_buf)
            .await
            .map_err(|e| DaemonError::SocketError(e.to_string()))?;

        let len = decode_length(&len_buf);
        let mut msg_buf = vec![0u8; len];

        stream
            .read_exact(&mut msg_buf)
            .await
            .map_err(|e| DaemonError::SocketError(e.to_string()))?;

        serde_json::from_slice(&msg_buf).map_err(|e| DaemonError::ProtocolError(e.to_string()))
    }

    pub async fn ping(&self) -> Result<(), DaemonError> {
        match self.send_request(Request::Ping).await? {
            Response::Pong => Ok(()),
            Response::Error(e) => Err(DaemonError::ProtocolError(e.message)),
            _ => Err(DaemonError::ProtocolError(
                "unexpected response".to_string(),
            )),
        }
    }

    pub async fn status(&self) -> Result<StatusResponse, DaemonError> {
        match self.send_request(Request::Status).await? {
            Response::Status(s) => Ok(s),
            Response::Error(e) => Err(DaemonError::ProtocolError(e.message)),
            _ => Err(DaemonError::ProtocolError(
                "unexpected response".to_string(),
            )),
        }
    }

    pub async fn shutdown(&self) -> Result<(), DaemonError> {
        match self.send_request(Request::Shutdown).await? {
            Response::ShutdownAck => Ok(()),
            Response::Error(e) => Err(DaemonError::ProtocolError(e.message)),
            _ => Err(DaemonError::ProtocolError(
                "unexpected response".to_string(),
            )),
        }
    }

    pub async fn embed(
        &self,
        texts: Vec<String>,
        is_query: bool,
    ) -> Result<Vec<Vec<f32>>, DaemonError> {
        self.ensure_running().await?;

        let request = Request::Embed(EmbedRequest { texts, is_query });

        match self.send_request(request).await? {
            Response::Embed(r) => Ok(r.embeddings),
            Response::Error(e) => Err(DaemonError::ProtocolError(e.message)),
            _ => Err(DaemonError::ProtocolError(
                "unexpected response".to_string(),
            )),
        }
    }
}

pub fn stop_daemon(config: &Config) -> Result<(), DaemonError> {
    let pid_path = config.pid_path();
    if !pid_path.exists() {
        return Err(DaemonError::NotRunning);
    }

    let pid_str = std::fs::read_to_string(&pid_path)?;
    let pid: i32 = pid_str
        .trim()
        .parse()
        .map_err(|_| DaemonError::ProtocolError("invalid pid file".to_string()))?;

    #[cfg(unix)]
    {
        use nix::sys::signal::{Signal, kill};
        use nix::unistd::Pid;

        kill(Pid::from_raw(pid), Signal::SIGTERM)
            .map_err(|e| DaemonError::SocketError(e.to_string()))?;
    }

    let _ = std::fs::remove_file(&pid_path);
    let _ = std::fs::remove_file(config.socket_path());

    Ok(())
}
