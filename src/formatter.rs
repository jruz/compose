use owo_colors::{OwoColorize as _, Style};

const COLORS: &[fn() -> Style] = &[
    || Style::new().cyan(),
    || Style::new().green(),
    || Style::new().yellow(),
    || Style::new().magenta(),
    || Style::new().blue(),
    || Style::new().red(),
];

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

pub fn pad_width(names: &[String]) -> Option<usize> {
    names.iter().map(String::len).max()
}
