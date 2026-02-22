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
  name = \"db\"
  command = \"postgres -D /data\"
  depends_on = []
  [process.health]
  command = \"pg_isready\"
  interval_secs = 1
  timeout_secs = 30

  [[process]]
  name = \"app\"
  command = \"./my-server\"
  depends_on = [\"db\"]
  env_file = \"path/to/.env\"
  [process.env]
  DATABASE_URL = \"postgres://localhost/mydb\"

Fields:
  name          Process name (must be unique)
  command       Shell command to run
  depends_on    List of process names that must be healthy first
  env           Optional table of environment variables (overrides env_file)
  env_file      Optional path to KEY=VALUE file (comments and blanks skipped)
  health        Optional health check (command, interval_secs, timeout_secs)"
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
