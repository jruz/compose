use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::version;

#[derive(Debug, Parser)]
#[command(
    version = version::clap_version(),
    about = "Simple process orchestrator",
    long_about = "\
Simple process orchestrator

Reads a YAML compose file and runs processes with colored, prefixed output.
Processes start in dependency order. Health checks gate dependent startup.

Config schema:

  runtime: <docker|podman>            # container runtime (default: docker)
  shell: <string>                     # shell for exec processes (default: sh)
  processes:
    <name>:                           # unique process name
      type: <service|oneshot>         # process type (default: service)
      command: <string>               # shell command (exec) or CMD override (docker)
      exec: [<string>, ...]           # CMD override, exec form (docker only)
      docker:                         # container driver (alternative to command)
        image: <string>               # docker image (required if no build)
        build: <string>               # build context path (required if no image)
        dockerfile: <string>          # dockerfile path (optional)
        ports: [<host:container>]     # port mappings
        volumes: [<host:container>]   # volume mounts
        stop_timeout_secs: <int>      # seconds before force kill (default: 10)
      depends_on: [<string>, ...]     # processes that must be healthy first
      env_file: <string>              # optional path to KEY=VALUE file
      env:                            # optional env vars (overrides env_file)
        KEY: \"VALUE\"
      health:                         # optional health check (one of command or exec)
        command: <string>             # health check shell command
        exec: [<string>, ...]         # health check exec form (no shell)
        interval_secs: <int>          # seconds between retries
        timeout_secs: <int>           # seconds before giving up

Variable interpolation:

  All string values support ${VAR} interpolation from resolved env vars
  (inline env, env_file, --env-file) and host environment.
  Applies to: command, exec, ports, volumes, health.command, health.exec.
  Undefined variables stay literal: ${UNKNOWN} remains ${UNKNOWN}."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Args)]
pub struct ConfigArg {
    #[arg(long, help = "Path to the YAML compose file")]
    pub config: PathBuf,

    #[arg(long, help = "Path to a global KEY=VALUE env file")]
    pub env_file: Option<PathBuf>,
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

pub fn env_file_path(command: &Command) -> Option<&PathBuf> {
    match command {
        Command::Run(args) | Command::Up(args) => args.env_file.as_ref(),
    }
}
