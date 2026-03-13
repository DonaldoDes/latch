pub mod events;
pub mod state;
pub mod ui;

use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use std::time::Duration;

use crate::commands::list::{check_liveness, LiveStatus};
use crate::session::{sessions_dir, SessionMeta, SessionPaths};
use state::{Action, AppState, Mode, SessionEntry, SessionStatus};

/// Detect the TUI session status from live probing
fn detect_status(meta: &SessionMeta, socket_path: &std::path::Path) -> SessionStatus {
    let current_session = std::env::var("LATCH_SESSION").ok();
    if current_session.as_deref() == Some(&meta.name) {
        return SessionStatus::Current;
    }

    match check_liveness(meta, socket_path) {
        LiveStatus::Attached => SessionStatus::Attached,
        LiveStatus::Detached => SessionStatus::Detached,
        LiveStatus::Dead => SessionStatus::Dead,
    }
}

/// Collect all sessions with TUI status detection
fn collect_tui_sessions() -> Result<Vec<SessionEntry>> {
    let base = sessions_dir();
    if !base.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for dir_entry in std::fs::read_dir(&base)? {
        let dir_entry = dir_entry?;
        let meta_path = dir_entry.path().join("meta.json");
        let socket_path = dir_entry.path().join("server.sock");

        if !meta_path.exists() {
            continue;
        }

        if let Ok(meta) = SessionMeta::read_from(&meta_path) {
            let status = detect_status(&meta, &socket_path);
            entries.push(SessionEntry { meta, status });
        }
    }

    // Sort: Current first, then alphabetically
    entries.sort_by(|a, b| {
        let a_current = a.status == SessionStatus::Current;
        let b_current = b.status == SessionStatus::Current;
        b_current
            .cmp(&a_current)
            .then(a.meta.name.cmp(&b.meta.name))
    });

    Ok(entries)
}

/// Refresh sessions in AppState, preserving selection
fn refresh_sessions(app: &mut AppState) -> Result<()> {
    let old_selected_id = app.selected_entry().map(|e| e.meta.id.clone());
    let new_sessions = collect_tui_sessions()?;
    app.sessions = new_sessions;

    // Re-apply filter if any
    if app.filtered_indices.is_some() {
        if let Mode::Filter { ref input } = app.mode {
            let filter = input.clone();
            app.apply_filter(&filter);
        }
    }

    // Try to restore selection
    if let Some(old_id) = old_selected_id {
        let visible = app.visible_sessions();
        if let Some(pos) = visible.iter().position(|e| e.meta.id == old_id) {
            app.selected = pos;
        }
    }

    // Clamp
    let len = app.visible_sessions().len();
    if len > 0 && app.selected >= len {
        app.selected = len - 1;
    }

    Ok(())
}

/// Cleanup a dead session directory
fn cleanup_dead(session_id: &str) -> Result<()> {
    let paths = SessionPaths::new(session_id);
    if paths.dir.exists() {
        std::fs::remove_dir_all(&paths.dir)?;
    }
    Ok(())
}

/// Run the TUI session picker
pub fn run() -> Result<Option<Action>> {
    let sessions = collect_tui_sessions()?;
    let mut app = AppState::new(sessions);

    let mut terminal = ratatui::init();

    let tick_rate = Duration::from_secs(2);
    let mut last_refresh = std::time::Instant::now();

    let result = loop {
        terminal.draw(|frame| ui::render(frame, &mut app))?;

        // Poll for events with short timeout
        let timeout = tick_rate
            .checked_sub(last_refresh.elapsed())
            .unwrap_or_else(|| Duration::from_millis(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                let action = events::handle_key(&mut app, key);
                match action {
                    Action::Quit => break None,
                    Action::Attach { session_id } => {
                        break Some(Action::Attach { session_id });
                    }
                    Action::NewSession { name } => {
                        break Some(Action::NewSession { name });
                    }
                    Action::KillSession { session_id } => {
                        // Kill then refresh
                        if let Err(e) = crate::commands::kill::run_by_id(&session_id) {
                            eprintln!("Kill failed: {}", e);
                        }
                        // Check if this was the current session
                        let was_current = app
                            .selected_entry()
                            .map(|e| e.status == SessionStatus::Current)
                            .unwrap_or(false);
                        refresh_sessions(&mut app)?;
                        if was_current {
                            break Some(Action::Quit);
                        }
                    }
                    Action::CleanupDead { session_id } => {
                        if let Err(e) = cleanup_dead(&session_id) {
                            eprintln!("Cleanup failed: {}", e);
                        }
                        refresh_sessions(&mut app)?;
                    }
                    Action::RenameSession {
                        session_id,
                        new_name,
                    } => {
                        if let Err(e) = crate::commands::rename::run_by_id(&session_id, &new_name) {
                            eprintln!("Rename failed: {}", e);
                        }
                        refresh_sessions(&mut app)?;
                    }
                    Action::None => {}
                }
            }
        }

        // Periodic refresh
        if last_refresh.elapsed() >= tick_rate {
            refresh_sessions(&mut app)?;
            last_refresh = std::time::Instant::now();
        }
    };

    ratatui::restore();

    Ok(result)
}
