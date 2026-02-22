mod cli;
mod config;
mod error;

use clap::Parser as _;
use color_eyre::Result;

use crate::cli::Cli;

fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();
    let config = config::load(&cli.file)?;

    match cli.command {
        cli::Command::Run => println!("run mode"),
        cli::Command::Up => println!("up mode"),
    }

    println!("loaded {} processes:", config.process.len());
    for proc in &config.process {
        let health = proc.health.as_deref().unwrap_or("none");
        let deps = proc.depends_on.join(", ");
        println!(
            "  {} -> {} (health: {health}, depends_on: [{deps}])",
            proc.name, proc.command,
        );
    }

    Ok(())
}
