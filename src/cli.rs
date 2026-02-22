use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    about = "Simple process orchestrator",
    long_about = "\
Simple process orchestrator

Reads a YAML compose file and runs processes with colored, prefixed output.
Processes start in dependency order. Health checks gate dependent startup.

Config schema:

  processes:
    <name>:                           # unique process name
      command: <string>               # shell command to run
      depends_on: [<string>, ...]     # processes that must be healthy first
      env_file: <string>              # optional path to KEY=VALUE file
      env:                            # optional env vars (overrides env_file)
        KEY: \"VALUE\"
      health:                         # optional health check
        command: <string>             # health check shell command
        interval_secs: <int>          # seconds between retries
        timeout_secs: <int>           # seconds before giving up"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Args)]
pub struct ConfigArg {
    #[arg(long, help = "Path to the YAML compose file")]
    pub config: PathBuf,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "Run all processes, exit with the last process's exit code")]
    Run(ConfigArg),
    #[command(about = "Keep all processes running, any exit triggers shutdown")]
    Up(ConfigArg),
}

pub fn config_path(command: &Command) -> &PathBuf {
    match command {
        Command::Run(args) | Command::Up(args) => &args.config,
    }
}
