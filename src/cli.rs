use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(about = "Simple process orchestrator")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    #[arg(long)]
    pub file: PathBuf,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Run,
    Up,
}
