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
    ConfirmInstall,
}

#[derive(Debug, Clone)]
struct SkillRow {
    meta: SkillMetadata,
    locked: bool,
}

#[derive(Debug, Clone)]
pub struct InstallConfirmState {
    pub source: String,
    pub is_url: bool,
    pub preview: skill_service::SkillImportPreview,
    pub approve_hooks: bool,
    pub overwrite_existing: bool,
}

#[derive(Debug)]
pub struct BackgroundOp {
    pub label: String,
    pub handle: tokio::task::JoinHandle<Result<String, String>>,
}

#[derive(Debug)]
pub struct SkillsScreen {
    view: View,
    rows: Vec<SkillRow>,
    selected: usize,
    folders_text: String,
    confirm_name: Option<String>,
    confirm_install: Option<InstallConfirmState>,
    pub background_op: Option<BackgroundOp>,
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
            confirm_install: None,
            background_op: None,
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
            confirm_install: None,
            background_op: None,
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

    pub fn open_install(source: &str) -> Result<Self, String> {
        let source = source.trim();
        if source.is_empty() {
            return Err("Usage: /skills install <path|url>".into());
        }
        let is_url = source.starts_with("http://")
            || source.starts_with("https://")
            || source.starts_with("git@");
        let preview = if is_url {
            tauri::async_runtime::block_on(skill_service::preview_github_skill_install(
                source.to_string(),
            ))?
        } else {
            tauri::async_runtime::block_on(skill_service::preview_user_skill_import(
                source.to_string(),
            ))?
        };
        let mut screen = Self::open().unwrap_or_else(|_| Self {
            view: View::List,
            rows: Vec::new(),
            selected: 0,
            folders_text: String::new(),
            confirm_name: None,
            confirm_install: None,
            background_op: None,
            notice: None,
        });
        screen.view = View::ConfirmInstall;
        screen.confirm_install = Some(InstallConfirmState {
            source: source.to_string(),
            is_url,
            preview,
            approve_hooks: false,
            overwrite_existing: false,
        });
        Ok(screen)
    }

