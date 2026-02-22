use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("circular dependency detected involving: {0}")]
    CircularDependency(String),

    #[error("duplicate process name: {0}")]
    DuplicateName(String),

    #[error("failed to read file {path}: {source}")]
    FileRead {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("no processes defined in config")]
    NoProcesses,

    #[error("failed to parse TOML from {path}: {source}")]
    TomlParse {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("process {process} depends on unknown process: {dependency}")]
    UnknownDependency { dependency: String, process: String },
}
