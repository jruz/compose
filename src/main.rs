mod cli;
mod config;
mod formatter;
mod health;
mod orchestrator;
mod process;

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

    let config = match config::load(&cli.file) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(1);
        }
    };

    let (log_tx, mut log_rx) = mpsc::unbounded_channel();

    let orchestrator = tokio::spawn(orchestrator::run(config, cli.command, log_tx));

    while let Some(line) = log_rx.recv().await {
        println!("{line}");
    }

    match orchestrator.await {
        Ok(Ok(code)) => ExitCode::from(code),
        Ok(Err(err)) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
        Err(join_err) => {
            eprintln!("orchestrator panicked: {join_err}");
            ExitCode::from(1)
        }
    }
}
