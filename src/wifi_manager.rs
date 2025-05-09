use std::{path::Path, time::Duration};

use serde::Deserialize;
use tokio::io::AsyncWriteExt;

use crate::{WiFiCfgSubscriber, WifiConfigError};

const WPA_SUPPLICANT_PATH: &str = "/etc/wpa_supplicant/wpa_supplicant.conf";
const WPA_SUPPLICANT_BACKUP_PATH: &str = "/etc/wpa_supplicant/wpa_supplicant.conf.bak";

#[derive(Debug, Deserialize)]
pub struct WiFiCredentials {
    pub ssid: String,
    pub psswd: String,
    pub country: String,
}

pub struct WiFiManager;

impl WiFiCfgSubscriber for WiFiManager {
    async fn apply_wifi_config(&self, config_str: &str) -> Result<(), WifiConfigError> {
        tracing::info!("'apply_wifi_config' with content {config_str}");

        Self::rescan().await?;

        let credentials: WiFiCredentials = serde_json::from_str(config_str)?;

        Self::connect(&credentials, Duration::from_secs(5)).await
    }
}

impl WiFiManager {
    async fn rescan() -> Result<(), WifiConfigError> {
        tracing::info!("Attempting to rescan");

        let nmcli_output = tokio::process::Command::new("nmcli")
            .args(["dev", "wifi", "rescan"])
            .output()
            .await?;

        tracing::debug!("nmcli rescan: status = {:?}.", nmcli_output.status);

        tokio::time::sleep(Duration::from_millis(250)).await;

        if nmcli_output.status.success() {
            tracing::info!("nmcli rescan success.");
            Ok(())
        } else {
            tracing::error!("nmcli rescan failed: {:?}", nmcli_output);
            Err(WifiConfigError::NmcliRescanFailed)
        }
    }
    async fn connect(credentials: &WiFiCredentials, timeout_duration: Duration) -> Result<(), WifiConfigError> {
        tracing::info!("Attempting to connect to SSID '{}'", credentials.ssid);

        let connect_cmd = tokio::process::Command::new("nmcli")
            .args(["dev", "wifi", "connect", credentials.ssid.as_str(), "password", credentials.psswd.as_str()])
            .output();

        let result = tokio::time::timeout(timeout_duration, connect_cmd).await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                if output.status.success() {
                    tracing::info!("Successfully connected to '{}'. stdout: {}", credentials.ssid, stdout);
                    Ok(())
                } else {
                    tracing::error!(
                        "Failed to connect to '{}'. Status: {:?}, stderr: {}",
                        credentials.ssid,
                        output.status,
                        stderr
                    );
                    Err(WifiConfigError::NmcliConnectFailed)
                }
            }
            Ok(Err(e)) => {
                tracing::error!("Error running nmcli command: {:?}", e);
                Err(e.into())
            }
            Err(_) => {
                tracing::error!("Timed out while trying to connect to '{}'", credentials.ssid);
                Err(WifiConfigError::Timeout)
            }
        }
    }
}