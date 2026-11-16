//! TUI `/assistants` library screens (issue #2816 / parity §3.3).
//!
//! Persistence uses the in-process assistant repository (same SQLite path as the
//! GUI). An attached second TUI that does not boot the backend (#2827) has no
//! local repository, so these screens return "Assistants not ready yet" until
//! HTTP CRUD for assistants exists on the shared Session API.

use crate::models::assistant::{AssistantDto, AssistantSummaryDto};
use crate::repositories::AssistantRepository;
use crate::services::AssistantService;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssistantsCommand {
    List { query: Option<String> },
    New,
    Edit { id: String },
    Delete { id: String },
}

/// Parse `/assistants …` composer lines.
pub fn parse_assistants_command(text: &str) -> Option<AssistantsCommand> {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') || trimmed.starts_with("//") {
        return None;
    }
    let mut parts = trimmed[1..].split_whitespace();
    let verb = parts.next()?.to_ascii_lowercase();
    if verb != "assistants" {
        return None;
    }
    match parts.next().map(|s| s.to_ascii_lowercase()) {
        None => Some(AssistantsCommand::List { query: None }),
        Some(cmd) if cmd == "new" => Some(AssistantsCommand::New),
        Some(cmd) if cmd == "edit" => {
            let id = parts.next()?.to_string();
            Some(AssistantsCommand::Edit { id })
        }
        Some(cmd) if cmd == "delete" => {
            let id = parts.next()?.to_string();
            Some(AssistantsCommand::Delete { id })
        }
        Some(cmd) if cmd == "search" => {
            let query = parts.collect::<Vec<_>>().join(" ");
            Some(AssistantsCommand::List {
                query: if query.is_empty() { None } else { Some(query) },
            })
        }
        Some(other) => {
            // `/assistants <text>` = search shortcut
            let mut rest = vec![other];
            rest.extend(parts.map(str::to_string));
            Some(AssistantsCommand::List {
                query: Some(rest.join(" ")),
            })
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    List,
    Edit,
    ConfirmDelete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditPage {
    General,
    Tools,
    Skills,
}

impl EditPage {
    fn title(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Tools => "Tools",
            Self::Skills => "Skills",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::General => Self::Tools,
            Self::Tools => Self::Skills,
            Self::Skills => Self::General,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::General => Self::Skills,
            Self::Tools => Self::General,
            Self::Skills => Self::Tools,
        }
    }
}

#[derive(Debug)]
struct EditForm {
    id: String,
    is_new: bool,
    name: String,
    description: String,
    system_prompt: String,
    /// Comma-separated `mcpServerIds` (Tools page; GUI key).
    mcp_server_ids: String,
    /// Comma-separated `disabledSkills` exclusion list (Skills page; GUI key).
    disabled_skills: String,
    page: EditPage,
    field: usize,
    editing: bool,
    buf: String,
    baseline_config: Value,
}

pub struct AssistantsScreen {
    view: View,
    rows: Vec<AssistantSummaryDto>,
    filter: String,
    selected: usize,
    edit: Option<EditForm>,
    confirm_id: Option<String>,
    notice: Option<String>,
    /// Injected for tests; production uses [`try_get_assistant_repository`].
    repo: Option<Arc<dyn AssistantRepository>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssistantsAction {
    None,
    Closed,
}

impl AssistantsScreen {
    pub fn open(query: Option<String>) -> Result<Self, String> {
        Self::open_with_repo(query, None)
    }

    /// Test / injectable entry: pass a repository when the process has none.
    pub fn open_with_repo(
        query: Option<String>,
        repo: Option<Arc<dyn AssistantRepository>>,
    ) -> Result<Self, String> {
        let mut screen = Self {
            view: View::List,
            rows: Vec::new(),
            filter: query.unwrap_or_default(),
            selected: 0,
            edit: None,
            confirm_id: None,
            notice: None,
            repo,
        };
        screen.reload()?;
        Ok(screen)
    }

    pub fn open_new() -> Result<Self, String> {
        let mut screen = Self::open(None)?;
        screen.begin_new();
        Ok(screen)
    }

    pub fn open_edit(id: &str) -> Result<Self, String> {
        let mut screen = Self::open(None)?;
        screen.begin_edit(id)?;
        Ok(screen)
    }

    pub fn open_delete(id: &str) -> Result<Self, String> {
        let mut screen = Self::open(None)?;
        if let Some(row) = screen.rows.iter().find(|r| r.id == id) {
            if row.deletion_protected {
                return Err(format!("Assistant {id} is deletion-protected"));
            }
        }
        screen.confirm_id = Some(id.to_string());
        screen.view = View::ConfirmDelete;
        Ok(screen)
    }

    fn with_repo<T>(
        &self,
        f: impl FnOnce(&dyn AssistantRepository) -> Result<T, String>,
    ) -> Result<T, String> {
        if let Some(repo) = &self.repo {
            f(repo.as_ref())
        } else {
            let repo = crate::state::try_get_assistant_repository()
                .ok_or_else(|| "Assistants not ready yet".to_string())?;
            f(repo)
        }
    }

    fn reload(&mut self) -> Result<(), String> {
        let models = self.with_repo(|repo| {
            tauri::async_runtime::block_on(AssistantService::list_assistants(repo))
        })?;
        let mut rows: Vec<AssistantSummaryDto> = models.into_iter().map(|m| m.into()).collect();
        if !self.filter.is_empty() {
            let q = self.filter.to_lowercase();
            rows.retain(|r| {
                r.name.to_lowercase().contains(&q)
                    || r.description
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&q)
                    || r.id.to_lowercase().contains(&q)
            });
        }
        rows.sort_by_key(|a| a.name.to_lowercase());
        self.rows = rows;
        if self.selected >= self.rows.len() {
            self.selected = self.rows.len().saturating_sub(1);
        }
        Ok(())
    }

    fn begin_new(&mut self) {
        self.edit = Some(EditForm {
            id: uuid::Uuid::new_v4().to_string(),
            is_new: true,
            name: String::new(),
            description: String::new(),
            system_prompt: String::new(),
            mcp_server_ids: String::new(),
            disabled_skills: String::new(),
            page: EditPage::General,
            field: 0,
            editing: false,
            buf: String::new(),
            baseline_config: json!({}),
        });
        self.view = View::Edit;
    }

    fn begin_edit(&mut self, id: &str) -> Result<(), String> {
        let model = self
            .with_repo(|repo| {
                tauri::async_runtime::block_on(AssistantService::get_assistant(repo, id))
            })?
            .ok_or_else(|| format!("Assistant not found: {id}"))?;
        let dto: AssistantDto = model.into();
        let config = dto.config.clone();
        let description = config
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let system_prompt = config
            .get("systemPrompt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let (mcp_server_ids, disabled_skills) = read_tools_skills_csv(&config);
        self.edit = Some(EditForm {
            id: dto.id,
            is_new: false,
            name: dto.name,
            description,
            system_prompt,
            mcp_server_ids,
            disabled_skills,
            page: EditPage::General,
            field: 0,
            editing: false,
            buf: String::new(),
            baseline_config: config,
        });
        self.view = View::Edit;
        Ok(())
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> AssistantsAction {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match self.view {
            View::ConfirmDelete => self.handle_confirm(key),
            View::Edit => self.handle_edit(key, ctrl),
            View::List => self.handle_list(key, ctrl),
        }
    }

    fn handle_list(&mut self, key: KeyEvent, ctrl: bool) -> AssistantsAction {
        match key.code {
            KeyCode::Esc => AssistantsAction::Closed,
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                AssistantsAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected + 1 < self.rows.len() {
                    self.selected += 1;
                }
                AssistantsAction::None
            }
            KeyCode::Char('n') if !ctrl => {
                self.begin_new();
                AssistantsAction::None
            }
            KeyCode::Char('e') | KeyCode::Enter => {
                if let Some(row) = self.rows.get(self.selected) {
                    let id = row.id.clone();
                    if let Err(err) = self.begin_edit(&id) {
                        self.notice = Some(err);
                    }
                }
                AssistantsAction::None
            }
            KeyCode::Char('d') | KeyCode::Delete => {
                if let Some(row) = self.rows.get(self.selected) {
                    if row.deletion_protected {
                        self.notice = Some(format!("{} is deletion-protected", row.id));
                    } else {
                        self.confirm_id = Some(row.id.clone());
                        self.view = View::ConfirmDelete;
                    }
                }
                AssistantsAction::None
            }
            KeyCode::Char('/') => {
                self.notice = Some("Type /assistants search <text> in the composer".into());
                AssistantsAction::None
            }
            _ => AssistantsAction::None,
        }
    }

    fn handle_confirm(&mut self, key: KeyEvent) -> AssistantsAction {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(id) = self.confirm_id.take() {
                    match self.delete_by_id(&id) {
                        Ok(()) => {
                            self.notice = Some(format!("Deleted assistant {id}"));
                            let _ = self.reload();
                            self.view = View::List;
                        }
                        Err(err) => {
                            self.notice = Some(err);
                            self.view = View::List;
                        }
                    }
                }
                AssistantsAction::None
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.confirm_id = None;
                self.view = View::List;
                self.notice = Some("Delete cancelled".into());
                AssistantsAction::None
            }
            _ => AssistantsAction::None,
        }
    }

    fn handle_edit(&mut self, key: KeyEvent, ctrl: bool) -> AssistantsAction {
        if ctrl && matches!(key.code, KeyCode::Char('s') | KeyCode::Char('S')) {
            match self.save_edit() {
                Ok(()) => {
                    self.notice = Some("Assistant saved".into());
                    let _ = self.reload();
                    self.view = View::List;
                    self.edit = None;
                }
                Err(err) => self.notice = Some(err),
            }
            return AssistantsAction::None;
        }
        if ctrl && matches!(key.code, KeyCode::Left) {
            if let Some(edit) = self.edit.as_mut() {
                edit.page = edit.page.prev();
                edit.field = 0;
                edit.editing = false;
            }
            return AssistantsAction::None;
        }
        if ctrl && matches!(key.code, KeyCode::Right) {
            if let Some(edit) = self.edit.as_mut() {
                edit.page = edit.page.next();
                edit.field = 0;
                edit.editing = false;
            }
            return AssistantsAction::None;
        }

        let Some(edit) = self.edit.as_mut() else {
            self.view = View::List;
            return AssistantsAction::None;
        };

        if edit.editing {
            match key.code {
                KeyCode::Esc => {
                    edit.editing = false;
                    edit.buf.clear();
                }
                KeyCode::Enter => {
                    Self::commit_field(edit);
                    edit.editing = false;
                    edit.buf.clear();
                }
                KeyCode::Backspace => {
                    edit.buf.pop();
                }
                KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    edit.buf.push(ch);
                }
                _ => {}
            }
            return AssistantsAction::None;
        }

        let field_count = match edit.page {
            EditPage::General => 3,
            EditPage::Tools | EditPage::Skills => 1,
        };
        match key.code {
            KeyCode::Esc => {
                self.edit = None;
                self.view = View::List;
                AssistantsAction::None
            }
            KeyCode::Up | KeyCode::BackTab => {
                if edit.field > 0 {
                    edit.field -= 1;
                }
                AssistantsAction::None
            }
            KeyCode::Down | KeyCode::Tab => {
                if edit.field + 1 < field_count {
                    edit.field += 1;
                }
                AssistantsAction::None
            }
            KeyCode::Enter => {
                edit.buf = match (edit.page, edit.field) {
                    (EditPage::General, 0) => edit.name.clone(),
                    (EditPage::General, 1) => edit.description.clone(),
                    (EditPage::General, 2) => edit.system_prompt.clone(),
                    (EditPage::Tools, _) => edit.mcp_server_ids.clone(),
                    (EditPage::Skills, _) => edit.disabled_skills.clone(),
                    _ => String::new(),
                };
                edit.editing = true;
                AssistantsAction::None
            }
            _ => AssistantsAction::None,
        }
    }

    fn commit_field(edit: &mut EditForm) {
        match (edit.page, edit.field) {
            (EditPage::General, 0) => edit.name = edit.buf.clone(),
            (EditPage::General, 1) => edit.description = edit.buf.clone(),
            (EditPage::General, 2) => edit.system_prompt = edit.buf.clone(),
            (EditPage::Tools, _) => edit.mcp_server_ids = edit.buf.clone(),
            (EditPage::Skills, _) => edit.disabled_skills = edit.buf.clone(),
            _ => {}
        }
    }

    fn save_edit(&mut self) -> Result<(), String> {
        let edit = self
            .edit
            .as_ref()
            .ok_or_else(|| "No assistant form open".to_string())?;
        let name = edit.name.trim();
        if name.is_empty() {
            return Err("Name is required".into());
        }
        let config = build_assistant_config(
            &edit.baseline_config,
            &edit.description,
            &edit.system_prompt,
            &edit.mcp_server_ids,
            &edit.disabled_skills,
        )?;

        let id = edit.id.clone();
        let is_new = edit.is_new;
        let name = name.to_string();
        self.with_repo(|repo| {
            if is_new {
                tauri::async_runtime::block_on(AssistantService::create_assistant(
                    repo, id, name, config,
                ))
                .map(|_| ())
            } else {
                tauri::async_runtime::block_on(AssistantService::update_assistant(
                    repo,
                    &id,
                    Some(name),
                    Some(config),
                ))
                .map(|_| ())
            }
        })
    }

    fn delete_by_id(&self, id: &str) -> Result<(), String> {
        self.with_repo(|repo| {
            tauri::async_runtime::block_on(AssistantService::delete_assistant(repo, id))
        })
    }
}

