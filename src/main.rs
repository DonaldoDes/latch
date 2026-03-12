mod cli;
mod client;
mod server;
mod session;
mod tui;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::New { name, command }) => {
            eprintln!("TODO: create session '{}' with command {:?}", name, command);
        }
        Some(Commands::Attach { session }) => {
            eprintln!("TODO: attach to session '{}'", session);
        }
        Some(Commands::Detach) => {
            eprintln!("TODO: detach from current session");
        }
        Some(Commands::List) => {
            eprintln!("TODO: list sessions");
        }
        Some(Commands::Kill { session }) => {
            eprintln!("TODO: kill session '{}'", session);
        }
        Some(Commands::History { session }) => {
            eprintln!("TODO: show history for session '{}'", session);
        }
        Some(Commands::Rename { session, new_name }) => {
            eprintln!("TODO: rename session '{}' to '{}'", session, new_name);
        }
        None => {
            // No subcommand: launch TUI picker
            eprintln!("TODO: launch TUI session picker");
        }
    }

    Ok(())
}
