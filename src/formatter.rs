use std::io::IsTerminal as _;

use owo_colors::{OwoColorize as _, Style};
use tokio::sync::mpsc;

use crate::message;

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
    #[error("no processes defined")]
    NoProcesses,

    #[error("unknown process name in formatter: {0}")]
    UnknownProcess(String),
}

pub fn spawn_task(
    names: &[String],
    mut display_rx: mpsc::UnboundedReceiver<message::DisplayMsg>,
    log_tx: mpsc::UnboundedSender<String>,
) -> Result<tokio::task::JoinHandle<()>, Error> {
    if names.is_empty() {
        return Err(Error::NoProcesses);
    }

    let use_color = std::io::stdout().is_terminal();
    let mut process_index = 0;
    let styles: Vec<(String, Style)> = names
        .iter()
        .map(|name| {
            let style = if !use_color {
                Style::new()
            } else if name == "docker" {
                Style::new().blue()
            } else {
                let style = color_for_index(process_index);
                process_index += 1;
                style
            };
            (name.clone(), style)
        })
        .collect();

    let handle = tokio::spawn(async move {
        while let Some(msg) = display_rx.recv().await {
            match render(&styles, &msg) {
                Ok(rendered) => {
                    if let Err(_send_err) = log_tx.send(rendered) {
                        break;
                    }
                }
                Err(err) => {
                    if let Err(_send_err) = log_tx.send(format!("[formatter] {err}")) {
                        break;
                    }
                }
            }
        }
    });

    Ok(handle)
}

fn render(styles: &[(String, Style)], msg: &message::DisplayMsg) -> Result<String, Error> {
    let name = message::display_msg_name(msg);
    let style = lookup_style(styles, name)?;
    let tag = make_tag(name, style);

    let line = match msg {
        message::DisplayMsg::Building { .. } => format!("{tag} building"),
        message::DisplayMsg::Built { .. } => format!("{tag} built"),
        message::DisplayMsg::CleanupError { reason, .. } => {
            format!("{tag} cleanup error: {reason}")
        }
        message::DisplayMsg::Completed { .. } => format!("{tag} completed"),
        message::DisplayMsg::Crashed { code, .. } => {
            format!("{tag} crashed with code {code}")
        }
        message::DisplayMsg::CreatingNetwork { .. } => format!("{tag} creating network"),
        message::DisplayMsg::Exited { code, .. } => format!("{tag} exited with code {code}"),
        message::DisplayMsg::HealthFailed { reason, .. } => {
            format!("{tag} {reason}")
        }
        message::DisplayMsg::Healthy { .. } => format!("{tag} healthy"),
        message::DisplayMsg::Killing { reason, .. } => format!("{tag} killing ({reason})"),
        message::DisplayMsg::Output { line, .. } => format!("{tag} {line}"),
        message::DisplayMsg::RemovingContainers { .. } => format!("{tag} removing containers"),
        message::DisplayMsg::RemovingNetwork { .. } => format!("{tag} removing network"),
        message::DisplayMsg::Starting { command, .. } => format!("{tag} starting: {command}"),
        message::DisplayMsg::Summary { stderr_lines, .. } => render_summary(&tag, stderr_lines),
        message::DisplayMsg::WaitingCompletion { .. } => {
            format!("{tag} waiting for completion")
        }
        message::DisplayMsg::WaitingHealth { .. } => {
            format!("{tag} waiting for health check...")
        }
    };
    Ok(line)
}

fn lookup_style(styles: &[(String, Style)], name: &str) -> Result<Style, Error> {
    styles
        .iter()
        .find(|(nm, _style)| nm == name)
        .map(|(_nm, style)| *style)
        .ok_or_else(|| Error::UnknownProcess(name.to_string()))
}

fn make_tag(name: &str, style: Style) -> String {
    format!("[{}]", name.style(style))
}

fn color_for_index(index: usize) -> Style {
    let wrapped = index.checked_rem(COLORS.len());
    match wrapped {
        Some(pos) => match COLORS.get(pos) {
            Some(color_fn) => color_fn(),
            None => Style::new(),
        },
        None => Style::new(),
    }
}

pub fn render_error_msg(msg: &str) -> String {
    let use_color = std::io::stdout().is_terminal();
    if use_color {
        let style = Style::new().red();
        format!("[{}] {}", "ERROR".style(style), msg.style(style))
    } else {
        format!("[ERROR] {msg}")
    }
}

pub fn render_ok() -> String {
    let use_color = std::io::stdout().is_terminal();
    if use_color {
        let style = Style::new().green();
        format!(
            "[{}] {}",
            "OK".style(style),
            "all processes completed successfully".style(style)
        )
    } else {
        "[OK] all processes completed successfully".to_string()
    }
}

pub fn render_error(name: &str) -> String {
    render_error_msg(&format!("{name} failed"))
}

fn render_summary(tag: &str, stderr_lines: &[String]) -> String {
    let use_color = std::io::stdout().is_terminal();
    let red = if use_color {
        Style::new().red()
    } else {
        Style::new()
    };

    let mut parts: Vec<String> = Vec::new();
    parts.push(format!("{tag} {}", "--- last stderr ---".style(red)));
    for line in stderr_lines {
        parts.push(format!("{tag} {}", line.style(red)));
    }
    parts.join("\n")
}