impl std::fmt::Debug for AssistantsScreen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssistantsScreen")
            .field("view", &self.view)
            .field("rows", &self.rows)
            .field("filter", &self.filter)
            .field("selected", &self.selected)
            .field("edit", &self.edit)
            .field("confirm_id", &self.confirm_id)
            .field("notice", &self.notice)
            .field("repo", &self.repo.as_ref().map(|_| "Some(..)"))
            .finish()
    }
}

/// Parse a comma-separated id list; reject tokens that contain whitespace.
fn parse_id_csv(csv: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for raw in csv.split(',') {
        let id = raw.trim();
        if id.is_empty() {
            continue;
        }
        if id.chars().any(char::is_whitespace) {
            return Err(format!("Invalid id (contains whitespace): {id}"));
        }
        out.push(id.to_string());
    }
    Ok(out)
}

fn string_array_csv(config: &Value, key: &str) -> String {
    config
        .get(key)
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

/// Read GUI-shaped Tools/Skills fields from an assistant config.
pub(crate) fn read_tools_skills_csv(config: &Value) -> (String, String) {
    (
        string_array_csv(config, "mcpServerIds"),
        string_array_csv(config, "disabledSkills"),
    )
}

/// Merge form fields into a GUI-shaped assistant config (`mcpServerIds`,
/// `disabledSkills` exclusion list). Removes the legacy TUI-only keys.
pub(crate) fn build_assistant_config(
    baseline: &Value,
    description: &str,
    system_prompt: &str,
    mcp_server_ids_csv: &str,
    disabled_skills_csv: &str,
) -> Result<Value, String> {
    let mut config = baseline.clone();
    if !config.is_object() {
        config = json!({});
    }
    let obj = config.as_object_mut().expect("object");
    obj.insert("description".into(), json!(description));
    obj.insert("systemPrompt".into(), json!(system_prompt));
    let servers = parse_id_csv(mcp_server_ids_csv)?;
    let disabled = parse_id_csv(disabled_skills_csv)?;
    obj.insert(
        "mcpServerIds".into(),
        Value::Array(servers.into_iter().map(Value::String).collect()),
    );
    obj.insert(
        "disabledSkills".into(),
        Value::Array(disabled.into_iter().map(Value::String).collect()),
    );
    obj.remove("mcpServers");
    obj.remove("skills");
    Ok(config)
}

pub fn draw_assistants(frame: &mut Frame<'_>, screen: &AssistantsScreen) {
    match screen.view {
        View::ConfirmDelete => draw_confirm(frame, screen),
        View::Edit => draw_edit(frame, screen),
        View::List => draw_list(frame, screen),
    }
}

fn draw_list(frame: &mut Frame<'_>, screen: &AssistantsScreen) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(frame.area());
    let title = if screen.filter.is_empty() {
        " Assistants ".to_string()
    } else {
        format!(" Assistants · filter: {} ", screen.filter)
    };
    frame.render_widget(Paragraph::new(title), chunks[0]);
    let items: Vec<ListItem> = screen
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let prefix = if i == screen.selected { ">" } else { " " };
            let prot = if row.deletion_protected {
                " [protected]"
            } else {
                ""
            };
            let desc = row.description.as_deref().unwrap_or("");
            ListItem::new(format!(
                "{prefix} {} ({}){prot}  {}",
                row.name, row.id, desc
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
        .unwrap_or("↑/↓ · Enter/e edit · n new · d delete · Esc back");
    frame.render_widget(
        Paragraph::new(format!(" {footer} ")).wrap(Wrap { trim: true }),
        chunks[2],
    );
}

fn draw_edit(frame: &mut Frame<'_>, screen: &AssistantsScreen) {
    let Some(edit) = &screen.edit else {
        return;
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(frame.area());
    let mode = if edit.is_new { "New" } else { "Edit" };
    frame.render_widget(
        Paragraph::new(format!(
            " {mode} assistant · {} · page {} ",
            edit.id,
            edit.page.title()
        )),
        chunks[0],
    );
    let lines = match edit.page {
        EditPage::General => vec![
            field_line(0, edit.field, edit.editing, "Name", &edit.name, &edit.buf),
            field_line(
                1,
                edit.field,
                edit.editing,
                "Description",
                &edit.description,
                &edit.buf,
            ),
            field_line(
                2,
                edit.field,
                edit.editing,
                "System prompt",
                &edit.system_prompt,
                &edit.buf,
            ),
        ],
        EditPage::Tools => vec![field_line(
            0,
            edit.field,
            edit.editing,
            "MCP server IDs (comma-separated)",
            &edit.mcp_server_ids,
            &edit.buf,
        )],
        EditPage::Skills => vec![field_line(
            0,
            edit.field,
            edit.editing,
            "Disabled skills (comma-separated exclusion list)",
            &edit.disabled_skills,
            &edit.buf,
        )],
    };
    frame.render_widget(
        Paragraph::new(lines.join("\n")).wrap(Wrap { trim: false }),
        chunks[1],
    );
    let footer = screen
        .notice
        .as_deref()
        .unwrap_or("Enter edit · Ctrl+S save · Ctrl+←/→ pages · Esc back");
    frame.render_widget(Paragraph::new(format!(" {footer} ")), chunks[2]);
}

fn field_line(
    index: usize,
    cursor: usize,
    editing: bool,
    label: &str,
    value: &str,
    buf: &str,
) -> String {
    let prefix = if index == cursor { ">" } else { " " };
    let shown = if editing && index == cursor {
        format!("[{buf}]")
    } else if value.is_empty() {
        "(empty)".into()
    } else {
        value.to_string()
    };
    format!("{prefix} {label}: {shown}")
}

fn draw_confirm(frame: &mut Frame<'_>, screen: &AssistantsScreen) {
    let id = screen.confirm_id.as_deref().unwrap_or("?");
    let text = format!("Delete assistant {id}?\n\n[y] Yes   [n]/Esc Cancel");
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
    use crate::entity::assistant::Model as AssistantModel;
    use crate::repositories::error::DbError;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[test]
    fn parse_assistants_commands() {
        assert_eq!(
            parse_assistants_command("/assistants"),
            Some(AssistantsCommand::List { query: None })
        );
        assert_eq!(
            parse_assistants_command("/assistants new"),
            Some(AssistantsCommand::New)
        );
        assert_eq!(
            parse_assistants_command("/assistants edit abc"),
            Some(AssistantsCommand::Edit { id: "abc".into() })
        );
        assert_eq!(
            parse_assistants_command("/assistants delete abc"),
            Some(AssistantsCommand::Delete { id: "abc".into() })
        );
        assert_eq!(
            parse_assistants_command("/assistants search foo bar"),
            Some(AssistantsCommand::List {
                query: Some("foo bar".into())
            })
        );
        assert_eq!(
            parse_assistants_command("/assistants foo"),
            Some(AssistantsCommand::List {
                query: Some("foo".into())
            })
        );
        assert_eq!(parse_assistants_command("/new"), None);
    }

    #[test]
    fn gui_shaped_config_round_trips_without_loss() {
        let baseline = json!({
            "description": "d0",
            "systemPrompt": "p0",
            "mcpServerIds": ["srv-a", "srv-b"],
            "disabledSkills": ["skill-x"],
            "allowedBuiltInServiceAliases": ["browser"],
            "deletionProtected": false
        });
        let (mcp_csv, disabled_csv) = read_tools_skills_csv(&baseline);
        assert_eq!(mcp_csv, "srv-a, srv-b");
        assert_eq!(disabled_csv, "skill-x");
        let rebuilt =
            build_assistant_config(&baseline, "d1", "p1", &mcp_csv, &disabled_csv).expect("build");
        assert_eq!(
            rebuilt.get("mcpServerIds"),
            Some(&json!(["srv-a", "srv-b"]))
        );
        assert_eq!(rebuilt.get("disabledSkills"), Some(&json!(["skill-x"])));
        assert_eq!(
            rebuilt.get("allowedBuiltInServiceAliases"),
            Some(&json!(["browser"]))
        );
        assert!(rebuilt.get("mcpServers").is_none());
        assert!(rebuilt.get("skills").is_none());
    }

    #[test]
    fn open_without_repo_reports_not_ready() {
        let err = match AssistantsScreen::open_with_repo(None, None) {
            Ok(_) => panic!("expected not-ready error"),
            Err(e) => e,
        };
        assert!(
            err.contains("not ready"),
            "attached TUI without local repos must fail clearly: {err}"
        );
    }

    struct MemAssistantRepo {
        rows: Mutex<HashMap<String, AssistantModel>>,
    }

    impl MemAssistantRepo {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                rows: Mutex::new(HashMap::new()),
            })
        }

        fn seed(self: &Arc<Self>, id: &str, name: &str, config: Value) {
            let now = 1;
            self.rows.lock().unwrap().insert(
                id.to_string(),
                AssistantModel {
                    id: id.to_string(),
                    name: name.to_string(),
                    config: config.to_string(),
                    created_at: now,
                    updated_at: now,
                },
            );
        }
    }

    #[async_trait::async_trait]
    impl AssistantRepository for MemAssistantRepo {
        async fn create_assistant(
            &self,
            id: String,
            name: String,
            config: String,
        ) -> Result<AssistantModel, DbError> {
            let now = 2;
            let model = AssistantModel {
                id: id.clone(),
                name,
                config,
                created_at: now,
                updated_at: now,
            };
            self.rows.lock().unwrap().insert(id, model.clone());
            Ok(model)
        }

        async fn get_assistant(&self, id: &str) -> Result<Option<AssistantModel>, DbError> {
            Ok(self.rows.lock().unwrap().get(id).cloned())
        }

        async fn list_assistants(&self) -> Result<Vec<AssistantModel>, DbError> {
            Ok(self.rows.lock().unwrap().values().cloned().collect())
        }

        async fn list_assistants_paginated(
            &self,
            _limit: u64,
            _offset: u64,
        ) -> Result<Vec<AssistantModel>, DbError> {
            self.list_assistants().await
        }

        async fn update_assistant(
            &self,
            id: &str,
            name: Option<String>,
            config: Option<String>,
        ) -> Result<AssistantModel, DbError> {
            let mut map = self.rows.lock().unwrap();
            let row = map
                .get_mut(id)
                .ok_or_else(|| DbError::NotFound(format!("assistant {id}")))?;
            if let Some(n) = name {
                row.name = n;
            }
            if let Some(c) = config {
                row.config = c;
            }
            row.updated_at = 3;
            Ok(row.clone())
        }

        async fn delete_assistant(&self, id: &str) -> Result<(), DbError> {
            self.rows.lock().unwrap().remove(id);
            Ok(())
        }

        async fn check_assistant_exists(&self, name: &str) -> Result<bool, DbError> {
            Ok(self.rows.lock().unwrap().values().any(|r| r.name == name))
        }

        async fn search_assistants(&self, _query: &str) -> Result<Vec<AssistantModel>, DbError> {
            self.list_assistants().await
        }

        async fn count_assistants(&self) -> Result<u64, DbError> {
            Ok(self.rows.lock().unwrap().len() as u64)
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    #[test]
    fn create_edit_save_and_reject_empty_name() {
        let repo = MemAssistantRepo::new();
        let mut screen = AssistantsScreen::open_with_repo(
            None,
            Some(repo.clone() as Arc<dyn AssistantRepository>),
        )
        .expect("open");
        screen.begin_new();
        assert!(matches!(screen.save_edit(), Err(e) if e.contains("Name is required")));

        let edit = screen.edit.as_mut().unwrap();
        edit.name = "Demo".into();
        edit.mcp_server_ids = "mcp-1".into();
        edit.disabled_skills = "skip-me".into();
        screen.save_edit().expect("save");
        screen.reload().expect("reload");
        assert_eq!(screen.rows.len(), 1);
        assert_eq!(screen.rows[0].name, "Demo");

        let id = screen.rows[0].id.clone();
        screen.begin_edit(&id).expect("edit");
        let edit = screen.edit.as_mut().unwrap();
        assert_eq!(edit.mcp_server_ids, "mcp-1");
        assert_eq!(edit.disabled_skills, "skip-me");
        edit.description = "updated".into();
        screen.save_edit().expect("update");
        let stored = repo.rows.lock().unwrap().get(&id).unwrap().clone();
        let cfg: Value = serde_json::from_str(&stored.config).unwrap();
        assert_eq!(cfg.get("description"), Some(&json!("updated")));
        assert_eq!(cfg.get("mcpServerIds"), Some(&json!(["mcp-1"])));
        assert_eq!(cfg.get("disabledSkills"), Some(&json!(["skip-me"])));
    }

    #[test]
    fn delete_protected_refused_and_confirm_deletes() {
        let repo = MemAssistantRepo::new();
        repo.seed(
            "prot",
            "Protected",
            json!({ "deletionProtected": true, "description": "x" }),
        );
        repo.seed(
            "free",
            "Free",
            json!({ "deletionProtected": false, "description": "y" }),
        );
        let mut screen = AssistantsScreen::open_with_repo(
            None,
            Some(repo.clone() as Arc<dyn AssistantRepository>),
        )
        .expect("open");

        // Select protected row and press d.
        screen.selected = screen
            .rows
            .iter()
            .position(|r| r.id == "prot")
            .expect("prot");
        screen.handle_key(key(KeyCode::Char('d')));
        assert!(screen.confirm_id.is_none());
        assert!(screen
            .notice
            .as_deref()
            .unwrap_or("")
            .contains("deletion-protected"));

        screen.selected = screen
            .rows
            .iter()
            .position(|r| r.id == "free")
            .expect("free");
        screen.handle_key(key(KeyCode::Char('d')));
        assert_eq!(screen.confirm_id.as_deref(), Some("free"));
        screen.handle_key(key(KeyCode::Char('y')));
        assert!(repo.rows.lock().unwrap().get("free").is_none());
        assert!(repo.rows.lock().unwrap().get("prot").is_some());
    }

    #[test]
    fn list_filter_keeps_matching_rows() {
        let repo = MemAssistantRepo::new();
        repo.seed("a", "Alpha Bot", json!({ "description": "one" }));
        repo.seed("b", "Beta", json!({ "description": "alpha-ish" }));
        let screen = AssistantsScreen::open_with_repo(
            Some("alpha".into()),
            Some(repo as Arc<dyn AssistantRepository>),
        )
        .expect("open");
        assert_eq!(screen.rows.len(), 2);
    }

    #[test]
    fn ctrl_s_saves_from_edit_view() {
        let repo = MemAssistantRepo::new();
        let mut screen =
            AssistantsScreen::open_with_repo(None, Some(repo as Arc<dyn AssistantRepository>))
                .expect("open");
        screen.begin_new();
        screen.edit.as_mut().unwrap().name = "Saved".into();
        screen.handle_key(ctrl(KeyCode::Char('s')));
        assert!(matches!(screen.view, View::List));
        assert_eq!(screen.rows.len(), 1);
    }
}
