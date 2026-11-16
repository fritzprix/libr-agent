//! TUI `/playbooks` library screens (issue #2816 / parity §3.3).

use crate::services::playbook_service::{PlaybookDto, PlaybookService};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaybooksCommand {
    List {
        query: Option<String>,
        sort: Option<String>,
    },
    Delete {
        id: String,
    },
}

/// Parse `/playbooks …` composer lines.
pub fn parse_playbooks_command(text: &str) -> Option<PlaybooksCommand> {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') || trimmed.starts_with("//") {
        return None;
    }
    let mut parts = trimmed[1..].split_whitespace();
    let verb = parts.next()?.to_ascii_lowercase();
    if verb != "playbooks" {
        return None;
    }

    let mut query: Option<String> = None;
    let mut sort: Option<String> = None;
    let mut delete_id: Option<String> = None;
    let mut search_tokens = Vec::new();

    while let Some(tok) = parts.next() {
        let lower = tok.to_ascii_lowercase();
        match lower.as_str() {
            "delete" => {
                delete_id = parts.next().map(str::to_string);
            }
            "search" => {
                // remaining non-flag tokens collected below via search_tokens
            }
            "--sort" => {
                sort = parts.next().map(str::to_string);
            }
            other if other.starts_with("--sort=") => {
                sort = Some(other.trim_start_matches("--sort=").to_string());
            }
            other => search_tokens.push(other.to_string()),
        }
    }

    if let Some(id) = delete_id {
        return Some(PlaybooksCommand::Delete { id });
    }
    if !search_tokens.is_empty() {
        query = Some(search_tokens.join(" "));
    }
    Some(PlaybooksCommand::List { query, sort })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    List,
    ConfirmDelete,
}

#[derive(Debug)]
pub struct PlaybooksScreen {
    view: View,
    rows: Vec<PlaybookDto>,
    filter: String,
    sort: String,
    selected: usize,
    confirm: Option<(String, String)>, // id, assistant_id
    notice: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaybooksAction {
    None,
    Closed,
}

impl PlaybooksScreen {
    pub fn open(query: Option<String>, sort: Option<String>) -> Result<Self, String> {
        let mut screen = Self {
            view: View::List,
            rows: Vec::new(),
            filter: query.unwrap_or_default(),
            sort: sort.unwrap_or_else(|| "created_at".into()),
            selected: 0,
            confirm: None,
            notice: None,
        };
        screen.reload()?;
        Ok(screen)
    }

    pub fn open_delete(id: &str) -> Result<Self, String> {
        let mut screen = Self::open(None, None)?;
        let row = screen
            .rows
            .iter()
            .find(|r| r.id == id)
            .ok_or_else(|| format!("Playbook not found: {id}"))?;
        screen.confirm = Some((row.id.clone(), row.assistant_id.clone()));
        screen.view = View::ConfirmDelete;
        Ok(screen)
    }

    fn reload(&mut self) -> Result<(), String> {
        let repo = crate::state::try_get_playbook_repository()
            .ok_or_else(|| "Playbooks not ready yet".to_string())?;
        let mut rows = tauri::async_runtime::block_on(PlaybookService::list_playbooks(
            repo,
            String::new(),
            Some(self.sort.clone()),
            Some(true),
        ))?;
        if !self.filter.is_empty() {
            let q = self.filter.to_lowercase();
            rows.retain(|r| {
                r.goal.to_lowercase().contains(&q)
                    || r.id.to_lowercase().contains(&q)
                    || r.assistant_id.to_lowercase().contains(&q)
            });
        }
        self.rows = rows;
        if self.selected >= self.rows.len() {
            self.selected = self.rows.len().saturating_sub(1);
        }
        Ok(())
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> PlaybooksAction {
        match self.view {
            View::ConfirmDelete => self.handle_confirm(key),
            View::List => self.handle_list(key),
        }
    }

    fn handle_list(&mut self, key: KeyEvent) -> PlaybooksAction {
        match key.code {
            KeyCode::Esc => PlaybooksAction::Closed,
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                PlaybooksAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected + 1 < self.rows.len() {
                    self.selected += 1;
                }
                PlaybooksAction::None
            }
            KeyCode::Char('s') => {
                self.sort = if self.sort == "assistant" {
                    "created_at".into()
                } else {
                    "assistant".into()
                };
                let _ = self.reload();
                self.notice = Some(format!("Sort: {}", self.sort));
                PlaybooksAction::None
            }
            KeyCode::Char('d') | KeyCode::Delete => {
                if let Some(row) = self.rows.get(self.selected) {
                    self.confirm = Some((row.id.clone(), row.assistant_id.clone()));
                    self.view = View::ConfirmDelete;
                }
                PlaybooksAction::None
            }
            KeyCode::Char('b') => {
                if let Some(row) = self.rows.get(self.selected) {
                    let id = row.id.clone();
                    let assistant_id = row.assistant_id.clone();
                    let was_bookmarked = row.is_bookmarked;
                    match self.toggle_bookmark(&id, &assistant_id, !was_bookmarked) {
                        Ok(()) => {
                            self.notice = Some(if was_bookmarked {
                                "Bookmark removed".into()
                            } else {
                                "Bookmarked".into()
                            });
                            let _ = self.reload();
                        }
                        Err(err) => self.notice = Some(err),
                    }
                }
                PlaybooksAction::None
            }
            _ => PlaybooksAction::None,
        }
    }

