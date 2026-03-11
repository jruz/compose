mod actor;
mod build_actor;
mod cli;
mod config;
mod docker;
mod docker_actor;
mod docker_infra;
mod env;
mod formatter;
mod lifecycle;
mod message;
mod process;
mod shutdown;
mod startup;
mod supervisor;

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

    let config = match config::load(cli::config_path(&cli.command)) {
        Ok(config) => config,
        Err(err) => {
            println!("{}", formatter::render_error_msg(&err.to_string()));
            return ExitCode::from(1);
        }
    };

    let (log_tx, mut log_rx) = mpsc::unbounded_channel();

    let global_env_file = cli::env_file_path(&cli.command).cloned();
    let project_name = match cli::config_path(&cli.command).canonicalize() {
        Ok(path) => path
            .parent()
            .and_then(std::path::Path::file_name)
            .map(|name| name.to_string_lossy().to_string()),
        Err(err) => {
            eprintln!("failed to resolve config path: {err}");
            None
        }
    };
    let supervisor = tokio::spawn(supervisor::run(
        config,
        cli.command,
        global_env_file,
        project_name,
        log_tx,
    ));

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
