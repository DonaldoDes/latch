use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "latch")]
#[command(version)]
#[command(about = "Transparent terminal session manager -- attach, detach, persist")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a new session
    New {
        /// Session name
        name: String,
        /// Command to run (default: $SHELL)
        command: Option<String>,
    },
    /// Attach to an existing session
    Attach {
        /// Session name or ID
        session: String,
    },
    /// Detach from the current session
    Detach,
    /// List all sessions
    List,
    /// Kill a session
    Kill {
        /// Session name or ID
        session: String,
    },
    /// Show session history
    History {
        /// Session name or ID
        session: String,
    },
    /// Rename a session
    Rename {
        /// Current session name or ID
        session: String,
        /// New name
        new_name: String,
    },
}
