//! TUI `/tools` session tools list (issue #2817 / parity §3.3).

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolsCommand {
    List,
}

pub fn parse_tools_command(text: &str) -> Option<ToolsCommand> {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') || trimmed.starts_with("//") {
        return None;
    }
    let mut parts = trimmed[1..].split_whitespace();
    let verb = parts.next()?.to_ascii_lowercase();
    if verb != "tools" {
        return None;
    }
    Some(ToolsCommand::List)
}

#[derive(Debug, Clone)]
pub struct ToolRow {
    pub name: String,
    pub description: String,
}

#[derive(Debug)]
pub struct ToolsScreen {
    pub rows: Vec<ToolRow>,
    pub selected: usize,
    pub filter: String,
    pub notice: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolsAction {
    None,
    Closed,
}

impl ToolsScreen {
    pub fn open(session_id: Option<&str>) -> Result<Self, String> {
        let session_id =
            session_id.ok_or_else(|| "Open a session first (/new or /resume)".to_string())?;
        let manager = crate::state::try_get_channel_dispatch_agent().ok_or_else(|| {
            "Tools require the primary backend process (not available in attached TUI mode)"
                .to_string()
        })?;
        let tools = tauri::async_runtime::block_on(manager.get_available_tools(session_id))?;
        let rows: Vec<ToolRow> = tools
            .into_iter()
            .map(|t| ToolRow {
                name: t.name,
                description: t.description,
            })
            .collect();
        Ok(Self {
            rows,
            selected: 0,
            filter: String::new(),
            notice: None,
        })
    }

    fn visible(&self) -> Vec<&ToolRow> {
        if self.filter.is_empty() {
            return self.rows.iter().collect();
        }
        let q = self.filter.to_lowercase();
        self.rows
            .iter()
            .filter(|r| {
                r.name.to_lowercase().contains(&q) || r.description.to_lowercase().contains(&q)
            })
            .collect()
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ToolsAction {
        match key.code {
            KeyCode::Esc => ToolsAction::Closed,
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                ToolsAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let len = self.visible().len();
                if self.selected + 1 < len {
                    self.selected += 1;
                }
                ToolsAction::None
            }
            KeyCode::Char('/') => {
                self.filter.clear();
                self.notice = Some("Type to filter (Backspace edits, Enter clears notice)".into());
                ToolsAction::None
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.selected = 0;
                ToolsAction::None
            }
            KeyCode::Enter => {
                self.notice = None;
                ToolsAction::None
            }
            KeyCode::Char(ch) => {
                self.filter.push(ch);
                self.selected = 0;
                ToolsAction::None
            }
            _ => ToolsAction::None,
        }
    }
}

pub fn draw_tools(frame: &mut Frame<'_>, screen: &ToolsScreen) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(frame.area());
    let title = if screen.filter.is_empty() {
        format!(" Session tools ({}) ", screen.rows.len())
    } else {
        format!(
            " Session tools ({}) · filter: {} ",
            screen.visible().len(),
            screen.filter
        )
    };
    frame.render_widget(Paragraph::new(title), chunks[0]);
    let visible = screen.visible();
    let items: Vec<ListItem> = visible
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let prefix = if i == screen.selected { ">" } else { " " };
            ListItem::new(format!("{prefix} {} — {}", row.name, row.description))
        })
        .collect();
    frame.render_widget(
        List::new(items).block(Block::default().borders(Borders::ALL)),
        chunks[1],
    );
    let footer = screen
        .notice
        .as_deref()
        .unwrap_or("↑/↓ · / filter · Esc close");
    frame.render_widget(
        Paragraph::new(format!(" {footer} ")).wrap(Wrap { trim: true }),
        chunks[2],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tools_slash_command() {
        assert_eq!(parse_tools_command("/tools"), Some(ToolsCommand::List));
        assert_eq!(
            parse_tools_command("/tools extra"),
            Some(ToolsCommand::List)
        );
        assert_eq!(parse_tools_command("/tool"), None);
    }

    #[test]
    fn tools_requires_open_session() {
        let err = ToolsScreen::open(None).unwrap_err();
        assert!(err.contains("Open a session first"));
    }

    #[test]
    fn tools_attached_mode_limitation() {
        let err = ToolsScreen::open(Some("non-existent-session-id")).unwrap_err();
        assert!(
            err.contains("Tools require the primary backend process")
                || err.contains("not available in attached TUI mode")
        );
    }

    #[test]
    fn tools_filter_and_key_navigation() {
        let mut screen = ToolsScreen {
            rows: vec![
                ToolRow {
                    name: "read_file".into(),
                    description: "Read file contents".into(),
                },
                ToolRow {
                    name: "write_file".into(),
                    description: "Write content to disk".into(),
                },
                ToolRow {
                    name: "search_web".into(),
                    description: "Search online".into(),
                },
            ],
            selected: 0,
            filter: String::new(),
            notice: None,
        };

        assert_eq!(screen.visible().len(), 3);

        // Move down
        let act = screen.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(act, ToolsAction::None);
        assert_eq!(screen.selected, 1);

        // Filter 'file'
        screen.handle_key(KeyEvent::from(KeyCode::Char('/')));
        screen.handle_key(KeyEvent::from(KeyCode::Char('f')));
        screen.handle_key(KeyEvent::from(KeyCode::Char('i')));
        screen.handle_key(KeyEvent::from(KeyCode::Char('l')));
        screen.handle_key(KeyEvent::from(KeyCode::Char('e')));
        assert_eq!(screen.visible().len(), 2);

        // Backspace
        screen.handle_key(KeyEvent::from(KeyCode::Backspace));
        assert_eq!(screen.filter, "fil");

        // Esc closes
        let act = screen.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(act, ToolsAction::Closed);
    }
}
