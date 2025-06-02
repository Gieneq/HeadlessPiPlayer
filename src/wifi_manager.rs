use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct WiFiCredentialsCfg {
    ssid: String,
    psswd: String
}

pub fn wifi_manager_procedure(config_file_content: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let config: WiFiCredentialsCfg = serde_json::from_slice(config_file_content)
        .inspect_err(|_| {
            let str_content = String::from_utf8_lossy(config_file_content);
            tracing::warn!("Failed to parse wifi credentials file: {str_content}");
    })?;

    tracing::debug!("wifi config = {config:?}");

    // TODO finish with: nmcli dev wifi connect "<ssid>" password "<psswd>"
    Ok(())
}