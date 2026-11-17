mod actor;
mod build_actor;
mod cli;
mod commands;
mod config;
mod docker;
mod docker_actor;
mod docker_flags;
mod docker_infra;
mod env;
mod exec_shutdown;
mod formatter;
mod health;
mod lifecycle;
mod message;
mod process;
mod shutdown;
mod startup;
mod supervisor;
mod utils;

use std::process::ExitCode;

use clap::Parser as _;
use tokio::sync::mpsc;

use crate::cli::Cli;

#[tokio::main]
async fn main() -> ExitCode {
    if let Err(err) = color_eyre::install() {
        eprintln!("failed to install color_eyre: {err}");
        return ExitCode::from(1);
    }

    let cli = Cli::parse();

    let command = match cli.command {
        cli::Command::Version => {
            let output = commands::version::run();
            print!("{}", output.text);
            if output.success {
                return ExitCode::SUCCESS;
            }
            return ExitCode::FAILURE;
        }
        cli::Command::Process(command) => command,
    };

    let config_path = cli::config_path(&command);
    let config = match config::load(config_path) {
        Ok(config) => config,
        Err(err) => {
            println!("{}", formatter::render_error_msg(&err.to_string()));
            return ExitCode::from(1);
        }
    };

    let (log_tx, mut log_rx) = mpsc::unbounded_channel();

    let global_env_file = cli::env_file_path(&command).cloned();
    let config_dir = match cli::config_path(&command).canonicalize() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("failed to resolve config path: {err}");
            return ExitCode::from(1);
        }
    };
    let Some(project_name) = config_dir
        .parent()
        .and_then(std::path::Path::file_name)
        .map(|name| name.to_string_lossy().to_string())
    else {
        eprintln!("failed to resolve project name from config path");
        return ExitCode::from(1);
    };
    let supervisor_future = supervisor::run(config, command, global_env_file, project_name, log_tx);
    let supervisor = tokio::spawn(supervisor_future);

    while let Some(line) = log_rx.recv().await {
        println!("{line}");
    }

    match supervisor.await {
        Ok(Ok(result)) => {
            match (result.code, &result.failed_name) {
                (0, _) => println!("{}", formatter::render_ok()),
                (_, Some(name)) => println!("{}", formatter::render_error(name)),
                (_, None) => {}
            }
            ExitCode::from(result.code)
        }
        Ok(Err(err)) => {
            println!("{}", formatter::render_error_msg(&err.to_string()));
            ExitCode::from(1)
        }
        Err(join_err) => {
            eprintln!("supervisor panicked: {join_err}");
            ExitCode::from(1)
        }
    }
}
