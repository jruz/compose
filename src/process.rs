use owo_colors::{OwoColorize as _, Style};
use tokio::io::{AsyncBufReadExt as _, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

const COLORS: &[fn() -> Style] = &[
    || Style::new().cyan(),
    || Style::new().green(),
    || Style::new().yellow(),
    || Style::new().magenta(),
    || Style::new().blue(),
    || Style::new().red(),
];

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to spawn process {name}: {source}")]
    Spawn {
        name: String,
        source: std::io::Error,
    },
}

pub struct SpawnedProcess {
    pub child: tokio::process::Child,
    pub name: String,
}

pub fn color_for_index(index: usize) -> Style {
    let wrapped = index.checked_rem(COLORS.len());
    match wrapped {
        Some(pos) => match COLORS.get(pos) {
            Some(color_fn) => color_fn(),
            None => Style::new(),
        },
        None => Style::new(),
    }
}

pub fn make_tag(name: &str, pad_width: usize, style: Style) -> String {
    let padded = format!("{name:pad_width$}");
    format!("[{}]", padded.style(style))
}

pub fn spawn(
    name: &str,
    command: &str,
    pad_width: usize,
    style: Style,
    log_tx: mpsc::UnboundedSender<String>,
) -> Result<SpawnedProcess, Error> {
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
    let name_owned = name.to_string();

    if let Some(stdout) = stdout {
        let tag = make_tag(&name_owned, pad_width, style);
        tokio::spawn(stream_lines(tag, BufReader::new(stdout), log_tx.clone()));
    }

    if let Some(stderr) = stderr {
        let tag = make_tag(&name_owned, pad_width, style);
        tokio::spawn(stream_lines(tag, BufReader::new(stderr), log_tx));
    }

    Ok(SpawnedProcess {
        child,
        name: name_owned,
    })
}

async fn stream_lines<R: tokio::io::AsyncRead + Unpin>(
    tag: String,
    reader: BufReader<R>,
    log_tx: mpsc::UnboundedSender<String>,
) {
    let mut lines = reader.lines();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                if log_tx.send(format!("{tag} {line}")).is_err() {
                    break;
                }
            }
            Ok(None) => break,
            Err(err) => {
                let _report = log_tx.send(format!("{tag} pipe error: {err}"));
                break;
            }
        }
    }
}
