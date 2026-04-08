#[cfg(target_os = "windows")]
pub async fn dpapi_unprotect(data: &[u8], timeout_ms: Option<u64>) -> Result<Vec<u8>, String> {
    use crate::util::exec::exec_capture;
    use base64::Engine;

    let timeout = timeout_ms.unwrap_or(5_000);
    let input_b64 = base64::engine::general_purpose::STANDARD.encode(data);

    let prelude = "try { Add-Type -AssemblyName System.Security.Cryptography.ProtectedData -ErrorAction Stop } catch { try { Add-Type -AssemblyName System.Security -ErrorAction Stop } catch {} };";
    let script = format!(
        "{prelude}$in=[Convert]::FromBase64String('{input_b64}');\
         $out=[System.Security.Cryptography.ProtectedData]::Unprotect(\
         $in,$null,[System.Security.Cryptography.DataProtectionScope]::CurrentUser);\
         [Convert]::ToBase64String($out)"
    );

    let res = exec_capture(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", &script],
        Some(timeout),
    )
    .await;

    if res.code != 0 {
        let err = res.stderr.trim();
        return Err(if err.is_empty() {
            format!("powershell exit {}", res.code)
        } else {
            err.to_string()
        });
    }

    base64::engine::general_purpose::STANDARD
        .decode(res.stdout.trim())
        .map_err(|e| e.to_string())
}
