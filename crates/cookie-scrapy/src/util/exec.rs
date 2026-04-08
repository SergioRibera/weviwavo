use std::time::Duration;
use tokio::process::Command;

#[derive(Debug)]
pub struct ExecResult {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub async fn exec_capture(program: &str, args: &[&str], timeout_ms: Option<u64>) -> ExecResult {
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(10_000));

    let result = tokio::time::timeout(timeout, async {
        let output = Command::new(program)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .await;

        match output {
            Ok(output) => ExecResult {
                code: output.status.code().unwrap_or(0),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            },
            Err(e) => ExecResult {
                code: 127,
                stdout: String::new(),
                stderr: e.to_string(),
            },
        }
    })
    .await;

    match result {
        Ok(r) => r,
        Err(_) => ExecResult {
            code: 124,
            stdout: String::new(),
            stderr: format!("Timed out after {timeout_ms:?}ms"),
        },
    }
}
