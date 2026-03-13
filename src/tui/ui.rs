use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use super::state::{AppState, Mode, SessionEntry, SessionStatus};

/// Minimum terminal dimensions
pub const MIN_COLS: u16 = 80;
pub const MIN_ROWS: u16 = 24;

/// Check if terminal is too small
pub fn is_too_small(area: Rect) -> bool {
    area.width < MIN_COLS || area.height < MIN_ROWS
}

/// Render the too-small message
pub fn render_too_small(frame: &mut Frame) {
    let msg =
        Paragraph::new("Terminal too small. Minimum: 80x24").style(Style::default().fg(Color::Red));
    frame.render_widget(msg, frame.area());
}

/// Main render function
pub fn render(frame: &mut Frame, app: &mut AppState) {
    let area = frame.area();

    if is_too_small(area) {
        render_too_small(frame);
        return;
    }

    // Layout: main list + bottom bar
    let [list_area, prompt_area, help_area] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);

    // Render session list
    render_session_list(frame, app, list_area);

    // Render prompt bar (mode-dependent)
    render_prompt(frame, app, prompt_area);

    // Render help bar
    render_help_bar(frame, app, help_area);

    // Render help overlay if in Help mode
    if app.mode == Mode::Help {
        render_help_overlay(frame, area);
    }
}

/// Format a duration from seconds to human-readable
pub fn format_duration(secs: i64) -> String {
    if secs < 0 {
        return "(dead)".to_string();
    }
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    if hours > 0 {
        format!("{}h{:02}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

/// Get the status indicator character
pub fn status_indicator(status: &SessionStatus) -> &'static str {
    match status {
        SessionStatus::Current => "*",
        SessionStatus::Attached => "\u{25CF}", // filled circle
        SessionStatus::Detached => "\u{25CB}", // empty circle
        SessionStatus::Dead => "\u{2717}",     // cross mark
    }
}

/// Get the current-session marker
pub fn current_marker(status: &SessionStatus) -> &'static str {
    match status {
        SessionStatus::Current => "*",
        _ => " ",
    }
}

/// Build a line for a session entry
pub fn build_session_line(entry: &SessionEntry) -> Line<'static> {
    let marker = current_marker(&entry.status);
    let indicator = status_indicator(&entry.status);
    let name = entry.meta.name.clone();
    let cmd = entry.meta.cmd.clone();

    let duration_str = if entry.status == SessionStatus::Dead {
        "(dead)".to_string()
    } else {
        // Compute duration from created_at
        match chrono::DateTime::parse_from_rfc3339(&entry.meta.created_at) {
            Ok(created) => {
                let now = chrono::Utc::now();
                let secs = (now - created.with_timezone(&chrono::Utc)).num_seconds();
                format_duration(secs)
            }
            Err(_) => "?".to_string(),
        }
    };

    let style = if entry.status == SessionStatus::Dead {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
    };

    Line::from(vec![
        Span::styled(format!("  {}  ", marker), style),
        Span::styled(format!("{:<12}", name), style),
        Span::styled(format!("{}  ", indicator), style),
        Span::styled(format!("{:<8}", cmd), style),
        Span::styled(duration_str, style),
    ])
}

