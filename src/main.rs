use anyhow::Result;
use clap::Parser;
use latch::cli::{Cli, Commands};
use latch::commands;
use latch::tui::state::Action;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::New { name, command }) => {
            commands::new::run(name, command)?;
        }
        Some(Commands::Attach { session }) => {
            commands::attach::run(&session)?;
        }
        Some(Commands::Detach { session }) => {
            commands::detach::run(session)?;
        }
        Some(Commands::List) => {
            commands::list::run()?;
        }
        Some(Commands::Kill { session }) => {
            commands::kill::run(&session)?;
        }
        Some(Commands::History { session }) => {
            commands::history::run(&session)?;
        }
        Some(Commands::Rename { session, new_name }) => {
            commands::rename::run(&session, &new_name)?;
        }
        None => {
            // No subcommand: launch TUI picker
            if let Some(action) = latch::tui::run()? {
                match action {
                    Action::Attach { session_id } => {
                        commands::attach::run(&session_id)?;
                    }
                    Action::NewSession { name } => {
                        commands::new::run(Some(name), None)?;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