    fn handle_confirm(&mut self, key: KeyEvent) -> PlaybooksAction {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some((id, assistant_id)) = self.confirm.take() {
                    match self.delete_one(&id, &assistant_id) {
                        Ok(()) => {
                            self.notice = Some(format!("Deleted playbook {id}"));
                            let _ = self.reload();
                            self.view = View::List;
                        }
                        Err(err) => {
                            self.notice = Some(err);
                            self.view = View::List;
                        }
                    }
                }
                PlaybooksAction::None
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.confirm = None;
                self.view = View::List;
                self.notice = Some("Delete cancelled".into());
                PlaybooksAction::None
            }
            _ => PlaybooksAction::None,
        }
    }

    fn delete_one(&self, id: &str, assistant_id: &str) -> Result<(), String> {
        let repo = crate::state::try_get_playbook_repository()
            .ok_or_else(|| "Playbooks not ready yet".to_string())?;
        tauri::async_runtime::block_on(PlaybookService::delete_playbook(repo, id, assistant_id))
    }

    fn toggle_bookmark(
        &self,
        id: &str,
        assistant_id: &str,
        bookmarked: bool,
    ) -> Result<(), String> {
        let repo = crate::state::try_get_playbook_repository()
            .ok_or_else(|| "Playbooks not ready yet".to_string())?;
        tauri::async_runtime::block_on(PlaybookService::toggle_playbook_bookmark(
            repo,
            id,
            assistant_id,
            bookmarked,
        ))
    }
}

pub fn draw_playbooks(frame: &mut Frame<'_>, screen: &PlaybooksScreen) {
    match screen.view {
        View::ConfirmDelete => draw_confirm(frame, screen),
        View::List => draw_list(frame, screen),
    }
}

fn draw_list(frame: &mut Frame<'_>, screen: &PlaybooksScreen) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(frame.area());
    let mut title = format!(" Playbooks · sort={} ", screen.sort);
    if !screen.filter.is_empty() {
        title = format!(
            " Playbooks · filter: {} · sort={} ",
            screen.filter, screen.sort
        );
    }
    frame.render_widget(Paragraph::new(title), chunks[0]);
    let items: Vec<ListItem> = screen
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let prefix = if i == screen.selected { ">" } else { " " };
            let star = if row.is_bookmarked { "*" } else { " " };
            ListItem::new(format!(
                "{prefix}{star} {} · {} · {}",
                row.goal, row.id, row.assistant_id
            ))
        })
        .collect();
    frame.render_widget(
        List::new(items).block(Block::default().borders(Borders::ALL)),
        chunks[1],
    );
    let footer = screen
        .notice
        .as_deref()
        .unwrap_or("↑/↓ · d delete · b bookmark · s sort · Esc back");
    frame.render_widget(
        Paragraph::new(format!(" {footer} ")).wrap(Wrap { trim: true }),
        chunks[2],
    );
}

fn draw_confirm(frame: &mut Frame<'_>, screen: &PlaybooksScreen) {
    let (id, _) = screen
        .confirm
        .as_ref()
        .map(|(i, a)| (i.as_str(), a.as_str()))
        .unwrap_or(("?", "?"));
    let text = format!("Delete playbook {id}?\n\n[y] Yes   [n]/Esc Cancel");
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(" Confirm "))
            .wrap(Wrap { trim: false }),
        centered(50, 30, frame.area()),
    );
}

fn centered(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(v[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_playbooks_commands() {
        assert_eq!(
            parse_playbooks_command("/playbooks"),
            Some(PlaybooksCommand::List {
                query: None,
                sort: None
            })
        );
        assert_eq!(
            parse_playbooks_command("/playbooks delete pb1"),
            Some(PlaybooksCommand::Delete { id: "pb1".into() })
        );
        assert_eq!(
            parse_playbooks_command("/playbooks report --sort assistant"),
            Some(PlaybooksCommand::List {
                query: Some("report".into()),
                sort: Some("assistant".into())
            })
        );
        assert_eq!(parse_playbooks_command("/assistants"), None);
    }
}
