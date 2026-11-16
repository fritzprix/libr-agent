//! TUI `/skills` screens (issue #2817 / parity §3.3).

use crate::services::skill_service::{self, SkillMetadata};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillsCommand {
    List,
    Install { source: String },
    Delete { name: String },
    Reset,
    Folders,
}

pub fn parse_skills_command(text: &str) -> Option<SkillsCommand> {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') || trimmed.starts_with("//") {
        return None;
    }
    let mut parts = trimmed[1..].split_whitespace();
    let verb = parts.next()?.to_ascii_lowercase();
    if verb != "skills" {
        return None;
    }
    match parts.next().map(|s| s.to_ascii_lowercase()) {
        None => Some(SkillsCommand::List),
        Some(cmd) => match cmd.as_str() {
            "install" => {
                let source = parts.collect::<Vec<_>>().join(" ");
                if source.is_empty() {
                    None
                } else {
                    Some(SkillsCommand::Install { source })
                }
            }
            "delete" => Some(SkillsCommand::Delete {
                name: parts.next()?.to_string(),
            }),
            "reset" => Some(SkillsCommand::Reset),
            "folders" => Some(SkillsCommand::Folders),
            _ => Some(SkillsCommand::List),
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    List,
    Folders,
    ConfirmDelete,
    ConfirmReset,
}

#[derive(Debug, Clone)]
struct SkillRow {
    meta: SkillMetadata,
    locked: bool,
}

#[derive(Debug)]
pub struct SkillsScreen {
    view: View,
    rows: Vec<SkillRow>,
    selected: usize,
    folders_text: String,
    confirm_name: Option<String>,
    notice: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillsAction {
    None,
    Closed,
}

impl SkillsScreen {
    pub fn open() -> Result<Self, String> {
        let mut screen = Self {
            view: View::List,
            rows: Vec::new(),
            selected: 0,
            folders_text: String::new(),
            confirm_name: None,
            notice: None,
        };
        screen.reload()?;
        Ok(screen)
    }

    pub fn open_folders() -> Result<Self, String> {
        let overview =
            tauri::async_runtime::block_on(skill_service::get_managed_skills_overview())?;
        Ok(Self {
            view: View::Folders,
            rows: Vec::new(),
            selected: 0,
            folders_text: format!(
                "System:\n  {}\n\nUser:\n  {}",
                overview.system_directory, overview.user_directory
            ),
            confirm_name: None,
            notice: None,
        })
    }

    pub fn open_delete(name: &str) -> Result<Self, String> {
        let mut screen = Self::open()?;
        let row = screen
            .rows
            .iter()
            .find(|r| r.meta.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| format!("Skill not found: {name}"))?;
        if row.locked {
            return Err(format!("System skill '{name}' is locked"));
        }
        screen.confirm_name = Some(row.meta.name.clone());
        screen.view = View::ConfirmDelete;
        Ok(screen)
    }

    pub fn open_reset() -> Result<Self, String> {
        let mut screen = Self::open()?;
        screen.view = View::ConfirmReset;
        Ok(screen)
    }

    pub fn apply_install(source: &str) -> Result<String, String> {
        let source = source.trim();
        if source.is_empty() {
            return Err("Usage: /skills install <path|url>".into());
        }
        let is_url = source.starts_with("http://")
            || source.starts_with("https://")
            || source.starts_with("git@");
        let result = if is_url {
            tauri::async_runtime::block_on(skill_service::install_github_skills(
                source.to_string(),
                true,
                None,
                Some(true),
            ))?
        } else {
            tauri::async_runtime::block_on(skill_service::import_user_skills(
                source.to_string(),
                true,
                None,
                Some(true),
            ))?
        };
        Ok(format!(
            "Installed {} skill(s) from {source}",
            result.imported_names.len()
        ))
    }

    fn reload(&mut self) -> Result<(), String> {
        let overview =
            tauri::async_runtime::block_on(skill_service::get_managed_skills_overview())?;
        let mut rows: Vec<SkillRow> = overview
            .system_skills
            .into_iter()
            .map(|meta| SkillRow { meta, locked: true })
            .collect();
        rows.extend(overview.user_skills.into_iter().map(|meta| SkillRow {
            meta,
            locked: false,
        }));
        rows.sort_by(|a, b| a.meta.name.to_lowercase().cmp(&b.meta.name.to_lowercase()));
        self.rows = rows;
        if self.selected >= self.rows.len() {
            self.selected = self.rows.len().saturating_sub(1);
        }
        Ok(())
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> SkillsAction {
        match self.view {
            View::Folders => match key.code {
                KeyCode::Esc | KeyCode::Enter => SkillsAction::Closed,
                _ => SkillsAction::None,
            },
            View::ConfirmDelete => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    if let Some(name) = self.confirm_name.take() {
                        match tauri::async_runtime::block_on(skill_service::delete_user_skill(
                            name.clone(),
                        )) {
                            Ok(msg) => {
                                self.notice = Some(msg);
                                let _ = self.reload();
                            }
                            Err(err) => self.notice = Some(err),
                        }
                    }
                    self.view = View::List;
                    SkillsAction::None
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.confirm_name = None;
                    self.view = View::List;
                    self.notice = Some("Delete cancelled".into());
                    SkillsAction::None
                }
                _ => SkillsAction::None,
            },
            View::ConfirmReset => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    match tauri::async_runtime::block_on(skill_service::reset_user_skills()) {
                        Ok(msg) => {
                            self.notice = Some(msg);
                            let _ = self.reload();
                        }
                        Err(err) => self.notice = Some(err),
                    }
                    self.view = View::List;
                    SkillsAction::None
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.view = View::List;
                    self.notice = Some("Reset cancelled".into());
                    SkillsAction::None
                }
                _ => SkillsAction::None,
            },
            View::List => match key.code {
                KeyCode::Esc => SkillsAction::Closed,
                KeyCode::Up | KeyCode::Char('k') => {
                    self.selected = self.selected.saturating_sub(1);
                    SkillsAction::None
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.selected + 1 < self.rows.len() {
                        self.selected += 1;
                    }
                    SkillsAction::None
                }
                KeyCode::Char('d') | KeyCode::Delete => {
                    if let Some(row) = self.rows.get(self.selected) {
                        if row.locked {
                            self.notice =
                                Some(format!("System skill '{}' is locked", row.meta.name));
                        } else {
                            self.confirm_name = Some(row.meta.name.clone());
                            self.view = View::ConfirmDelete;
                        }
                    }
                    SkillsAction::None
                }
                KeyCode::Char('r') => {
                    self.view = View::ConfirmReset;
                    SkillsAction::None
                }
                KeyCode::Char('f') => {
                    match Self::open_folders() {
                        Ok(next) => *self = next,
                        Err(err) => self.notice = Some(err),
                    }
                    SkillsAction::None
                }
                _ => SkillsAction::None,
            },
        }
    }
}

