//! The `vividctl` command line.

#![forbid(unsafe_code)]

mod cli;
mod commands;
#[cfg(feature = "gnome")]
mod extension;
mod render;

use std::{io, process::ExitCode};

use clap::Parser;
use vivid_engine::Engine;

fn main() -> ExitCode {
    let cli: cli::Cli = cli::Cli::parse();
    let mut engine: Engine = Engine::system();
    ExitCode::from(commands::run(
        &cli.command,
        &mut engine,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    ))
}
