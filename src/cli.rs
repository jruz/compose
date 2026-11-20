use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::commands::version;

#[derive(Debug, Parser)]
#[command(
    version = version::text(),
    about = "Simple process orchestrator",
    long_about = "\
Simple process orchestrator

Reads a YAML compose file and runs processes with colored, prefixed output.
Processes start in dependency order. Health checks gate dependent startup.

Config schema:

  runtime: <docker|podman>            # container runtime (default: docker)
  shell: <string>                     # shell for exec processes (default: sh)
  services:
    <name>:                           # unique host, container, or oneshot service name
      type: <service|oneshot>         # process type (default: service)
      command: <string>               # shell command (exec) or CMD override (docker)
      exec: [<string>, ...]           # CMD override, exec form (docker only)
      docker:                         # container driver (alternative to command)
        image: <string>               # docker image (required if no build)
        build:                        # required if no image
          context: <string>           # positional build context
          file: <string>              # example: --file
          build-arg: {KEY: \"VALUE\"}   # omit to forward resolved env; {} to disable
        run:
          publish: [<host:container>] # example: repeated --publish
          init: <bool>                # example: --init=true or --init=false
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

  ${VAR} resolves from inline env, env_file, --env-file, then host environment.
  Applies to command, exec, health commands, env values, image, build context,
  and Docker build/run flag values. Keys are not interpolated.

Docker flags:

  Use long CLI flag names without leading dashes. A scalar becomes --flag=value;
  lists repeat the flag. Maps for build-arg and label produce --flag=KEY=value.
  Compose reserves flags that it manages for build output and container lifecycle.
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
    #[command(flatten)]
    Process(ProcessCommand),
    #[command(about = "Show version and build metadata")]
    Version,
}

#[derive(Debug, Subcommand)]
pub enum ProcessCommand {
    #[command(about = "Run all processes, exit with the last process's exit code")]
    Run(ConfigArg),
    #[command(about = "Keep all processes running, any exit triggers shutdown")]
    Up(ConfigArg),
}

pub fn config_path(command: &ProcessCommand) -> &PathBuf {
    match command {
        ProcessCommand::Run(args) | ProcessCommand::Up(args) => &args.config,
    }
}

pub fn env_file_path(command: &ProcessCommand) -> Option<&PathBuf> {
    match command {
        ProcessCommand::Run(args) | ProcessCommand::Up(args) => args.env_file.as_ref(),
    }
}
