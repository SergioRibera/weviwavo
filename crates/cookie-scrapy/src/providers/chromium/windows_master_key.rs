#[cfg(target_os = "windows")]
pub async fn get_windows_chromium_master_key(
    user_data_dir: &std::path::Path,
    label: &str,
) -> Result<Vec<u8>, String> {
    use super::windows_dpapi::dpapi_unprotect;
    use base64::Engine;

    let local_state_path = user_data_dir.join("Local State");
    if !local_state_path.exists() {
        return Err(format!("{label} Local State file not found."));
    }

    let raw = std::fs::read_to_string(&local_state_path)
        .map_err(|e| format!("Failed to parse {label} Local State: {e}"))?;

    let parsed: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| format!("Failed to parse {label} Local State: {e}"))?;

    let encrypted_key_b64 = parsed
        .get("os_crypt")
        .and_then(|o| o.get("encrypted_key"))
        .and_then(|k| k.as_str())
        .ok_or_else(|| format!("{label} Local State missing os_crypt.encrypted_key."))?;

    let encrypted_key = base64::engine::general_purpose::STANDARD
        .decode(encrypted_key_b64)
        .map_err(|_| format!("{label} Local State contains an invalid encrypted_key."))?;

    let prefix = b"DPAPI";
    if encrypted_key.len() < prefix.len() || &encrypted_key[..prefix.len()] != prefix {
        return Err(format!(
            "{label} encrypted_key does not start with DPAPI prefix."
        ));
    }

    let unprotected = dpapi_unprotect(&encrypted_key[prefix.len()..], None).await?;
    Ok(unprotected)
}
