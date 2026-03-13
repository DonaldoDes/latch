use crossterm::event::{KeyCode, KeyEvent};

use super::state::{Action, AppState, Mode, SessionStatus};

/// Handle a key event and return the resulting action
pub fn handle_key(app: &mut AppState, key: KeyEvent) -> Action {
    match &app.mode {
        Mode::Normal => handle_normal(app, key),
        Mode::NewSession { .. } => handle_new_session(app, key),
        Mode::Filter { .. } => handle_filter(app, key),
        Mode::Help => handle_help(app, key),
        Mode::KillConfirm { .. } => handle_kill_confirm(app, key),
        Mode::Rename { .. } => handle_rename(app, key),
    }
}

fn handle_normal(app: &mut AppState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
        KeyCode::Char('j') | KeyCode::Down => {
            app.move_down();
            Action::None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.move_up();
            Action::None
        }
        KeyCode::Enter => {
            if let Some(entry) = app.selected_entry() {
                match entry.status {
                    // Current and Dead: Enter is ignored
                    SessionStatus::Current | SessionStatus::Dead => Action::None,
                    // Attached and Detached: attach
                    SessionStatus::Attached | SessionStatus::Detached => Action::Attach {
                        session_id: entry.meta.id.clone(),
                    },
                }
            } else {
                Action::None
            }
        }
        KeyCode::Char('n') => {
            app.mode = Mode::NewSession {
                input: String::new(),
            };
            Action::None
        }
        KeyCode::Char('x') => {
            if let Some(entry) = app.selected_entry() {
                match entry.status {
                    SessionStatus::Dead => {
                        // Dead: cleanup immediately, no confirmation
                        Action::CleanupDead {
                            session_id: entry.meta.id.clone(),
                        }
                    }
                    _ => {
                        // All others: ask for confirmation
                        app.mode = Mode::KillConfirm {
                            session_id: entry.meta.id.clone(),
                            name: entry.meta.name.clone(),
                        };
                        Action::None
                    }
                }
            } else {
                Action::None
            }
        }
        KeyCode::Char('r') => {
            if let Some(entry) = app.selected_entry() {
                match entry.status {
                    SessionStatus::Dead => Action::None, // Ignored on Dead
                    _ => {
                        app.mode = Mode::Rename {
                            session_id: entry.meta.id.clone(),
                            current_name: entry.meta.name.clone(),
                            input: String::new(),
                        };
                        Action::None
                    }
                }
            } else {
                Action::None
            }
        }
        KeyCode::Char('/') => {
            app.mode = Mode::Filter {
                input: String::new(),
            };
            Action::None
        }
        KeyCode::Char('?') => {
            app.mode = Mode::Help;
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_new_session(app: &mut AppState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
            Action::None
        }
        KeyCode::Enter => {
            if let Mode::NewSession { ref input } = app.mode {
                let name = input.clone();
                app.mode = Mode::Normal;
                if name.is_empty() {
                    Action::None
                } else {
                    Action::NewSession { name }
                }
            } else {
                Action::None
            }
        }
        KeyCode::Char(c) => {
            if let Mode::NewSession { ref mut input } = app.mode {
                input.push(c);
            }
            Action::None
        }
        KeyCode::Backspace => {
            if let Mode::NewSession { ref mut input } = app.mode {
                input.pop();
            }
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_filter(app: &mut AppState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => {
            app.clear_filter();
            app.mode = Mode::Normal;
            Action::None
        }
        KeyCode::Enter => {
            // Keep the filter active, go back to Normal
            app.mode = Mode::Normal;
            Action::None
        }
        KeyCode::Char(c) => {
            if let Mode::Filter { ref mut input } = app.mode {
                input.push(c);
                let filter = input.clone();
                app.apply_filter(&filter);
            }
            Action::None
        }
        KeyCode::Backspace => {
            if let Mode::Filter { ref mut input } = app.mode {
                input.pop();
                let filter = input.clone();
                app.apply_filter(&filter);
            }
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_help(app: &mut AppState, key: KeyEvent) -> Action {
    // Any key closes help
    let _ = key;
    app.mode = Mode::Normal;
    Action::None
}

fn handle_kill_confirm(app: &mut AppState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            if let Mode::KillConfirm { ref session_id, .. } = app.mode {
                let id = session_id.clone();
                app.mode = Mode::Normal;
                Action::KillSession { session_id: id }
            } else {
                Action::None
            }
        }
        _ => {
            // Any other key = cancel (N by default)
            app.mode = Mode::Normal;
            Action::None
        }
    }
}

fn handle_rename(app: &mut AppState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
            Action::None
        }
        KeyCode::Enter => {
            if let Mode::Rename {
                ref session_id,
                ref input,
                ..
            } = app.mode
            {
                let id = session_id.clone();
                let new_name = input.clone();
                app.mode = Mode::Normal;
                if new_name.is_empty() {
                    Action::None
                } else {
                    Action::RenameSession {
                        session_id: id,
                        new_name,
                    }
                }
            } else {
                Action::None
            }
        }
        KeyCode::Char(c) => {
            if let Mode::Rename { ref mut input, .. } = app.mode {
                input.push(c);
            }
            Action::None
        }
        KeyCode::Backspace => {
            if let Mode::Rename { ref mut input, .. } = app.mode {
                input.pop();
            }
            Action::None
        }
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionMeta;
    use crate::tui::state::SessionEntry;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn make_entry(name: &str, status: SessionStatus) -> SessionEntry {
        SessionEntry {
            meta: SessionMeta {
                id: format!("session-{}", name),
                name: name.to_string(),
                cmd: "bash".to_string(),
                pid: 1000,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                status: crate::session::SessionStatus::Detached,
            },
            status,
        }
    }

    fn make_app(names: &[(&str, SessionStatus)]) -> AppState {
        let entries: Vec<SessionEntry> = names
            .iter()
            .map(|(n, s)| make_entry(n, s.clone()))
            .collect();
        AppState::new(entries)
    }

    // --- Normal mode: navigation ---

    #[test]
    fn j_moves_down() {
        let mut app = make_app(&[
            ("a", SessionStatus::Detached),
            ("b", SessionStatus::Detached),
        ]);
        let action = handle_key(&mut app, key(KeyCode::Char('j')));
        assert_eq!(action, Action::None);
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn k_moves_up() {
        let mut app = make_app(&[
            ("a", SessionStatus::Detached),
            ("b", SessionStatus::Detached),
        ]);
        app.selected = 1;
        let action = handle_key(&mut app, key(KeyCode::Char('k')));
        assert_eq!(action, Action::None);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn arrow_down_moves_down() {
        let mut app = make_app(&[
            ("a", SessionStatus::Detached),
            ("b", SessionStatus::Detached),
        ]);
        let action = handle_key(&mut app, key(KeyCode::Down));
        assert_eq!(action, Action::None);
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn arrow_up_moves_up() {
        let mut app = make_app(&[
            ("a", SessionStatus::Detached),
            ("b", SessionStatus::Detached),
        ]);
        app.selected = 1;
        let action = handle_key(&mut app, key(KeyCode::Up));
        assert_eq!(action, Action::None);
        assert_eq!(app.selected, 0);
    }

    // --- Normal mode: quit ---

    #[test]
    fn q_quits() {
        let mut app = make_app(&[("a", SessionStatus::Detached)]);
        assert_eq!(handle_key(&mut app, key(KeyCode::Char('q'))), Action::Quit);
    }

    #[test]
    fn esc_quits_in_normal_mode() {
        let mut app = make_app(&[("a", SessionStatus::Detached)]);
        assert_eq!(handle_key(&mut app, key(KeyCode::Esc)), Action::Quit);
    }

    // --- Normal mode: enter ---

    #[test]
    fn enter_on_detached_returns_attach() {
        let mut app = make_app(&[("dev", SessionStatus::Detached)]);
        let action = handle_key(&mut app, key(KeyCode::Enter));
        assert_eq!(
            action,
            Action::Attach {
                session_id: "session-dev".to_string()
            }
        );
    }

    #[test]
    fn enter_on_attached_returns_attach_multi_client() {
        let mut app = make_app(&[("dev", SessionStatus::Attached)]);
        let action = handle_key(&mut app, key(KeyCode::Enter));
        assert_eq!(
            action,
            Action::Attach {
                session_id: "session-dev".to_string()
            }
        );
    }

    #[test]
    fn enter_on_current_is_ignored() {
        let mut app = make_app(&[("cur", SessionStatus::Current)]);
        assert_eq!(handle_key(&mut app, key(KeyCode::Enter)), Action::None);
    }

    #[test]
    fn enter_on_dead_is_ignored() {
        let mut app = make_app(&[("old", SessionStatus::Dead)]);
        assert_eq!(handle_key(&mut app, key(KeyCode::Enter)), Action::None);
    }

    // --- Normal mode: new session ---

    #[test]
    fn n_enters_new_session_mode() {
        let mut app = make_app(&[("a", SessionStatus::Detached)]);
        handle_key(&mut app, key(KeyCode::Char('n')));
        assert_eq!(
            app.mode,
            Mode::NewSession {
                input: String::new()
            }
        );
    }

    // --- Normal mode: kill ---

    #[test]
    fn x_on_detached_enters_kill_confirm() {
        let mut app = make_app(&[("dev", SessionStatus::Detached)]);
        handle_key(&mut app, key(KeyCode::Char('x')));
        assert_eq!(
            app.mode,
            Mode::KillConfirm {
                session_id: "session-dev".to_string(),
                name: "dev".to_string(),
            }
        );
    }

    #[test]
    fn x_on_dead_returns_cleanup_immediately() {
        let mut app = make_app(&[("old", SessionStatus::Dead)]);
        let action = handle_key(&mut app, key(KeyCode::Char('x')));
        assert_eq!(
            action,
            Action::CleanupDead {
                session_id: "session-old".to_string()
            }
        );
        // Mode stays Normal (no confirmation needed)
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn x_on_current_enters_kill_confirm() {
        let mut app = make_app(&[("cur", SessionStatus::Current)]);
        handle_key(&mut app, key(KeyCode::Char('x')));
        assert_eq!(
            app.mode,
            Mode::KillConfirm {
                session_id: "session-cur".to_string(),
                name: "cur".to_string(),
            }
        );
    }

    // --- Normal mode: rename ---

    #[test]
    fn r_on_detached_enters_rename_mode() {
        let mut app = make_app(&[("dev", SessionStatus::Detached)]);
        handle_key(&mut app, key(KeyCode::Char('r')));
        assert_eq!(
            app.mode,
            Mode::Rename {
                session_id: "session-dev".to_string(),
                current_name: "dev".to_string(),
                input: String::new(),
            }
        );
    }

    #[test]
    fn r_on_dead_is_ignored() {
        let mut app = make_app(&[("old", SessionStatus::Dead)]);
        handle_key(&mut app, key(KeyCode::Char('r')));
        assert_eq!(app.mode, Mode::Normal);
    }

    // --- Normal mode: filter ---

    #[test]
    fn slash_enters_filter_mode() {
        let mut app = make_app(&[("a", SessionStatus::Detached)]);
        handle_key(&mut app, key(KeyCode::Char('/')));
        assert_eq!(
            app.mode,
            Mode::Filter {
                input: String::new()
            }
        );
    }

    // --- Normal mode: help ---

    #[test]
    fn question_mark_enters_help_mode() {
        let mut app = make_app(&[("a", SessionStatus::Detached)]);
        handle_key(&mut app, key(KeyCode::Char('?')));
        assert_eq!(app.mode, Mode::Help);
    }

    // --- NewSession mode ---

    #[test]
    fn new_session_typing_appends() {
        let mut app = make_app(&[("a", SessionStatus::Detached)]);
        app.mode = Mode::NewSession {
            input: String::new(),
        };
        handle_key(&mut app, key(KeyCode::Char('m')));
        handle_key(&mut app, key(KeyCode::Char('y')));
        assert_eq!(
            app.mode,
            Mode::NewSession {
                input: "my".to_string()
            }
        );
    }

    #[test]
    fn new_session_backspace_removes() {
        let mut app = make_app(&[("a", SessionStatus::Detached)]);
        app.mode = Mode::NewSession {
            input: "abc".to_string(),
        };
        handle_key(&mut app, key(KeyCode::Backspace));
        assert_eq!(
            app.mode,
            Mode::NewSession {
                input: "ab".to_string()
            }
        );
    }

    #[test]
    fn new_session_enter_creates_session() {
        let mut app = make_app(&[("a", SessionStatus::Detached)]);
        app.mode = Mode::NewSession {
            input: "work".to_string(),
        };
        let action = handle_key(&mut app, key(KeyCode::Enter));
        assert_eq!(
            action,
            Action::NewSession {
                name: "work".to_string()
            }
        );
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn new_session_enter_empty_does_nothing() {
        let mut app = make_app(&[("a", SessionStatus::Detached)]);
        app.mode = Mode::NewSession {
            input: String::new(),
        };
        let action = handle_key(&mut app, key(KeyCode::Enter));
        assert_eq!(action, Action::None);
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn new_session_esc_cancels() {
        let mut app = make_app(&[("a", SessionStatus::Detached)]);
        app.mode = Mode::NewSession {
            input: "partial".to_string(),
        };
        handle_key(&mut app, key(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
    }

    // --- Filter mode ---

    #[test]
    fn filter_typing_filters_live() {
        let mut app = make_app(&[
            ("work", SessionStatus::Detached),
            ("dev", SessionStatus::Detached),
        ]);
        app.mode = Mode::Filter {
            input: String::new(),
        };
        handle_key(&mut app, key(KeyCode::Char('d')));
        assert_eq!(app.visible_sessions().len(), 1);
        assert_eq!(app.visible_sessions()[0].meta.name, "dev");
    }

    #[test]
    fn filter_esc_clears_and_returns_to_normal() {
        let mut app = make_app(&[
            ("work", SessionStatus::Detached),
            ("dev", SessionStatus::Detached),
        ]);
        app.mode = Mode::Filter {
            input: "dev".to_string(),
        };
        app.apply_filter("dev");
        handle_key(&mut app, key(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.visible_sessions().len(), 2);
    }

    #[test]
    fn filter_enter_keeps_filter_active() {
        let mut app = make_app(&[
            ("work", SessionStatus::Detached),
            ("dev", SessionStatus::Detached),
        ]);
        app.mode = Mode::Filter {
            input: "dev".to_string(),
        };
        app.apply_filter("dev");
        handle_key(&mut app, key(KeyCode::Enter));
        assert_eq!(app.mode, Mode::Normal);
        // Filter is kept active
        assert_eq!(app.visible_sessions().len(), 1);
    }

    // --- Help mode ---

    #[test]
    fn help_any_key_closes() {
        let mut app = make_app(&[("a", SessionStatus::Detached)]);
        app.mode = Mode::Help;
        handle_key(&mut app, key(KeyCode::Char('x')));
        assert_eq!(app.mode, Mode::Normal);
    }

    // --- KillConfirm mode ---

    #[test]
    fn kill_confirm_y_returns_kill() {
        let mut app = make_app(&[("dev", SessionStatus::Detached)]);
        app.mode = Mode::KillConfirm {
            session_id: "session-dev".to_string(),
            name: "dev".to_string(),
        };
        let action = handle_key(&mut app, key(KeyCode::Char('y')));
        assert_eq!(
            action,
            Action::KillSession {
                session_id: "session-dev".to_string()
            }
        );
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn kill_confirm_n_cancels() {
        let mut app = make_app(&[("dev", SessionStatus::Detached)]);
        app.mode = Mode::KillConfirm {
            session_id: "session-dev".to_string(),
            name: "dev".to_string(),
        };
        let action = handle_key(&mut app, key(KeyCode::Char('n')));
        assert_eq!(action, Action::None);
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn kill_confirm_any_other_key_cancels() {
        let mut app = make_app(&[("dev", SessionStatus::Detached)]);
        app.mode = Mode::KillConfirm {
            session_id: "session-dev".to_string(),
            name: "dev".to_string(),
        };
        let action = handle_key(&mut app, key(KeyCode::Esc));
        assert_eq!(action, Action::None);
        assert_eq!(app.mode, Mode::Normal);
    }

    // --- Rename mode ---

    #[test]
    fn rename_typing_appends() {
        let mut app = make_app(&[("dev", SessionStatus::Detached)]);
        app.mode = Mode::Rename {
            session_id: "session-dev".to_string(),
            current_name: "dev".to_string(),
            input: String::new(),
        };
        handle_key(&mut app, key(KeyCode::Char('n')));
        handle_key(&mut app, key(KeyCode::Char('e')));
        handle_key(&mut app, key(KeyCode::Char('w')));
        if let Mode::Rename { ref input, .. } = app.mode {
            assert_eq!(input, "new");
        } else {
            panic!("expected Rename mode");
        }
    }

    #[test]
    fn rename_enter_returns_action() {
        let mut app = make_app(&[("dev", SessionStatus::Detached)]);
        app.mode = Mode::Rename {
            session_id: "session-dev".to_string(),
            current_name: "dev".to_string(),
            input: "newname".to_string(),
        };
        let action = handle_key(&mut app, key(KeyCode::Enter));
        assert_eq!(
            action,
            Action::RenameSession {
                session_id: "session-dev".to_string(),
                new_name: "newname".to_string(),
            }
        );
    }

    #[test]
    fn rename_enter_empty_does_nothing() {
        let mut app = make_app(&[("dev", SessionStatus::Detached)]);
        app.mode = Mode::Rename {
            session_id: "session-dev".to_string(),
            current_name: "dev".to_string(),
            input: String::new(),
        };
        let action = handle_key(&mut app, key(KeyCode::Enter));
        assert_eq!(action, Action::None);
    }

    #[test]
    fn rename_esc_cancels() {
        let mut app = make_app(&[("dev", SessionStatus::Detached)]);
        app.mode = Mode::Rename {
            session_id: "session-dev".to_string(),
            current_name: "dev".to_string(),
            input: "partial".to_string(),
        };
        handle_key(&mut app, key(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn rename_backspace_removes() {
        let mut app = make_app(&[("dev", SessionStatus::Detached)]);
        app.mode = Mode::Rename {
            session_id: "session-dev".to_string(),
            current_name: "dev".to_string(),
            input: "abc".to_string(),
        };
        handle_key(&mut app, key(KeyCode::Backspace));
        if let Mode::Rename { ref input, .. } = app.mode {
            assert_eq!(input, "ab");
        } else {
            panic!("expected Rename mode");
        }
    }
}