pub fn draw_skills(frame: &mut Frame<'_>, screen: &SkillsScreen) {
    match screen.view {
        View::Folders => {
            frame.render_widget(
                Paragraph::new(format!("{}\n\nEsc / Enter close", screen.folders_text))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(" Skill folders "),
                    )
                    .wrap(Wrap { trim: false }),
                frame.area(),
            );
        }
        View::ConfirmDelete => {
            let name = screen.confirm_name.as_deref().unwrap_or("?");
            frame.render_widget(
                Paragraph::new(format!(
                    "Delete user skill {name}?\n\n[y] Yes  [n]/Esc Cancel"
                ))
                .block(Block::default().borders(Borders::ALL).title(" Confirm ")),
                frame.area(),
            );
        }
        View::ConfirmReset => {
            frame.render_widget(
                Paragraph::new("Reset all user skills?\n\n[y] Yes  [n]/Esc Cancel")
                    .block(Block::default().borders(Borders::ALL).title(" Confirm ")),
                frame.area(),
            );
        }
        View::List => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Min(3),
                    Constraint::Length(2),
                ])
                .split(frame.area());
            frame.render_widget(Paragraph::new(" Skills "), chunks[0]);
            let items: Vec<ListItem> = screen
                .rows
                .iter()
                .enumerate()
                .map(|(i, row)| {
                    let prefix = if i == screen.selected { ">" } else { " " };
                    let lock = if row.locked { " [locked]" } else { "" };
                    ListItem::new(format!(
                        "{prefix} {}{lock} — {}",
                        row.meta.name, row.meta.description
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
                .unwrap_or("↑/↓ · d delete · r reset · f folders · Esc");
            frame.render_widget(Paragraph::new(format!(" {footer} ")), chunks[2]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_skills_commands() {
        assert_eq!(parse_skills_command("/skills"), Some(SkillsCommand::List));
        assert_eq!(
            parse_skills_command("/skills install ./pack.skill"),
            Some(SkillsCommand::Install {
                source: "./pack.skill".into()
            })
        );
        assert_eq!(
            parse_skills_command("/skills delete demo"),
            Some(SkillsCommand::Delete {
                name: "demo".into()
            })
        );
        assert_eq!(
            parse_skills_command("/skills reset"),
            Some(SkillsCommand::Reset)
        );
        assert_eq!(
            parse_skills_command("/skills folders"),
            Some(SkillsCommand::Folders)
        );
    }
}
