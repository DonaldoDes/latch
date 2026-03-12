use anyhow::Result;
use std::process;

use crate::session::{generate_session_id, SessionMeta, SessionPaths, SessionStatus};

/// Run `latch new [name] [cmd]`
/// Spawns a daemonised server process with a PTY child.
pub fn run(name: Option<String>, command: Option<String>) -> Result<()> {
    let id = generate_session_id();
    let session_name = name.unwrap_or_else(|| id.clone());
    let cmd =
        command.unwrap_or_else(|| std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string()));

    let paths = SessionPaths::new(&id);
    paths.ensure_dir()?;

    // Fork: child becomes the server process
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        anyhow::bail!("fork() failed");
    }

    if pid > 0 {
        // Parent: write meta.json with the child PID then exit
        let meta = SessionMeta {
            id: id.clone(),
            name: session_name.clone(),
            cmd: cmd.clone(),
            pid: pid as u32,
            created_at: chrono::Utc::now().to_rfc3339(),
            status: SessionStatus::Detached,
        };
        meta.write_to(&paths.meta)?;
        println!(
            "Session '{}' created (id: {}, pid: {})",
            session_name, id, pid
        );
        return Ok(());
    }

    // Child: daemonise via setsid
    unsafe {
        if libc::setsid() == -1 {
            eprintln!("setsid() failed");
            process::exit(1);
        }
    }

    // Run the async server (PTY + IPC loop)
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(crate::server::run_server(&paths, &cmd, &session_name));

    if let Err(e) = result {
        eprintln!("Server error: {}", e);
    }

    process::exit(0);
}
