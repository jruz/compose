use std::time::Duration;

use tokio::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to run health check for {name}: {source}")]
    CommandFailed {
        name: String,
        source: std::io::Error,
    },

    #[error("health check timed out for {name} after {timeout:?}, last stderr: {last_stderr}")]
    Timeout {
        last_stderr: String,
        name: String,
        timeout: Duration,
    },
}

pub async fn wait_healthy(
    name: &str,
    cmd: &str,
    interval: Duration,
    timeout: Duration,
) -> Result<(), Error> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let output = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .output()
            .await
            .map_err(|source| Error::CommandFailed {
                name: name.to_string(),
                source,
            })?;

        if output.status.success() {
            break;
        }

        if tokio::time::Instant::now() >= deadline {
            let last_stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(Error::Timeout {
                name: name.to_string(),
                timeout,
                last_stderr,
            });
        }

        tokio::time::sleep(interval).await;
    }

    Ok(())
}
