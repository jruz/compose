use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    about = "Simple process orchestrator",
    long_about = "\
Simple process orchestrator

Reads a TOML compose file and runs processes with colored, prefixed output.
Processes start in dependency order. Health checks gate dependent startup.

Config schema:

  [[process]]
  name         = <string>           # unique process name
  command      = <string>           # shell command to run
  depends_on   = [<string>, ...]    # processes that must be healthy first
  env_file     = <string>           # optional path to KEY=VALUE file
  [process.env]                     # optional env vars (overrides env_file)
  KEY = \"VALUE\"
  [process.health]                  # optional health check
  command       = <string>          # health check shell command
  interval_secs = <int>             # seconds between retries
  timeout_secs  = <int>             # seconds before giving up"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    #[arg(long, help = "Path to the TOML compose file")]
    pub file: PathBuf,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "Run all processes, exit with the last process's exit code")]
    Run,
    #[command(about = "Keep all processes running, any exit triggers shutdown")]
    Up,
}
