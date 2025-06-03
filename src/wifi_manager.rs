use std::process::Command;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct WiFiCredentialsCfg {
    ssid: String,
    psswd: String
}

#[derive(Debug, thiserror::Error)]
pub enum WifiManagerError {
    #[error("StdIoError reason = '{0}'")]
    StdIoError(#[from] std::io::Error),

    #[error("DeserializationError reason = '{0}'")]
    DeserializationError(#[from] serde_json::Error),
}

pub fn wifi_manager_procedure(config_file_content: &[u8]) -> Result<String, WifiManagerError> {
    let config: WiFiCredentialsCfg = serde_json::from_slice(config_file_content)
        .inspect_err(|_| {
            let str_content = String::from_utf8_lossy(config_file_content);
            tracing::warn!("Failed to parse wifi credentials file: {str_content}");
    })?;

    tracing::debug!("wifi config = {config:?}");

    let output = Command::new("nmcli")
        .arg("dev")
        .arg("wifi")
        .arg("connect")
        .arg(&config.ssid)
        .arg("password")
        .arg(&config.psswd)
        .output()?;

    if output.status.success() {
        tracing::info!("wifi config = {config:?}, set.");
        Ok(format!("Connected to '{}'.", config.ssid))
    } else {
        let stderr_str = String::from_utf8_lossy(&output.stderr);
        tracing::error!(
            "Failed to connect to WiFi network '{}': {}",
            config.ssid,
            stderr_str
        );

        let error_str = format!(
            "nmcli failed with status {}: {}",
            output.status,
            stderr_str
        );
        Err(std::io::Error::new(std::io::ErrorKind::Other, error_str).into())
    }
}