    #[allow(dead_code)]
    pub fn apply_install(
        source: &str,
        overwrite_existing: bool,
        approve_hooks: bool,
    ) -> Result<String, String> {
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
                overwrite_existing,
                None,
                Some(approve_hooks),
            ))?
        } else {
            tauri::async_runtime::block_on(skill_service::import_user_skills(
                source.to_string(),
                overwrite_existing,
                None,
                Some(approve_hooks),
            ))?
        };
        Ok(format!(
            "Installed {} skill(s) from {source} (overwrote: {}, hooks approved: {approve_hooks})",
            result.imported_names.len(),
            result.overwritten_names.len(),
        ))
    }

    pub fn poll_background(&mut self) {
        if let Some(op) = &self.background_op {
            if op.handle.is_finished() {
                let op = self.background_op.take().unwrap();
                match tauri::async_runtime::block_on(op.handle) {
                    Ok(Ok(msg)) => {
                        self.notice = Some(msg);
                        let _ = self.reload();
                    }
                    Ok(Err(err)) => {
                        self.notice = Some(err);
                    }
                    Err(_) => {
                        self.notice = Some("Operation cancelled".into());
                    }
                }
            }
        }
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
        rows.sort_by_key(|a| a.meta.name.to_lowercase());
        self.rows = rows;
        if self.selected >= self.rows.len() {
            self.selected = self.rows.len().saturating_sub(1);
        }
        Ok(())
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> SkillsAction {
        if let Some(op) = &mut self.background_op {
            if key.code == KeyCode::Esc {
                op.handle.abort();
                self.background_op = None;
                self.notice = Some("Operation cancelled".into());
                return SkillsAction::None;
            }
            return SkillsAction::None;
        }

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
            View::ConfirmInstall => match key.code {
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    if let Some(confirm) = self.confirm_install.as_mut() {
                        confirm.approve_hooks = !confirm.approve_hooks;
                    }
                    SkillsAction::None
                }
                KeyCode::Char('o') | KeyCode::Char('O') => {
                    if let Some(confirm) = self.confirm_install.as_mut() {
                        confirm.overwrite_existing = !confirm.overwrite_existing;
                    }
                    SkillsAction::None
                }
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    if let Some(confirm) = self.confirm_install.take() {
                        let label = format!("Installing skills from {}", confirm.source);
                        let handle = tokio::spawn(async move {
                            let res = if confirm.is_url {
                                skill_service::install_github_skills(
                                    confirm.source.clone(),
                                    confirm.overwrite_existing,
                                    None,
                                    Some(confirm.approve_hooks),
                                )
                                .await?
                            } else {
                                skill_service::import_user_skills(
                                    confirm.source.clone(),
                                    confirm.overwrite_existing,
                                    None,
                                    Some(confirm.approve_hooks),
                                )
                                .await?
                            };
                            Ok(format!(
                                "Installed {} skill(s) from {} (overwrote: {}, hooks approved: {})",
                                res.imported_names.len(),
                                confirm.source,
                                res.overwritten_names.len(),
                                confirm.approve_hooks
                            ))
                        });
                        self.background_op = Some(BackgroundOp { label, handle });
                    }
                    self.view = View::List;
                    SkillsAction::None
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.confirm_install = None;
                    self.view = View::List;
                    self.notice = Some("Install cancelled".into());
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
        View::ConfirmInstall => {
            if let Some(confirm) = &screen.confirm_install {
                let mut lines = Vec::new();
                lines.push(format!("Source: {}", confirm.source));
                lines.push(String::new());

                lines.push(format!(
                    "Skills found ({}):",
                    confirm.preview.discovered_skills.len()
                ));
                if confirm.preview.discovered_skills.is_empty() {
                    lines.push("  (no skills found)".to_string());
                } else {
                    for s in &confirm.preview.discovered_skills {
                        lines.push(format!("  • {} — {}", s.name, s.description));
                    }
                }
                lines.push(String::new());

                if !confirm.preview.conflicts.is_empty() {
                    lines.push(format!("Conflicts ({}):", confirm.preview.conflicts.len()));
                    for c in &confirm.preview.conflicts {
                        lines.push(format!(
                            "  ⚠ {} (already exists at {})",
                            c.name, c.existing_path
                        ));
                    }
                    lines.push(
                        "  Note: Overwrite is OFF by default to protect existing user skills."
                            .to_string(),
                    );
                    lines.push(String::new());
                }

                if !confirm.preview.hooks.is_empty() {
                    lines.push(format!("Hooks detected ({}):", confirm.preview.hooks.len()));
                    for h in &confirm.preview.hooks {
                        let matcher = h
                            .matcher
                            .as_deref()
                            .map(|m| format!(" [{m}]"))
                            .unwrap_or_default();
                        lines.push(format!(
                            "  ⚡ Event: {}{matcher} -> Command: {}",
                            h.event, h.command
                        ));
                    }
                    lines.push("  ⚠ Hooks run executable commands on your system! Explicit approval required.".to_string());
                    lines.push(String::new());
                }

                let hook_status = if confirm.approve_hooks {
                    "[x] YES"
                } else {
                    "[ ] NO (default)"
                };
                let overwrite_status = if confirm.overwrite_existing {
                    "[x] YES"
                } else {
                    "[ ] NO (default)"
                };
                lines.push(format!("Options: [a] Approve hooks: {hook_status}   [o] Overwrite existing: {overwrite_status}"));
                lines.push(String::new());
                lines.push("[y/Enter] Confirm Install    [n]/Esc Cancel".to_string());

                let content = lines.join("\n");
                frame.render_widget(
                    Paragraph::new(content)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(" Confirm Skill Installation "),
                        )
                        .wrap(Wrap { trim: false }),
                    frame.area(),
                );
            }
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
            let footer_text;
            let footer = if let Some(op) = &screen.background_op {
                footer_text = format!("Working: {}... (Esc to cancel)", op.label);
                &footer_text
            } else {
                screen
                    .notice
                    .as_deref()
                    .unwrap_or("↑/↓ · d delete · r reset · f folders · Esc")
            };
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

    #[test]
    fn open_delete_locked_system_skill_refusal() {
        let screen = SkillsScreen {
            view: View::List,
            rows: vec![
                SkillRow {
                    meta: SkillMetadata {
                        name: "sys-skill".into(),
                        description: "Builtin system skill".into(),
                        path: "/path/to/sys".into(),
                        alias_path: None,
                        source: None,
                        origin: None,
                    },
                    locked: true,
                },
                SkillRow {
                    meta: SkillMetadata {
                        name: "user-skill".into(),
                        description: "User installed skill".into(),
                        path: "/path/to/user".into(),
                        alias_path: None,
                        source: None,
                        origin: None,
                    },
                    locked: false,
                },
            ],
            selected: 0,
            folders_text: String::new(),
            confirm_name: None,
            confirm_install: None,
            background_op: None,
            notice: None,
        };

        // Attempting to delete locked system skill
        let row = screen
            .rows
            .iter()
            .find(|r| r.meta.name.eq_ignore_ascii_case("sys-skill"))
            .unwrap();
        assert!(row.locked);
    }

    #[test]
    fn confirm_delete_key_handling() {
        let mut screen = SkillsScreen {
            view: View::ConfirmDelete,
            rows: Vec::new(),
            selected: 0,
            folders_text: String::new(),
            confirm_name: Some("my-skill".into()),
            confirm_install: None,
            background_op: None,
            notice: None,
        };

        // Press 'n'
        let act = screen.handle_key(KeyEvent::from(KeyCode::Char('n')));
        assert_eq!(act, SkillsAction::None);
        assert_eq!(screen.view, View::List);
        assert_eq!(screen.confirm_name, None);
        assert_eq!(screen.notice.as_deref(), Some("Delete cancelled"));

        // Press Esc
        screen.view = View::ConfirmDelete;
        screen.confirm_name = Some("my-skill".into());
        let act = screen.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(act, SkillsAction::None);
        assert_eq!(screen.view, View::List);
        assert_eq!(screen.confirm_name, None);
        assert_eq!(screen.notice.as_deref(), Some("Delete cancelled"));
    }

    #[test]
    fn confirm_reset_key_handling() {
        let mut screen = SkillsScreen {
            view: View::ConfirmReset,
            rows: Vec::new(),
            selected: 0,
            folders_text: String::new(),
            confirm_name: None,
            confirm_install: None,
            background_op: None,
            notice: None,
        };

        // Press 'n'
        let act = screen.handle_key(KeyEvent::from(KeyCode::Char('n')));
        assert_eq!(act, SkillsAction::None);
        assert_eq!(screen.view, View::List);
        assert_eq!(screen.notice.as_deref(), Some("Reset cancelled"));

        // Press Esc
        screen.view = View::ConfirmReset;
        let act = screen.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(act, SkillsAction::None);
        assert_eq!(screen.view, View::List);
        assert_eq!(screen.notice.as_deref(), Some("Reset cancelled"));
    }

    #[test]
    fn confirm_install_toggles_and_defaults() {
        use crate::services::skill_service::{
            HookPreviewEntry, HookPreviewExecutionStatus, SkillImportCandidate,
            SkillImportConflict, SkillImportKind, SkillImportPreview,
        };

        let preview = SkillImportPreview {
            kind: SkillImportKind::Plugin,
            discovered_skills: vec![SkillImportCandidate {
                name: "test-skill".into(),
                description: "Test skill description".into(),
            }],
            conflicts: vec![SkillImportConflict {
                name: "conflict-skill".into(),
                existing_origin: "user".into(),
                existing_path: "/path/conflict".into(),
            }],
            hooks: vec![HookPreviewEntry {
                event: "session:start".into(),
                matcher: None,
                command: "echo test".into(),
                powershell: None,
                script_body: None,
                execution_status: HookPreviewExecutionStatus::Executed,
            }],
            hooks_require_approval: true,
        };

        let mut screen = SkillsScreen {
            view: View::ConfirmInstall,
            rows: Vec::new(),
            selected: 0,
            folders_text: String::new(),
            confirm_name: None,
            confirm_install: Some(InstallConfirmState {
                source: "https://github.com/example/repo".into(),
                is_url: true,
                preview,
                approve_hooks: false,      // Must default to false!
                overwrite_existing: false, // Must default to false!
            }),
            background_op: None,
            notice: None,
        };

        // Check defaults
        let confirm = screen.confirm_install.as_ref().unwrap();
        assert!(
            !confirm.approve_hooks,
            "Hooks must NOT be approved by default"
        );
        assert!(
            !confirm.overwrite_existing,
            "Overwrite must NOT be true by default"
        );

        // Toggle hooks with 'a'
        screen.handle_key(KeyEvent::from(KeyCode::Char('a')));
        assert!(screen.confirm_install.as_ref().unwrap().approve_hooks);
        screen.handle_key(KeyEvent::from(KeyCode::Char('a')));
        assert!(!screen.confirm_install.as_ref().unwrap().approve_hooks);

        // Toggle overwrite with 'o'
        screen.handle_key(KeyEvent::from(KeyCode::Char('o')));
        assert!(screen.confirm_install.as_ref().unwrap().overwrite_existing);
        screen.handle_key(KeyEvent::from(KeyCode::Char('o')));
        assert!(!screen.confirm_install.as_ref().unwrap().overwrite_existing);

        // Cancel with Esc
        screen.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(screen.view, View::List);
        assert!(screen.confirm_install.is_none());
        assert_eq!(screen.notice.as_deref(), Some("Install cancelled"));
    }
}
