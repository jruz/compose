use crate::utils::output::Output;

const VERSION_STRING: &str = concat!(
    "\nversion: ",
    env!("CARGO_PKG_VERSION"),
    "\nrevision: ",
    env!("REVISION"),
    "\nbuild-time: ",
    env!("BUILD_TIME"),
);

#[must_use]
pub fn text() -> &'static str {
    VERSION_STRING
}

#[must_use]
pub fn run() -> Output {
    let text = text();
    Output {
        success: true,
        text: format!("{} {text}\n", env!("CARGO_PKG_NAME")),
    }
}
