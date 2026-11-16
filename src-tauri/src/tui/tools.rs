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
struct ToolRow {
    name: String,
    description: String,
}

#[derive(Debug)]
pub struct ToolsScreen {
    rows: Vec<ToolRow>,
    selected: usize,
    filter: String,
    notice: Option<String>,
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
        let manager = crate::state::try_get_channel_dispatch_agent()
            .ok_or_else(|| "Agent session manager not ready yet".to_string())?;
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
}
