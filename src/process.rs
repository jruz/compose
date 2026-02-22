use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use owo_colors::Style;
use tokio::io::{AsyncBufReadExt as _, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::formatter;

const STDERR_BUFFER_LINES: usize = 20;

pub type StderrBuffer = Arc<Mutex<Vec<String>>>;

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
    env: &BTreeMap<String, String>,
    pad_width: usize,
    style: Style,
    log_tx: mpsc::UnboundedSender<String>,
) -> Result<(tokio::process::Child, StderrBuffer), Error> {
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(command)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    for (key, value) in env {
        cmd.env(key, value);
    }

    let mut child = cmd.spawn().map_err(|source| Error::Spawn {
        name: name.to_string(),
        source,
    })?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stderr_buffer: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    if let Some(stdout) = stdout {
        let tag = formatter::make_tag(name, pad_width, style);
        tokio::spawn(stream_lines(
            tag,
            BufReader::new(stdout),
            log_tx.clone(),
            None,
        ));
    }

    if let Some(stderr) = stderr {
        let tag = formatter::make_tag(name, pad_width, style);
        tokio::spawn(stream_lines(
            tag,
            BufReader::new(stderr),
            log_tx,
            Some(Arc::clone(&stderr_buffer)),
        ));
    }

    Ok((child, stderr_buffer))
}

async fn stream_lines<R: tokio::io::AsyncRead + Unpin>(
    tag: String,
    reader: BufReader<R>,
    log_tx: mpsc::UnboundedSender<String>,
    capture: Option<Arc<Mutex<Vec<String>>>>,
) -> Result<(), Error> {
    let mut lines = reader.lines();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                if let Some(buf) = &capture
                    && let Ok(mut buf) = buf.lock()
                {
                    buf.push(line.clone());
                    if buf.len() > STDERR_BUFFER_LINES {
                        buf.remove(0);
                    }
                }
                log_tx
                    .send(format!("{tag} {line}"))
                    .map_err(Error::LogChannelClosed)?;
            }
            Ok(None) => break,
            Err(pipe_err) => return Err(Error::PipeRead(pipe_err)),
        }
    }
    Ok(())
}