fn render_session_list(frame: &mut Frame, app: &mut AppState, area: Rect) {
    let visible = app.visible_sessions();
    let items: Vec<ListItem> = visible
        .iter()
        .map(|e| ListItem::new(build_session_line(e)))
        .collect();

    let list = List::new(items)
        .block(Block::bordered().title(" latch "))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    let mut list_state = ListState::default();
    list_state.select(Some(app.selected));

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_prompt(frame: &mut Frame, app: &AppState, area: Rect) {
    let text = match &app.mode {
        Mode::NewSession { input } => format!("\u{25B6} Session name: {}_", input),
        Mode::Filter { input } => format!("\u{25B6} Filter: {}_", input),
        Mode::KillConfirm { name, .. } => format!("\u{25B6} Kill '{}'? [y/N] _", name),
        Mode::Rename {
            current_name,
            input,
            ..
        } => format!("\u{25B6} Rename: {} \u{2192} {}_", current_name, input),
        _ => String::new(),
    };

    let prompt = Paragraph::new(text).style(Style::default().fg(Color::Yellow));
    frame.render_widget(prompt, area);
}

fn render_help_bar(frame: &mut Frame, _app: &AppState, area: Rect) {
    let help_text = "j/k navigate  Enter attach  n new  x kill  r rename  / filter  ? help  q quit";
    let help = Paragraph::new(help_text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, area);
}

fn render_help_overlay(frame: &mut Frame, area: Rect) {
    // Center a box
    let width = 50u16.min(area.width.saturating_sub(4));
    let height = 14u16.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let overlay_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, overlay_area);

    let help_lines = vec![
        Line::from(""),
        Line::from("  j / Down    Move down"),
        Line::from("  k / Up      Move up"),
        Line::from("  Enter       Attach to session"),
        Line::from("  n           New session"),
        Line::from("  x           Kill session"),
        Line::from("  r           Rename session"),
        Line::from("  /           Filter sessions"),
        Line::from("  ?           Toggle help"),
        Line::from("  q / Esc     Quit"),
        Line::from(""),
        Line::from("  Press any key to close"),
    ];

    let help = Paragraph::new(help_lines)
        .block(Block::bordered().title(" Help "))
        .style(Style::default().fg(Color::White).bg(Color::Black));
    frame.render_widget(help, overlay_area);
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- format_duration tests ---

    #[test]
    fn format_duration_minutes_only() {
        assert_eq!(format_duration(47 * 60), "47m");
    }

    #[test]
    fn format_duration_hours_and_minutes() {
        assert_eq!(format_duration(2 * 3600 + 34 * 60), "2h34m");
    }

    #[test]
    fn format_duration_zero() {
        assert_eq!(format_duration(0), "0m");
    }

    #[test]
    fn format_duration_negative() {
        assert_eq!(format_duration(-1), "(dead)");
    }

    // --- status_indicator tests ---

    #[test]
    fn status_indicator_current() {
        assert_eq!(status_indicator(&SessionStatus::Current), "*");
    }

    #[test]
    fn status_indicator_attached() {
        assert_eq!(status_indicator(&SessionStatus::Attached), "\u{25CF}");
    }

    #[test]
    fn status_indicator_detached() {
        assert_eq!(status_indicator(&SessionStatus::Detached), "\u{25CB}");
    }

    #[test]
    fn status_indicator_dead() {
        assert_eq!(status_indicator(&SessionStatus::Dead), "\u{2717}");
    }

    // --- current_marker tests ---

    #[test]
    fn current_marker_for_current() {
        assert_eq!(current_marker(&SessionStatus::Current), "*");
    }

    #[test]
    fn current_marker_for_non_current() {
        assert_eq!(current_marker(&SessionStatus::Detached), " ");
        assert_eq!(current_marker(&SessionStatus::Attached), " ");
        assert_eq!(current_marker(&SessionStatus::Dead), " ");
    }

    // --- is_too_small tests ---

    #[test]
    fn too_small_width() {
        let area = Rect::new(0, 0, 79, 24);
        assert!(is_too_small(area));
    }

    #[test]
    fn too_small_height() {
        let area = Rect::new(0, 0, 80, 23);
        assert!(is_too_small(area));
    }

    #[test]
    fn adequate_size() {
        let area = Rect::new(0, 0, 80, 24);
        assert!(!is_too_small(area));
    }

    // --- build_session_line tests ---

    #[test]
    fn build_line_dead_has_dead_text() {
        let entry = SessionEntry {
            meta: crate::session::SessionMeta {
                id: "session-old".to_string(),
                name: "old".to_string(),
                cmd: "bash".to_string(),
                pid: 999,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                status: crate::session::SessionStatus::Dead,
            },
            status: SessionStatus::Dead,
        };
        let line = build_session_line(&entry);
        let text: String = line.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(text.contains("(dead)"));
        assert!(text.contains("\u{2717}")); // cross mark
    }

    #[test]
    fn build_line_current_has_star() {
        let entry = SessionEntry {
            meta: crate::session::SessionMeta {
                id: "session-work".to_string(),
                name: "work".to_string(),
                cmd: "bash".to_string(),
                pid: 999,
                created_at: chrono::Utc::now().to_rfc3339(),
                status: crate::session::SessionStatus::Attached,
            },
            status: SessionStatus::Current,
        };
        let line = build_session_line(&entry);
        let text: String = line.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(text.contains("*"));
    }

    #[test]
    fn build_line_detached_has_circle() {
        let entry = SessionEntry {
            meta: crate::session::SessionMeta {
                id: "session-dev".to_string(),
                name: "dev".to_string(),
                cmd: "zsh".to_string(),
                pid: 999,
                created_at: chrono::Utc::now().to_rfc3339(),
                status: crate::session::SessionStatus::Detached,
            },
            status: SessionStatus::Detached,
        };
        let line = build_session_line(&entry);
        let text: String = line.spans.iter().map(|s| s.content.to_string()).collect();
        assert!(text.contains("\u{25CB}")); // empty circle
    }
}
