//! TUI `/extensions` screens (issue #2817 / parity §3.3).

use crate::entity::mcp_server::Model as McpServerModel;
use crate::mcp::types::OAuthConfig;
use crate::repositories::MCPServerRepository;
use crate::services::mcp_server_service::{McpServerService, SaveVerification};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionsCommand {
    List { query: Option<String> },
    New,
    Edit { name: String },
    Delete { name: String },
    Enable { name: String },
    Disable { name: String },
    Tools { name: String },
    Retry { name: Option<String> },
    Auth { name: String },
    Logout { name: String },
    Presets,
    AppWizard,
}

pub fn parse_extensions_command(text: &str) -> Option<ExtensionsCommand> {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') || trimmed.starts_with("//") {
        return None;
    }
    let mut parts = trimmed[1..].split_whitespace();
    let verb = parts.next()?.to_ascii_lowercase();
    if verb != "extensions" {
        return None;
    }
    match parts.next().map(|s| s.to_ascii_lowercase()) {
        None => Some(ExtensionsCommand::List { query: None }),
        Some(cmd) => match cmd.as_str() {
            "search" => {
                let q = parts.collect::<Vec<_>>().join(" ");
                Some(ExtensionsCommand::List {
                    query: if q.is_empty() { None } else { Some(q) },
                })
            }
            "new" => Some(ExtensionsCommand::New),
            "edit" => Some(ExtensionsCommand::Edit {
                name: parts.next()?.to_string(),
            }),
            "delete" => Some(ExtensionsCommand::Delete {
                name: parts.next()?.to_string(),
            }),
            "enable" => Some(ExtensionsCommand::Enable {
                name: parts.next()?.to_string(),
            }),
            "disable" => Some(ExtensionsCommand::Disable {
                name: parts.next()?.to_string(),
            }),
            "tools" => Some(ExtensionsCommand::Tools {
                name: parts.next()?.to_string(),
            }),
            "retry" => Some(ExtensionsCommand::Retry {
                name: parts.next().map(str::to_string),
            }),
            "auth" => Some(ExtensionsCommand::Auth {
                name: parts.next()?.to_string(),
            }),
            "logout" => Some(ExtensionsCommand::Logout {
                name: parts.next()?.to_string(),
            }),
            "presets" => Some(ExtensionsCommand::Presets),
            "app-wizard" => Some(ExtensionsCommand::AppWizard),
            other => {
                let mut rest = vec![other.to_string()];
                rest.extend(parts.map(str::to_string));
                Some(ExtensionsCommand::List {
                    query: Some(rest.join(" ")),
                })
            }
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    List,
    Tools,
    Presets,
    ConfirmDelete,
    ConfirmWizard,
    Form,
    Notice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormField {
    Name,
    Transport,
    CommandOrUrl,
    Args,
}

#[derive(Debug)]
pub struct FormState {
    pub editing_id: Option<String>,
    pub name: String,
    /// `stdio` or `http`
    pub transport: String,
    pub command_or_url: String,
    pub args: String,
    pub field: FormField,
}

pub struct BackgroundOp {
    pub label: String,
    finished: std::sync::Arc<std::sync::atomic::AtomicBool>,
    outcome: std::sync::Arc<std::sync::Mutex<Option<Result<String, String>>>>,
    handle: tauri::async_runtime::JoinHandle<()>,
}

impl BackgroundOp {
    fn spawn(
        label: String,
        fut: impl std::future::Future<Output = Result<String, String>> + Send + 'static,
    ) -> Self {
        let finished = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let outcome = std::sync::Arc::new(std::sync::Mutex::new(None));
        let finished_task = finished.clone();
        let outcome_task = outcome.clone();
        struct FinishGuard(std::sync::Arc<std::sync::atomic::AtomicBool>);
        impl Drop for FinishGuard {
            fn drop(&mut self) {
                self.0.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let handle = tauri::async_runtime::spawn(async move {
            let _guard = FinishGuard(finished_task);
            let result = fut.await;
            if let Ok(mut slot) = outcome_task.lock() {
                *slot = Some(result);
            }
        });
        Self {
            label,
            finished,
            outcome,
            handle,
        }
    }

    fn is_finished(&self) -> bool {
        self.finished.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn abort(&self) {
        self.handle.abort();
    }

    fn take_outcome(&self) -> Option<Result<String, String>> {
        self.outcome.lock().ok().and_then(|mut slot| slot.take())
    }
}

impl std::fmt::Debug for BackgroundOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackgroundOp")
            .field("label", &self.label)
            .field("finished", &self.is_finished())
            .finish()
    }
}

#[derive(Debug)]
pub struct ExtensionsScreen {
    view: View,
    rows: Vec<McpServerModel>,
    filter: String,
    selected: usize,
    tools_title: String,
    tools_lines: Vec<String>,
    tools_scroll: usize,
    presets: Vec<(String, String)>,
    confirm_name: Option<String>,
    form: Option<FormState>,
    pub background_op: Option<BackgroundOp>,
    notice: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionsAction {
    None,
    Closed,
}

pub fn resolve_server_by_name(
    repo: &dyn MCPServerRepository,
    name: &str,
) -> Result<McpServerModel, String> {
    let rows = tauri::async_runtime::block_on(McpServerService::list_server_configs(repo))?;
    rows.into_iter()
        .find(|r| r.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("Extension not found: {name}"))
}

pub fn select_failed_servers(
    runtime: &crate::agent::runtime_state::SessionRuntimeState,
) -> Vec<String> {
    runtime
        .servers
        .iter()
        .filter(|s| s.status == crate::agent::runtime_state::SessionRuntimeServerStatus::Failed)
        .map(|s| s.name.clone())
        .collect()
}

fn is_active(config_json: &str) -> bool {
    serde_json::from_str::<Value>(config_json)
        .ok()
        .and_then(|v| v.get("isActive").and_then(Value::as_bool))
        .unwrap_or(true)
}

fn set_active(config_json: &str, active: bool) -> Result<Value, String> {
    let mut value: Value = serde_json::from_str(config_json).unwrap_or_else(|_| json!({}));
    if !value.is_object() {
        value = json!({});
    }
    value
        .as_object_mut()
        .ok_or_else(|| "Invalid MCP config JSON".to_string())?
        .insert("isActive".into(), Value::Bool(active));
    Ok(value)
}

pub fn build_config(form: &FormState) -> Result<Value, String> {
    let name = form.name.trim();
    if name.is_empty() {
        return Err("Name is required".into());
    }
    let transport = form.transport.trim().to_ascii_lowercase();
    let target = form.command_or_url.trim();
    if target.is_empty() {
        return Err(if transport == "http" {
            "URL is required".into()
        } else {
            "Command is required".into()
        });
    }
    let transport_value = if transport == "http" {
        json!({
            "type": "http",
            "url": target,
        })
    } else {
        let args: Vec<String> = form.args.split_whitespace().map(str::to_string).collect();
        json!({
            "type": "stdio",
            "command": target,
            "args": args,
        })
    };
    Ok(json!({
        "name": name,
        "transport": transport_value,
        "isActive": true,
    }))
}

impl ExtensionsScreen {
    pub fn open(query: Option<String>) -> Result<Self, String> {
        let mut screen = Self {
            view: View::List,
            rows: Vec::new(),
            filter: query.unwrap_or_default(),
            selected: 0,
            tools_title: String::new(),
            tools_lines: Vec::new(),
            tools_scroll: 0,
            presets: Vec::new(),
            confirm_name: None,
            form: None,
            background_op: None,
            notice: None,
        };
        screen.reload()?;
        Ok(screen)
    }

    pub fn open_new() -> Result<Self, String> {
        let mut screen = Self::open(None)?;
        screen.form = Some(FormState {
            editing_id: None,
            name: String::new(),
            transport: "stdio".into(),
            command_or_url: String::new(),
            args: String::new(),
            field: FormField::Name,
        });
        screen.view = View::Form;
        Ok(screen)
    }

    pub fn open_edit(name: &str) -> Result<Self, String> {
        let mut screen = Self::open(None)?;
        let row = screen.resolve(name)?.clone();
        let config: Value = serde_json::from_str(&row.config).unwrap_or_else(|_| json!({}));
        let transport = config
            .get("transport")
            .and_then(|t| t.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("stdio")
            .to_string();
        let (command_or_url, args) = if transport == "http" {
            (
                config
                    .pointer("/transport/url")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                String::new(),
            )
        } else {
            let cmd = config
                .pointer("/transport/command")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let args = config
                .pointer("/transport/args")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();
            (cmd, args)
        };
        screen.form = Some(FormState {
            editing_id: Some(row.id.clone()),
            name: row.name.clone(),
            transport,
            command_or_url,
            args,
            field: FormField::Name,
        });
        screen.view = View::Form;
        Ok(screen)
    }

    pub fn open_presets() -> Self {
        let presets = crate::mcp::presets::get_recommended_servers()
            .into_iter()
            .map(|p| (p.name.clone(), p.description.clone().unwrap_or_default()))
            .collect();
        Self {
            view: View::Presets,
            rows: Vec::new(),
            filter: String::new(),
            selected: 0,
            tools_title: "Presets".into(),
            tools_lines: Vec::new(),
            tools_scroll: 0,
            presets,
            confirm_name: None,
            form: None,
            background_op: None,
            notice: Some("Enter installs the highlighted preset · Esc closes".into()),
        }
    }

    pub fn open_app_wizard() -> Self {
        Self {
            view: View::ConfirmWizard,
            rows: Vec::new(),
            filter: String::new(),
            selected: 0,
            tools_title: String::new(),
            tools_lines: Vec::new(),
            tools_scroll: 0,
            presets: Vec::new(),
            confirm_name: None,
            form: None,
            background_op: None,
            notice: None,
        }
    }

    pub fn open_notice(message: &str) -> Self {
        Self {
            view: View::Notice,
            rows: Vec::new(),
            filter: String::new(),
            selected: 0,
            tools_title: String::new(),
            tools_lines: Vec::new(),
            tools_scroll: 0,
            presets: Vec::new(),
            confirm_name: None,
            form: None,
            background_op: None,
            notice: Some(message.to_string()),
        }
    }

    pub fn open_delete(name: &str) -> Result<Self, String> {
        let mut screen = Self::open(None)?;
        screen.resolve(name)?;
        screen.confirm_name = Some(name.to_string());
        screen.view = View::ConfirmDelete;
        Ok(screen)
    }

    pub fn apply_enable(name: &str, enable: bool) -> Result<String, String> {
        let repo = crate::state::try_get_mcp_server_repository()
            .ok_or_else(|| "Extensions require the primary backend process (not available in attached TUI mode)".to_string())?;
        let row = resolve_server_by_name(repo, name)?;
        let config = set_active(&row.config, enable)?;
        tauri::async_runtime::block_on(McpServerService::update_server_config(
            repo,
            row.id,
            None,
            Some(config),
            SaveVerification::Unverified,
        ))?;
        Ok(format!(
            "{} {}",
            if enable { "Enabled" } else { "Disabled" },
            name
        ))
    }

    pub fn apply_logout(name: &str) -> Result<String, String> {
        let repo = crate::state::try_get_mcp_server_repository()
            .ok_or_else(|| "Extensions require the primary backend process (not available in attached TUI mode)".to_string())?;
        let row = resolve_server_by_name(repo, name)?;
        tauri::async_runtime::block_on(crate::commands::mcp_commands::revoke_oauth_token(row.id))
    }

    pub fn start_probe(&mut self, name: &str) -> Result<(), String> {
        let repo = crate::state::try_get_mcp_server_repository().ok_or_else(|| {
            "Extensions require the primary backend process (not available in attached TUI mode)"
                .to_string()
        })?;
        let row = resolve_server_by_name(repo, name)?;
        let row_id = row.id.clone();
        let name_clone = name.to_string();
        self.background_op = Some(BackgroundOp::spawn(
            format!("Probing {name}"),
            async move {
                let tools = McpServerService::probe_server(repo, &row_id).await?;
                Ok(format!("Verified {name_clone} · {} tool(s)", tools.len()))
            },
        ));
        Ok(())
    }

    pub fn start_auth(&mut self, name: &str) -> Result<(), String> {
        let repo = crate::state::try_get_mcp_server_repository().ok_or_else(|| {
            "Extensions require the primary backend process (not available in attached TUI mode)"
                .to_string()
        })?;
        let row = resolve_server_by_name(repo, name)?;
        let config: Value = serde_json::from_str(&row.config)
            .map_err(|e| format!("Invalid config for {name}: {e}"))?;
        let auth: OAuthConfig = serde_json::from_value(
            config
                .get("authentication")
                .cloned()
                .ok_or_else(|| format!("{name} has no OAuth authentication config"))?,
        )
        .map_err(|e| format!("Invalid OAuth config: {e}"))?;
        let app_handle = crate::state::get_app_handle()
            .ok_or_else(|| "App handle not ready for OAuth browser open".to_string())?
            .clone();
        let row_id = row.id.clone();
        self.background_op = Some(BackgroundOp::spawn(
            format!("Authenticating {name} in browser"),
            async move {
                crate::commands::mcp_commands::start_oauth_flow(app_handle, row_id, auth).await
            },
        ));
        Ok(())
    }

    pub fn start_session_retry(&mut self, session_id: &str) -> Result<(), String> {
        // Avoid get_mcp_service_proxy_manager()'s panic when the manager is unset
        // (unit tests / attached TUI before boot).
        if crate::state::try_get_mcp_server_repository().is_none() {
            return Err(
                "Extensions require the primary backend process (not available in attached TUI mode)"
                    .into(),
            );
        }
        let mgr = crate::state::get_mcp_service_proxy_manager();
        let session_id = session_id.to_string();
        self.background_op = Some(BackgroundOp::spawn(
            "Retrying failed session servers".into(),
            async move {
                let runtime = mgr.get_runtime_state(&session_id).await;
                let failed = select_failed_servers(&runtime);
                if failed.is_empty() {
                    return Ok("No failed session servers to retry".into());
                }
                let app_handle = crate::state::get_app_handle();
                for name in &failed {
                    mgr.retry_failed_server(&session_id, name, app_handle)
                        .await?;
                }
                Ok(format!("Retried {} failed server(s)", failed.len()))
            },
        ));
        Ok(())
    }

    pub fn poll_background(&mut self) {
        if let Some(op) = &self.background_op {
            if !op.is_finished() {
                return;
            }
            let op = self.background_op.take().unwrap();
            match op.take_outcome() {
                Some(Ok(msg)) => {
                    self.notice = Some(msg);
                    let _ = self.reload();
                }
                Some(Err(err)) => {
                    self.notice = Some(err);
                }
                None => {
                    self.notice = Some("Operation cancelled".into());
                }
            }
        }
    }

    pub fn open_tools(name: &str) -> Result<Self, String> {
        let mut screen = Self::open(None)?;
        let row = screen.resolve(name)?.clone();
        let lines = row
            .cached_tools
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Vec<Value>>(raw).ok())
            .map(|tools| {
                tools
                    .into_iter()
                    .map(|t| {
                        let n = t.get("name").and_then(Value::as_str).unwrap_or("?");
                        let d = t.get("description").and_then(Value::as_str).unwrap_or("");
                        format!("{n} — {d}")
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        screen.tools_title = format!("Tools · {name}");
        screen.tools_lines = if lines.is_empty() {
            vec!["(no cached tools — run /extensions retry <name>)".into()]
        } else {
            lines
        };
        screen.view = View::Tools;
        Ok(screen)
    }

    fn reload(&mut self) -> Result<(), String> {
        let repo = crate::state::try_get_mcp_server_repository()
            .ok_or_else(|| "Extensions require the primary backend process (not available in attached TUI mode)".to_string())?;
        let mut rows = tauri::async_runtime::block_on(McpServerService::list_server_configs(repo))?;
        if !self.filter.is_empty() {
            let q = self.filter.to_lowercase();
            rows.retain(|r| r.name.to_lowercase().contains(&q));
        }
        rows.sort_by_key(|a| a.name.to_lowercase());
        self.rows = rows;
        if self.selected >= self.rows.len() {
            self.selected = self.rows.len().saturating_sub(1);
        }
        Ok(())
    }

    fn resolve(&self, name: &str) -> Result<&McpServerModel, String> {
        self.rows
            .iter()
            .find(|r| r.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| format!("Extension not found: {name}"))
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ExtensionsAction {
        if let Some(op) = &self.background_op {
            if key.code == KeyCode::Esc {
                op.abort();
                self.background_op = None;
                self.notice = Some("Operation cancelled".into());
                return ExtensionsAction::None;
            }
            return ExtensionsAction::None;
        }

        match self.view {
            View::Notice => match key.code {
                KeyCode::Esc | KeyCode::Enter => ExtensionsAction::Closed,
                _ => ExtensionsAction::None,
            },
            View::Presets => self.handle_presets(key),
            View::Tools => match key.code {
                KeyCode::Esc => {
                    self.view = View::List;
                    ExtensionsAction::None
                }
                KeyCode::Up => {
                    self.tools_scroll = self.tools_scroll.saturating_sub(1);
                    ExtensionsAction::None
                }
                KeyCode::Down => {
                    self.tools_scroll = self.tools_scroll.saturating_add(1);
                    ExtensionsAction::None
                }
                _ => ExtensionsAction::None,
            },
            View::ConfirmDelete => self.handle_confirm_delete(key),
            View::ConfirmWizard => self.handle_wizard(key),
            View::Form => self.handle_form(key),
            View::List => self.handle_list(key),
        }
    }

    fn handle_presets(&mut self, key: KeyEvent) -> ExtensionsAction {
        match key.code {
            KeyCode::Esc => ExtensionsAction::Closed,
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                ExtensionsAction::None
            }
            KeyCode::Down => {
                if self.selected + 1 < self.presets.len() {
                    self.selected += 1;
                }
                ExtensionsAction::None
            }
            KeyCode::Enter => {
                if let Some((name, _)) = self.presets.get(self.selected).cloned() {
                    match install_preset(&name) {
                        Ok(msg) => self.notice = Some(msg),
                        Err(err) => self.notice = Some(err),
                    }
                }
                ExtensionsAction::None
            }
            _ => ExtensionsAction::None,
        }
    }

    fn handle_confirm_delete(&mut self, key: KeyEvent) -> ExtensionsAction {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(name) = self.confirm_name.take() {
                    match self.delete_named(&name) {
                        Ok(()) => {
                            self.notice = Some(format!("Deleted {name}"));
                            let _ = self.reload();
                        }
                        Err(err) => self.notice = Some(err),
                    }
                }
                self.view = View::List;
                ExtensionsAction::None
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.confirm_name = None;
                self.view = View::List;
                self.notice = Some("Delete cancelled".into());
                ExtensionsAction::None
            }
            _ => ExtensionsAction::None,
        }
    }

    fn handle_wizard(&mut self, key: KeyEvent) -> ExtensionsAction {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let mut messages = Vec::new();
                for name in ["hn", "yahoo-finance"] {
                    match install_preset(name) {
                        Ok(msg) => messages.push(msg),
                        Err(err) => messages.push(format!("{name}: {err}")),
                    }
                }
                *self = Self::open_notice(&format!(
                    "App wizard complete.\n{}\n\nCreate an assistant and a 09:00 briefing task in the GUI if needed.",
                    messages.join("\n")
                ));
                ExtensionsAction::None
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => ExtensionsAction::Closed,
            _ => ExtensionsAction::None,
        }
    }

    fn handle_form(&mut self, key: KeyEvent) -> ExtensionsAction {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && matches!(key.code, KeyCode::Char('s') | KeyCode::Char('S')) {
            return self.save_form();
        }
        match key.code {
            KeyCode::Esc => {
                self.form = None;
                self.view = View::List;
                ExtensionsAction::None
            }
            KeyCode::Tab | KeyCode::Down => {
                if let Some(form) = self.form.as_mut() {
                    form.field = match form.field {
                        FormField::Name => FormField::Transport,
                        FormField::Transport => FormField::CommandOrUrl,
                        FormField::CommandOrUrl => FormField::Args,
                        FormField::Args => FormField::Name,
                    };
                }
                ExtensionsAction::None
            }
            KeyCode::BackTab | KeyCode::Up => {
                if let Some(form) = self.form.as_mut() {
                    form.field = match form.field {
                        FormField::Name => FormField::Args,
                        FormField::Transport => FormField::Name,
                        FormField::CommandOrUrl => FormField::Transport,
                        FormField::Args => FormField::CommandOrUrl,
                    };
                }
                ExtensionsAction::None
            }
            KeyCode::Enter => self.save_form(),
            KeyCode::Backspace => {
                if let Some(form) = self.form.as_mut() {
                    match form.field {
                        FormField::Name => {
                            form.name.pop();
                        }
                        FormField::Transport => {
                            form.transport.pop();
                        }
                        FormField::CommandOrUrl => {
                            form.command_or_url.pop();
                        }
                        FormField::Args => {
                            form.args.pop();
                        }
                    }
                }
                ExtensionsAction::None
            }
            KeyCode::Char(ch) if !ctrl => {
                if let Some(form) = self.form.as_mut() {
                    match form.field {
                        FormField::Name => form.name.push(ch),
                        FormField::Transport => form.transport.push(ch),
                        FormField::CommandOrUrl => form.command_or_url.push(ch),
                        FormField::Args => form.args.push(ch),
                    }
                }
                ExtensionsAction::None
            }
            _ => ExtensionsAction::None,
        }
    }

    fn save_form(&mut self) -> ExtensionsAction {
        let Some(form) = self.form.as_ref() else {
            return ExtensionsAction::None;
        };
        let config = match build_config(form) {
            Ok(c) => c,
            Err(err) => {
                self.notice = Some(err);
                return ExtensionsAction::None;
            }
        };
        let repo = match crate::state::try_get_mcp_server_repository() {
            Some(r) => r,
            None => {
                self.notice = Some("Extensions require the primary backend process (not available in attached TUI mode)".into());
                return ExtensionsAction::None;
            }
        };
        let result = if let Some(id) = form.editing_id.clone() {
            tauri::async_runtime::block_on(McpServerService::update_server_config(
                repo,
                id.clone(),
                Some(form.name.trim().to_string()),
                Some(config),
                SaveVerification::Pending,
            ))
            .map(|m| {
                McpServerService::schedule_background_probe(m.id.clone());
                format!("Updated {}", m.name)
            })
        } else {
            tauri::async_runtime::block_on(McpServerService::create_server_config(
                repo,
                form.name.trim().to_string(),
                config,
                SaveVerification::Pending,
            ))
            .map(|m| {
                McpServerService::schedule_background_probe(m.id.clone());
                format!("Created {}", m.name)
            })
        };
        match result {
            Ok(msg) => {
                self.form = None;
                self.view = View::List;
                self.notice = Some(msg);
                let _ = self.reload();
            }
            Err(err) => self.notice = Some(err),
        }
        ExtensionsAction::None
    }

    fn handle_list(&mut self, key: KeyEvent) -> ExtensionsAction {
        match key.code {
            KeyCode::Esc => ExtensionsAction::Closed,
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                ExtensionsAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected + 1 < self.rows.len() {
                    self.selected += 1;
                }
                ExtensionsAction::None
            }
            KeyCode::Char('e') => {
                if let Some(row) = self.rows.get(self.selected) {
                    let name = row.name.clone();
                    let enable = !is_active(&row.config);
                    match Self::apply_enable(&name, enable) {
                        Ok(msg) => {
                            self.notice = Some(msg);
                            let _ = self.reload();
                        }
                        Err(err) => self.notice = Some(err),
                    }
                }
                ExtensionsAction::None
            }
            KeyCode::Char('t') | KeyCode::Enter => {
                if let Some(row) = self.rows.get(self.selected) {
                    let name = row.name.clone();
                    match Self::open_tools(&name) {
                        Ok(next) => *self = next,
                        Err(err) => self.notice = Some(err),
                    }
                }
                ExtensionsAction::None
            }
            KeyCode::Char('r') => {
                if let Some(row) = self.rows.get(self.selected) {
                    let name = row.name.clone();
                    if let Err(err) = self.start_probe(&name) {
                        self.notice = Some(err);
                    }
                }
                ExtensionsAction::None
            }
            KeyCode::Char('d') | KeyCode::Delete => {
                if let Some(row) = self.rows.get(self.selected) {
                    self.confirm_name = Some(row.name.clone());
                    self.view = View::ConfirmDelete;
                }
                ExtensionsAction::None
            }
            _ => ExtensionsAction::None,
        }
    }

    fn delete_named(&self, name: &str) -> Result<(), String> {
        let repo = crate::state::try_get_mcp_server_repository()
            .ok_or_else(|| "Extensions require the primary backend process (not available in attached TUI mode)".to_string())?;
        let row = self.resolve(name)?;
        tauri::async_runtime::block_on(McpServerService::delete_server_config(repo, &row.id))
    }
}

fn install_preset(name: &str) -> Result<String, String> {
    let preset = crate::mcp::presets::get_recommended_servers()
        .into_iter()
        .find(|p| p.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("Preset not found: {name}"))?;
    let repo = crate::state::try_get_mcp_server_repository().ok_or_else(|| {
        "Extensions require the primary backend process (not available in attached TUI mode)"
            .to_string()
    })?;
    let mut config = json!({
        "name": preset.name,
        "transport": if preset.transport_type == "sse" || preset.url.is_some() {
            json!({
                "type": "http",
                "url": preset.url.clone().unwrap_or_default(),
            })
        } else {
            json!({
                "type": "stdio",
                "command": preset.command.clone().unwrap_or_default(),
                "args": preset.args.clone().unwrap_or_default(),
            })
        },
        "isActive": true,
    });
    if let Some(auth) = preset.authentication {
        if let Ok(v) = serde_json::to_value(auth) {
            config
                .as_object_mut()
                .ok_or_else(|| "config object".to_string())?
                .insert("authentication".into(), v);
        }
    }
    let model = tauri::async_runtime::block_on(McpServerService::create_server_config(
        repo,
        preset.name.clone(),
        config,
        SaveVerification::Pending,
    ))?;
    McpServerService::schedule_background_probe(model.id.clone());
    Ok(format!("Installed preset {}", model.name))
}

pub fn draw_extensions(frame: &mut Frame<'_>, screen: &ExtensionsScreen) {
    match screen.view {
        View::Notice => {
            let text = screen.notice.as_deref().unwrap_or("");
            frame.render_widget(
                Paragraph::new(format!("{text}\n\nPress Esc / Enter"))
                    .block(Block::default().borders(Borders::ALL).title(" Extensions "))
                    .wrap(Wrap { trim: false }),
                frame.area(),
            );
        }
        View::ConfirmWizard => {
            frame.render_widget(
                Paragraph::new(
                    "App Wizard\n\nInstall Hacker News and Yahoo Finance extensions?\n\n[y] Yes  [n]/Esc Cancel",
                )
                .block(Block::default().borders(Borders::ALL).title(" App Wizard ")),
                frame.area(),
            );
        }
        View::Presets => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Min(3),
                    Constraint::Length(2),
                ])
                .split(frame.area());
            frame.render_widget(Paragraph::new(" Extension presets "), chunks[0]);
            let items: Vec<ListItem> = screen
                .presets
                .iter()
                .enumerate()
                .map(|(i, (name, desc))| {
                    let prefix = if i == screen.selected { ">" } else { " " };
                    ListItem::new(format!("{prefix} {name} — {desc}"))
                })
                .collect();
            frame.render_widget(
                List::new(items).block(Block::default().borders(Borders::ALL)),
                chunks[1],
            );
            let footer = screen
                .notice
                .as_deref()
                .unwrap_or("Enter install · Esc close");
            frame.render_widget(Paragraph::new(format!(" {footer} ")), chunks[2]);
        }
        View::Tools => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Min(3),
                    Constraint::Length(1),
                ])
                .split(frame.area());
            frame.render_widget(
                Paragraph::new(format!(" {} ", screen.tools_title)),
                chunks[0],
            );
            let body = screen
                .tools_lines
                .iter()
                .skip(screen.tools_scroll)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n");
            frame.render_widget(
                Paragraph::new(body)
                    .wrap(Wrap { trim: false })
                    .block(Block::default().borders(Borders::ALL)),
                chunks[1],
            );
            frame.render_widget(Paragraph::new(" ↑/↓ scroll · Esc back "), chunks[2]);
        }
        View::ConfirmDelete => {
            let name = screen.confirm_name.as_deref().unwrap_or("?");
            frame.render_widget(
                Paragraph::new(format!(
                    "Delete extension {name}?\n\n[y] Yes  [n]/Esc Cancel"
                ))
                .block(Block::default().borders(Borders::ALL).title(" Confirm ")),
                frame.area(),
            );
        }
        View::Form => {
            let form = screen.form.as_ref();
            let mark = |field: FormField| {
                if form.map(|f| f.field) == Some(field) {
                    ">"
                } else {
                    " "
                }
            };
            let name = form.map(|f| f.name.as_str()).unwrap_or("");
            let transport = form.map(|f| f.transport.as_str()).unwrap_or("stdio");
            let command = form.map(|f| f.command_or_url.as_str()).unwrap_or("");
            let args = form.map(|f| f.args.as_str()).unwrap_or("");
            let title = if form.and_then(|f| f.editing_id.as_ref()).is_some() {
                " Edit extension "
            } else {
                " New extension "
            };
            let body = format!(
                "{m0} Name: {name}\n{m1} Transport (stdio|http): {transport}\n{m2} Command/URL: {command}\n{m3} Args: {args}\n\nTab fields · Ctrl+S / Enter save · Esc cancel",
                m0 = mark(FormField::Name),
                m1 = mark(FormField::Transport),
                m2 = mark(FormField::CommandOrUrl),
                m3 = mark(FormField::Args),
            );
            let notice = screen
                .notice
                .as_ref()
                .map(|n| format!("\n\n{n}"))
                .unwrap_or_default();
            frame.render_widget(
                Paragraph::new(format!("{body}{notice}"))
                    .block(Block::default().borders(Borders::ALL).title(title))
                    .wrap(Wrap { trim: false }),
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
            let title = if screen.filter.is_empty() {
                " Extensions ".into()
            } else {
                format!(" Extensions · filter: {} ", screen.filter)
            };
            frame.render_widget(Paragraph::new(title), chunks[0]);
            let items: Vec<ListItem> = screen
                .rows
                .iter()
                .enumerate()
                .map(|(i, row)| {
                    let prefix = if i == screen.selected { ">" } else { " " };
                    let active = if is_active(&row.config) { "on" } else { "off" };
                    let status = row.verification_status.as_deref().unwrap_or("-");
                    let tools = row.tool_count.unwrap_or(0);
                    ListItem::new(format!(
                        "{prefix} {} [{active}] ver={status} tools={tools}",
                        row.name
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
                    .unwrap_or("↑/↓ · Enter/t tools · e enable/disable · r retry · d delete · Esc")
            };
            frame.render_widget(
                Paragraph::new(format!(" {footer} ")).wrap(Wrap { trim: true }),
                chunks[2],
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_extensions_commands() {
        assert_eq!(
            parse_extensions_command("/extensions"),
            Some(ExtensionsCommand::List { query: None })
        );
        assert_eq!(
            parse_extensions_command("/extensions enable foo"),
            Some(ExtensionsCommand::Enable { name: "foo".into() })
        );
        assert_eq!(
            parse_extensions_command("/extensions retry"),
            Some(ExtensionsCommand::Retry { name: None })
        );
        assert_eq!(
            parse_extensions_command("/extensions retry bar"),
            Some(ExtensionsCommand::Retry {
                name: Some("bar".into())
            })
        );
        assert_eq!(
            parse_extensions_command("/extensions presets"),
            Some(ExtensionsCommand::Presets)
        );
        assert_eq!(
            parse_extensions_command("/extensions app-wizard"),
            Some(ExtensionsCommand::AppWizard)
        );
    }

    #[test]
    fn build_config_validation() {
        // Missing name
        let form_empty_name = FormState {
            editing_id: None,
            name: "   ".into(),
            transport: "stdio".into(),
            command_or_url: "cat".into(),
            args: "".into(),
            field: FormField::Name,
        };
        assert_eq!(
            build_config(&form_empty_name).unwrap_err(),
            "Name is required"
        );

        // Missing command for stdio
        let form_empty_cmd = FormState {
            editing_id: None,
            name: "srv".into(),
            transport: "stdio".into(),
            command_or_url: "".into(),
            args: "".into(),
            field: FormField::CommandOrUrl,
        };
        assert_eq!(
            build_config(&form_empty_cmd).unwrap_err(),
            "Command is required"
        );

        // Missing url for http
        let form_empty_url = FormState {
            editing_id: None,
            name: "srv".into(),
            transport: "http".into(),
            command_or_url: "".into(),
            args: "".into(),
            field: FormField::CommandOrUrl,
        };
        assert_eq!(
            build_config(&form_empty_url).unwrap_err(),
            "URL is required"
        );

        // Valid stdio with multiple arguments
        let form_stdio = FormState {
            editing_id: None,
            name: "my_tool".into(),
            transport: "stdio".into(),
            command_or_url: "python".into(),
            args: "script.py --arg 123".into(),
            field: FormField::Name,
        };
        let cfg = build_config(&form_stdio).unwrap();
        assert_eq!(cfg["name"], "my_tool");
        assert_eq!(cfg["transport"]["type"], "stdio");
        assert_eq!(cfg["transport"]["command"], "python");
        assert_eq!(
            cfg["transport"]["args"],
            json!(["script.py", "--arg", "123"])
        );
        assert_eq!(cfg["isActive"], true);

        // Valid http
        let form_http = FormState {
            editing_id: None,
            name: "http_tool".into(),
            transport: "http".into(),
            command_or_url: "https://api.example.com/mcp".into(),
            args: "".into(),
            field: FormField::Name,
        };
        let cfg_http = build_config(&form_http).unwrap();
        assert_eq!(cfg_http["transport"]["type"], "http");
        assert_eq!(cfg_http["transport"]["url"], "https://api.example.com/mcp");
    }

    #[test]
    fn select_failed_servers_only() {
        use crate::agent::runtime_state::{
            SessionRuntimeServerState, SessionRuntimeServerStatus, SessionRuntimeState,
            SessionRuntimeTransport,
        };

        let mut runtime = SessionRuntimeState::default();
        runtime.servers = vec![
            SessionRuntimeServerState {
                name: "ready_server".into(),
                transport: SessionRuntimeTransport::Stdio,
                status: SessionRuntimeServerStatus::Ready,
                tool_count: 3,
                error: None,
            },
            SessionRuntimeServerState {
                name: "failed_server_1".into(),
                transport: SessionRuntimeTransport::Stdio,
                status: SessionRuntimeServerStatus::Failed,
                tool_count: 0,
                error: Some("crash".into()),
            },
            SessionRuntimeServerState {
                name: "not_started_server".into(),
                transport: SessionRuntimeTransport::Stdio,
                status: SessionRuntimeServerStatus::NotStarted,
                tool_count: 0,
                error: None,
            },
            SessionRuntimeServerState {
                name: "failed_server_2".into(),
                transport: SessionRuntimeTransport::Stdio,
                status: SessionRuntimeServerStatus::Failed,
                tool_count: 0,
                error: Some("timeout".into()),
            },
        ];

        let failed = select_failed_servers(&runtime);
        assert_eq!(failed, vec!["failed_server_1", "failed_server_2"]);
    }

    #[test]
    fn attached_tui_mode_refusal() {
        // In unit test context, MCP_SERVER_REPOSITORY is unset, mimicking an attached TUI
        let open_res = ExtensionsScreen::open(None);
        assert!(open_res.is_err());
        assert!(open_res
            .unwrap_err()
            .contains("Extensions require the primary backend process"));

        let enable_res = ExtensionsScreen::apply_enable("any", true);
        assert!(enable_res.is_err());
        assert!(enable_res
            .unwrap_err()
            .contains("Extensions require the primary backend process"));

        let probe_res = ExtensionsScreen::apply_probe("any");
        assert!(probe_res.is_err());
        assert!(probe_res
            .unwrap_err()
            .contains("Extensions require the primary backend process"));

        let auth_res = ExtensionsScreen::apply_auth("any");
        assert!(auth_res.is_err());
        assert!(auth_res
            .unwrap_err()
            .contains("Extensions require the primary backend process"));

        let logout_res = ExtensionsScreen::apply_logout("any");
        assert!(logout_res.is_err());
        assert!(logout_res
            .unwrap_err()
            .contains("Extensions require the primary backend process"));
    }

    #[test]
    fn delete_confirmation_key_handling() {
        let mut screen = ExtensionsScreen {
            view: View::ConfirmDelete,
            rows: Vec::new(),
            filter: String::new(),
            selected: 0,
            tools_title: String::new(),
            tools_lines: Vec::new(),
            tools_scroll: 0,
            presets: Vec::new(),
            confirm_name: Some("test-srv".into()),
            form: None,
            background_op: None,
            notice: None,
        };

        // Press 'n' cancels
        let act = screen.handle_key(KeyEvent::from(KeyCode::Char('n')));
        assert_eq!(act, ExtensionsAction::None);
        assert_eq!(screen.view, View::List);
        assert_eq!(screen.confirm_name, None);
        assert_eq!(screen.notice.as_deref(), Some("Delete cancelled"));

        // Press Esc cancels
        screen.view = View::ConfirmDelete;
        screen.confirm_name = Some("test-srv".into());
        let act = screen.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(act, ExtensionsAction::None);
        assert_eq!(screen.view, View::List);
        assert_eq!(screen.confirm_name, None);
        assert_eq!(screen.notice.as_deref(), Some("Delete cancelled"));
    }

    #[test]
    fn form_key_navigation() {
        let mut screen = ExtensionsScreen {
            view: View::Form,
            rows: Vec::new(),
            filter: String::new(),
            selected: 0,
            tools_title: String::new(),
            tools_lines: Vec::new(),
            tools_scroll: 0,
            presets: Vec::new(),
            confirm_name: None,
            form: Some(FormState {
                editing_id: None,
                name: "initial".into(),
                transport: "stdio".into(),
                command_or_url: "cmd".into(),
                args: "".into(),
                field: FormField::Name,
            }),
            background_op: None,
            notice: None,
        };

        // Tab forwards
        screen.handle_key(KeyEvent::from(KeyCode::Tab));
        assert_eq!(screen.form.as_ref().unwrap().field, FormField::Transport);
        screen.handle_key(KeyEvent::from(KeyCode::Tab));
        assert_eq!(screen.form.as_ref().unwrap().field, FormField::CommandOrUrl);
        screen.handle_key(KeyEvent::from(KeyCode::Tab));
        assert_eq!(screen.form.as_ref().unwrap().field, FormField::Args);
        screen.handle_key(KeyEvent::from(KeyCode::Tab));
        assert_eq!(screen.form.as_ref().unwrap().field, FormField::Name);

        // BackTab backwards
        screen.handle_key(KeyEvent::from(KeyCode::BackTab));
        assert_eq!(screen.form.as_ref().unwrap().field, FormField::Args);

        // Esc cancels form
        let act = screen.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(act, ExtensionsAction::None);
        assert_eq!(screen.view, View::List);
        assert!(screen.form.is_none());
    }

    #[test]
    fn presets_key_navigation() {
        let mut screen = ExtensionsScreen {
            view: View::Presets,
            rows: Vec::new(),
            filter: String::new(),
            selected: 0,
            tools_title: "Presets".into(),
            tools_lines: Vec::new(),
            tools_scroll: 0,
            presets: vec![
                ("p1".into(), "preset 1".into()),
                ("p2".into(), "preset 2".into()),
            ],
            confirm_name: None,
            form: None,
            background_op: None,
            notice: None,
        };

        screen.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(screen.selected, 1);
        screen.handle_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(screen.selected, 0);

        let act = screen.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(act, ExtensionsAction::Closed);
    }

    #[test]
    fn background_spawn_on_plain_thread_without_panic() {
        // Gatekeeper G2.1: BackgroundOp must use tauri::async_runtime::spawn on a
        // plain std thread (no #[tokio::test]). start_session_retry itself needs a
        // booted backend; exercise the same spawn path directly.
        let mut screen = ExtensionsScreen::open_presets();
        screen.background_op = Some(BackgroundOp::spawn(
            "Retrying failed session servers".into(),
            async { Ok("ok".into()) },
        ));
        assert!(screen.background_op.is_some());
        assert!(screen
            .background_op
            .as_ref()
            .unwrap()
            .label
            .contains("Retrying"));
        if let Some(op) = screen.background_op.take() {
            op.abort();
        }
    }

    #[test]
    fn start_probe_returns_attached_tui_error_without_repo() {
        let mut screen = ExtensionsScreen::open_presets();
        let res = screen.start_probe("any");
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("attached TUI"));
    }
}
