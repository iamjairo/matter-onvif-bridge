//! MediaMTX lifecycle management — external mode (Docker) or local subprocess.

use std::time::Duration;

use tokio::process::Command;
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::mediamtx_api::MediaMtxApi;

/// MediaMTX process manager.
pub struct MediaMtxManager {
    api: MediaMtxApi,
    mode: MediaMtxMode,
    binary_path: String,
    api_port: u16,
    whep_port: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MediaMtxMode {
    /// Connect to an already-running MediaMTX instance (e.g., Docker Compose).
    External,
    /// Spawn MediaMTX as a local subprocess.
    Local,
}

impl MediaMtxManager {
    pub fn new(
        host: &str,
        api_port: u16,
        whep_port: u16,
        mode: MediaMtxMode,
        binary_path: &str,
    ) -> Self {
        Self {
            api: MediaMtxApi::new(host, api_port, whep_port),
            mode,
            binary_path: binary_path.to_string(),
            api_port,
            whep_port,
        }
    }

    /// Get a reference to the API client.
    pub fn api(&self) -> &MediaMtxApi {
        &self.api
    }

    /// Start or connect to MediaMTX.
    pub async fn start(&self) -> Result<(), String> {
        match self.mode {
            MediaMtxMode::External => {
                info!("Connecting to external MediaMTX...");
                self.wait_for_ready(Duration::from_secs(15)).await
            }
            MediaMtxMode::Local => {
                info!(binary = self.binary_path, "Starting local MediaMTX...");
                self.spawn_local().await?;
                self.wait_for_ready(Duration::from_secs(15)).await
            }
        }
    }

    /// Poll the MediaMTX API until it responds or timeout.
    async fn wait_for_ready(&self, timeout: Duration) -> Result<(), String> {
        let start = std::time::Instant::now();

        loop {
            if self.api.is_ready().await {
                info!("MediaMTX is ready");
                return Ok(());
            }

            if start.elapsed() > timeout {
                return Err(format!("MediaMTX not ready after {}s", timeout.as_secs()));
            }

            sleep(Duration::from_millis(500)).await;
        }
    }

    /// Spawn MediaMTX as a local subprocess with a minimal configuration.
    async fn spawn_local(&self) -> Result<(), String> {
        // Write a minimal YAML config enabling the API and WHEP
        let config = format!(
            "api:\n  address: \":{api_port}\"\nwebrtc:\n  address: \":{whep_port}\"\n",
            api_port = self.api_port,
            whep_port = self.whep_port,
        );

        let config_path = std::env::temp_dir().join("mediamtx.yml");
        std::fs::write(&config_path, config)
            .map_err(|e| format!("Failed to write MediaMTX config: {e}"))?;

        let binary_path = self.binary_path.clone();
        let config_path_str = config_path.to_string_lossy().to_string();

        tokio::spawn(async move {
            loop {
                info!("Spawning MediaMTX process: {}", binary_path);
                let result = Command::new(&binary_path)
                    .arg(&config_path_str)
                    .kill_on_drop(true)
                    .spawn();

                match result {
                    Ok(mut child) => match child.wait().await {
                        Ok(status) => {
                            warn!(exit_code = ?status.code(), "MediaMTX exited, restarting in 3s...");
                        }
                        Err(e) => {
                            error!("MediaMTX process error: {e}");
                        }
                    },
                    Err(e) => {
                        error!("Failed to spawn MediaMTX: {e}");
                    }
                }

                sleep(Duration::from_secs(3)).await;
            }
        });

        Ok(())
    }
}
