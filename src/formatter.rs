use owo_colors::{OwoColorize as _, Style};

const COLORS: &[fn() -> Style] = &[
    || Style::new().cyan(),
    || Style::new().green(),
    || Style::new().yellow(),
    || Style::new().magenta(),
    || Style::new().blue(),
    || Style::new().red(),
];

pub struct Context {
    pad_width: usize,
    styles: Vec<(String, Style)>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unknown process name in formatter: {0}")]
    UnknownProcess(String),
}

#[derive(Clone)]
pub enum Event<'ev> {
    Crashed {
        name: &'ev str,
        code: u8,
    },
    Exited {
        name: &'ev str,
        code: u8,
    },
    Healthy {
        name: &'ev str,
    },
    Killing {
        name: &'ev str,
        reason: &'ev str,
    },
    Starting {
        name: &'ev str,
        command: &'ev str,
    },
    Summary {
        name: &'ev str,
        stderr_lines: &'ev [String],
    },
    WaitingHealth {
        name: &'ev str,
    },
}

pub fn build_context(names: &[String]) -> Option<Context> {
    let pad_width = names.iter().map(String::len).max()?;
    let styles = names
        .iter()
        .enumerate()
        .map(|(idx, name)| (name.clone(), color_for_index(idx)))
        .collect();
    Some(Context { pad_width, styles })
}

pub fn context_pad_width(ctx: &Context) -> usize {
    ctx.pad_width
}

pub fn lookup_style(ctx: &Context, name: &str) -> Result<Style, Error> {
    ctx.styles
        .iter()
        .find(|(nm, _style)| nm == name)
        .map(|(_nm, style)| *style)
        .ok_or_else(|| Error::UnknownProcess(name.to_string()))
}

pub fn make_tag(name: &str, pad_width: usize, style: Style) -> String {
    let padded = format!("{name:pad_width$}");
    format!("[{}]", padded.style(style))
}

pub fn render(ctx: &Context, event: &Event<'_>) -> Result<String, Error> {
    let name = event_name(event);
    let style = lookup_style(ctx, name)?;
    let tag = make_tag(name, ctx.pad_width, style);

    let line = match event {
        Event::Crashed { code, .. } => format!("{tag} crashed with code {code}"),
        Event::Exited { code, .. } => format!("{tag} exited with code {code}"),
        Event::Healthy { .. } => format!("{tag} healthy"),
        Event::Killing { reason, .. } => format!("{tag} killing ({reason})"),
        Event::Starting { command, .. } => format!("{tag} starting: {command}"),
        Event::Summary { stderr_lines, .. } => render_summary(&tag, stderr_lines),
        Event::WaitingHealth { .. } => format!("{tag} waiting for health check..."),
    };
    Ok(line)
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

fn event_name<'ev>(event: &Event<'ev>) -> &'ev str {
    match event {
        Event::Crashed { name, .. }
        | Event::Exited { name, .. }
        | Event::Healthy { name }
        | Event::Killing { name, .. }
        | Event::Starting { name, .. }
        | Event::Summary { name, .. }
        | Event::WaitingHealth { name } => name,
    }
}

fn render_summary(tag: &str, stderr_lines: &[String]) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.push(format!("{tag} --- last stderr ---"));
    for line in stderr_lines {
        parts.push(format!("{tag} {line}"));
    }
    parts.join("\n")
}
