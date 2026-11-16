//! TUI `/assistants` library screens (issue #2816 / parity §3.3).

use crate::models::assistant::{AssistantDto, AssistantSummaryDto};
use crate::services::AssistantService;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use serde_json::{json, Value};

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
    /// Comma-separated MCP server ids (Tools page).
    tool_servers: String,
    /// Comma-separated skill names (Skills page).
    skills: String,
    page: EditPage,
    field: usize,
    editing: bool,
    buf: String,
    baseline_config: Value,
}

#[derive(Debug)]
pub struct AssistantsScreen {
    view: View,
    rows: Vec<AssistantSummaryDto>,
    filter: String,
    selected: usize,
    edit: Option<EditForm>,
    confirm_id: Option<String>,
    notice: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssistantsAction {
    None,
    Closed,
}

impl AssistantsScreen {
    pub fn open(query: Option<String>) -> Result<Self, String> {
        let mut screen = Self {
            view: View::List,
            rows: Vec::new(),
            filter: query.unwrap_or_default(),
            selected: 0,
            edit: None,
            confirm_id: None,
            notice: None,
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

    fn reload(&mut self) -> Result<(), String> {
        let repo = crate::state::try_get_assistant_repository()
            .ok_or_else(|| "Assistants not ready yet".to_string())?;
        let models = tauri::async_runtime::block_on(AssistantService::list_assistants(repo))?;
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
        rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
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
            tool_servers: String::new(),
            skills: String::new(),
            page: EditPage::General,
            field: 0,
            editing: false,
            buf: String::new(),
            baseline_config: json!({}),
        });
        self.view = View::Edit;
    }

    fn begin_edit(&mut self, id: &str) -> Result<(), String> {
        let repo = crate::state::try_get_assistant_repository()
            .ok_or_else(|| "Assistants not ready yet".to_string())?;
        let model = tauri::async_runtime::block_on(AssistantService::get_assistant(repo, id))?
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
        let tool_servers = config
            .get("mcpServers")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        let skills = config
            .get("skills")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        self.edit = Some(EditForm {
            id: dto.id,
            is_new: false,
            name: dto.name,
            description,
            system_prompt,
            tool_servers,
            skills,
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
                    (EditPage::Tools, _) => edit.tool_servers.clone(),
                    (EditPage::Skills, _) => edit.skills.clone(),
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
            (EditPage::Tools, _) => edit.tool_servers = edit.buf.clone(),
            (EditPage::Skills, _) => edit.skills = edit.buf.clone(),
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
        let mut config = edit.baseline_config.clone();
        if !config.is_object() {
            config = json!({});
        }
        let obj = config.as_object_mut().expect("object");
        obj.insert("description".into(), json!(edit.description));
        obj.insert("systemPrompt".into(), json!(edit.system_prompt));
        let servers: Vec<Value> = edit
            .tool_servers
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| Value::String(s.to_string()))
            .collect();
        obj.insert("mcpServers".into(), Value::Array(servers));
        let skills: Vec<Value> = edit
            .skills
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| Value::String(s.to_string()))
            .collect();
        obj.insert("skills".into(), Value::Array(skills));

        let repo = crate::state::try_get_assistant_repository()
            .ok_or_else(|| "Assistants not ready yet".to_string())?;
        if edit.is_new {
            tauri::async_runtime::block_on(AssistantService::create_assistant(
                repo,
                edit.id.clone(),
                name.to_string(),
                config,
            ))?;
        } else {
            tauri::async_runtime::block_on(AssistantService::update_assistant(
                repo,
                &edit.id,
                Some(name.to_string()),
                Some(config),
            ))?;
        }
        Ok(())
    }

    fn delete_by_id(&self, id: &str) -> Result<(), String> {
        let repo = crate::state::try_get_assistant_repository()
            .ok_or_else(|| "Assistants not ready yet".to_string())?;
        tauri::async_runtime::block_on(AssistantService::delete_assistant(repo, id))
    }
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
            "MCP servers (comma-separated)",
            &edit.tool_servers,
            &edit.buf,
        )],
        EditPage::Skills => vec![field_line(
            0,
            edit.field,
            edit.editing,
            "Skills (comma-separated)",
            &edit.skills,
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
}
