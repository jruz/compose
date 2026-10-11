const VERSION_STRING: &str = concat!(
    "\nversion: ",
    env!("CARGO_PKG_VERSION"),
    "\nrevision: ",
    env!("REVISION"),
    "\nbuild-time: ",
    env!("BUILD_TIME"),
);

#[must_use]
pub fn clap_version() -> &'static str {
    VERSION_STRING
}
