#[cfg(target_os = "macos")]
use crate::util::exec::exec_capture;

#[cfg(target_os = "macos")]
pub async fn read_keychain_generic_password(
    account: &str,
    service: &str,
    timeout_ms: u64,
) -> Result<String, String> {
    let res = exec_capture(
        "security",
        &["find-generic-password", "-w", "-a", account, "-s", service],
        Some(timeout_ms),
    )
    .await;

    if res.code == 0 {
        Ok(res.stdout.trim().to_string())
    } else {
        let err = res.stderr.trim();
        Err(if err.is_empty() {
            format!("exit {}", res.code)
        } else {
            err.to_string()
        })
    }
}

#[cfg(target_os = "macos")]
pub async fn read_keychain_generic_password_first(
    account: &str,
    services: &[&str],
    timeout_ms: u64,
    label: &str,
) -> Result<String, String> {
    let mut last_error = None;
    for service in services {
        match read_keychain_generic_password(account, service, timeout_ms).await {
            Ok(password) => return Ok(password),
            Err(e) => last_error = Some(e),
        }
    }
    Err(format!(
        "Failed to read macOS Keychain ({label}): {}",
        last_error
            .unwrap_or_else(|| "permission denied / keychain locked / entry missing.".to_string())
    ))
}
