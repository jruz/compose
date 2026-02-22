use owo_colors::Style;
use tokio::io::{AsyncBufReadExt as _, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::formatter;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("log channel closed: {0}")]
    LogChannelClosed(mpsc::error::SendError<String>),

    #[error("pipe read failed: {0}")]
    PipeRead(std::io::Error),

    #[error("failed to spawn process {name}: {source}")]
    Spawn {
        name: String,
        source: std::io::Error,
    },
}

pub fn spawn(
    name: &str,
    command: &str,
    pad_width: usize,
    style: Style,
    log_tx: mpsc::UnboundedSender<String>,
) -> Result<tokio::process::Child, Error> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|source| Error::Spawn {
            name: name.to_string(),
            source,
        })?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    if let Some(stdout) = stdout {
        let tag = formatter::make_tag(name, pad_width, style);
        tokio::spawn(stream_lines(tag, BufReader::new(stdout), log_tx.clone()));
    }

    if let Some(stderr) = stderr {
        let tag = formatter::make_tag(name, pad_width, style);
        tokio::spawn(stream_lines(tag, BufReader::new(stderr), log_tx));
    }

    Ok(child)
}

async fn stream_lines<R: tokio::io::AsyncRead + Unpin>(
    tag: String,
    reader: BufReader<R>,
    log_tx: mpsc::UnboundedSender<String>,
) -> Result<(), Error> {
    let mut lines = reader.lines();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => log_tx
                .send(format!("{tag} {line}"))
                .map_err(Error::LogChannelClosed)?,
            Ok(None) => break,
            Err(pipe_err) => return Err(Error::PipeRead(pipe_err)),
        }
    }
    Ok(())
}
