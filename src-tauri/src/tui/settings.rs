//! TUI `/settings` screens (issue #2815 / parity §3.3–3.6).
//!
//! Draft edits stay in memory until `/settings save` or Ctrl+S. Persistence
//! goes through the shared settings repository (same rows the GUI uses).

use crate::models::advanced_settings::AdvancedSettings;
use crate::models::experimental_settings::ExperimentalSettings;
use crate::models::system_settings::SystemSettings;
use crate::repositories::settings_repository::SettingsRepository;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

const LLM_CONFIG_JSON: &str = include_str!("../../../src/config/llm-config.json");

const BUILTIN_PROVIDERS: &[&str] = &[
    "openai",
    "anthropic",
    "gemini",
    "ollama",
    "groq",
    "fireworks",
    "cerebras",
    "openrouter",
];

/// Slash-command verbs that open or mutate settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsCommand {
    Open,
    Section(SettingsSection),
    Save,
    Discard,
    Update,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingsSection {
    General,
    AiModels,
    ChatInterface,
    System,
    Advanced,
    Experimental,
}

impl SettingsSection {
    pub fn all() -> &'static [SettingsSection] {
        &[
            SettingsSection::General,
            SettingsSection::AiModels,
            SettingsSection::ChatInterface,
            SettingsSection::System,
            SettingsSection::Advanced,
            SettingsSection::Experimental,
        ]
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::AiModels => "AI & Models",
            Self::ChatInterface => "Chat Interface",
            Self::System => "System",
            Self::Advanced => "Advanced",
            Self::Experimental => "Experimental",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "general" => Some(Self::General),
            "ai-models" | "ai" | "models" => Some(Self::AiModels),
            "chat-interface" | "chat" => Some(Self::ChatInterface),
            "system" => Some(Self::System),
            "advanced" => Some(Self::Advanced),
            "experimental" => Some(Self::Experimental),
            _ => None,
        }
    }
}

/// Parse `/settings…` and `/update` composer lines.
pub fn parse_settings_command(text: &str) -> Option<SettingsCommand> {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') || trimmed.starts_with("//") {
        return None;
    }
    let mut parts = trimmed[1..].split_whitespace();
    let verb = parts.next()?.to_ascii_lowercase();
    match verb.as_str() {
        "update" => Some(SettingsCommand::Update),
        "settings" => match parts.next() {
            None => Some(SettingsCommand::Open),
            Some("save") => Some(SettingsCommand::Save),
            Some("discard") => Some(SettingsCommand::Discard),
            Some(section) => SettingsSection::parse(section).map(SettingsCommand::Section),
        },
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldKind {
    Text,
    Secret,
    Number,
    Toggle,
    Choice(&'static [(&'static str, &'static str)]),
    ModelPick,
}

#[derive(Debug, Clone, Copy)]
struct FieldSpec {
    section: SettingsSection,
    label: &'static str,
    /// Top-level settings row key.
    row: &'static str,
    /// Dot path inside the row JSON (`""` = whole value).
    path: &'static str,
    kind: FieldKind,
}

const LANGUAGE_CHOICES: &[(&str, &str)] = &[("en", "English"), ("ko", "한국어")];
const CONTEXT_CHOICES: &[(&str, &str)] = &[
    ("full", "Full"),
    ("window", "Window"),
    ("summary", "Summary"),
];
const THINKING_CHOICES: &[(&str, &str)] = &[
    ("off", "Off"),
    ("low", "Low"),
    ("medium", "Medium"),
    ("high", "High"),
    ("auto", "Auto"),
];
const ISOLATION_CHOICES: &[(&str, &str)] = &[
    ("off", "Off"),
    ("low", "Low"),
    ("medium", "Medium"),
    ("high", "High"),
];
const TOOL_LOOP_CHOICES: &[(&str, &str)] = &[
    ("resampleThenBreak", "Resample then break"),
    ("legacyGuidance", "Legacy guidance"),
];

const FIELDS: &[FieldSpec] = &[
    // General
    FieldSpec {
        section: SettingsSection::General,
        label: "Language",
        row: "uiLanguage",
        path: "",
        kind: FieldKind::Choice(LANGUAGE_CHOICES),
    },
    FieldSpec {
        section: SettingsSection::General,
        label: "Tool detail level",
        row: "toolCallGroupVisibleCount",
        path: "",
        kind: FieldKind::Number,
    },
    // AI & Models (P0)
    FieldSpec {
        section: SettingsSection::AiModels,
        label: "Default AI Model",
        row: "preferredModel",
        path: "",
        kind: FieldKind::ModelPick,
    },
    FieldSpec {
        section: SettingsSection::AiModels,
        label: "OpenAI API Key",
        row: "serviceConfigs",
        path: "openai.apiKey",
        kind: FieldKind::Secret,
    },
    FieldSpec {
        section: SettingsSection::AiModels,
        label: "OpenAI Base URL",
        row: "serviceConfigs",
        path: "openai.baseUrl",
        kind: FieldKind::Text,
    },
    FieldSpec {
        section: SettingsSection::AiModels,
        label: "Anthropic API Key",
        row: "serviceConfigs",
        path: "anthropic.apiKey",
        kind: FieldKind::Secret,
    },
    FieldSpec {
        section: SettingsSection::AiModels,
        label: "Gemini API Key",
        row: "serviceConfigs",
        path: "gemini.apiKey",
        kind: FieldKind::Secret,
    },
    FieldSpec {
        section: SettingsSection::AiModels,
        label: "Ollama Base URL",
        row: "serviceConfigs",
        path: "ollama.baseUrl",
        kind: FieldKind::Text,
    },
    FieldSpec {
        section: SettingsSection::AiModels,
        label: "Groq API Key",
        row: "serviceConfigs",
        path: "groq.apiKey",
        kind: FieldKind::Secret,
    },
    FieldSpec {
        section: SettingsSection::AiModels,
        label: "Fireworks API Key",
        row: "serviceConfigs",
        path: "fireworks.apiKey",
        kind: FieldKind::Secret,
    },
    FieldSpec {
        section: SettingsSection::AiModels,
        label: "Cerebras API Key",
        row: "serviceConfigs",
        path: "cerebras.apiKey",
        kind: FieldKind::Secret,
    },
    FieldSpec {
        section: SettingsSection::AiModels,
        label: "OpenRouter API Key",
        row: "serviceConfigs",
        path: "openrouter.apiKey",
        kind: FieldKind::Secret,
    },
    FieldSpec {
        section: SettingsSection::AiModels,
        label: "Thinking depth",
        row: "advancedSettings",
        path: "thinkingEffort",
        kind: FieldKind::Choice(THINKING_CHOICES),
    },
    FieldSpec {
        section: SettingsSection::AiModels,
        label: "Temperature override",
        row: "temperatureOverrideEnabled",
        path: "",
        kind: FieldKind::Toggle,
    },
    FieldSpec {
        section: SettingsSection::AiModels,
        label: "Temperature",
        row: "temperature",
        path: "",
        kind: FieldKind::Number,
    },
    // Chat interface
    FieldSpec {
        section: SettingsSection::ChatInterface,
        label: "Context strategy",
        row: "contextStrategy",
        path: "",
        kind: FieldKind::Choice(CONTEXT_CHOICES),
    },
    FieldSpec {
        section: SettingsSection::ChatInterface,
        label: "Window size",
        row: "windowSize",
        path: "",
        kind: FieldKind::Number,
    },
    FieldSpec {
        section: SettingsSection::ChatInterface,
        label: "Max input context",
        row: "maxInputContext",
        path: "",
        kind: FieldKind::Number,
    },
    FieldSpec {
        section: SettingsSection::ChatInterface,
        label: "Diff context lines",
        row: "advancedSettings",
        path: "diffContextLines",
        kind: FieldKind::Number,
    },
    FieldSpec {
        section: SettingsSection::ChatInterface,
        label: "Default max output tokens",
        row: "advancedSettings",
        path: "defaultMaxOutputTokens",
        kind: FieldKind::Number,
    },
    // System
    FieldSpec {
        section: SettingsSection::System,
        label: "HTTP server port",
        row: "systemSettings",
        path: "httpServerPort",
        kind: FieldKind::Number,
    },
    FieldSpec {
        section: SettingsSection::System,
        label: "Expose HTTP server",
        row: "systemSettings",
        path: "httpServerExpose",
        kind: FieldKind::Toggle,
    },
    FieldSpec {
        section: SettingsSection::System,
        label: "Keep awake during agent work",
        row: "systemSettings",
        path: "preventSleepDuringAgentWork",
        kind: FieldKind::Toggle,
    },
    FieldSpec {
        section: SettingsSection::System,
        label: "Log retention days",
        row: "systemSettings",
        path: "logRetentionDays",
        kind: FieldKind::Number,
    },
    // Advanced
    FieldSpec {
        section: SettingsSection::Advanced,
        label: "Max retries",
        row: "advancedSettings",
        path: "maxRetries",
        kind: FieldKind::Number,
    },
    FieldSpec {
        section: SettingsSection::Advanced,
        label: "Retry delay (ms)",
        row: "advancedSettings",
        path: "retryDelay",
        kind: FieldKind::Number,
    },
    FieldSpec {
        section: SettingsSection::Advanced,
        label: "Shell isolation",
        row: "systemSettings",
        path: "shellIsolationLevel",
        kind: FieldKind::Choice(ISOLATION_CHOICES),
    },
    FieldSpec {
        section: SettingsSection::Advanced,
        label: "Allow linked worktrees",
        row: "systemSettings",
        path: "allowLinkedWorktrees",
        kind: FieldKind::Toggle,
    },
    FieldSpec {
        section: SettingsSection::Advanced,
        label: "Max concurrent active sessions",
        row: "advancedSettings",
        path: "maxConcurrentActiveSessions",
        kind: FieldKind::Number,
    },
    // Experimental
    FieldSpec {
        section: SettingsSection::Experimental,
        label: "Inline audio attachment",
        row: "experimentalSettings",
        path: "inlineAudioAttachment",
        kind: FieldKind::Toggle,
    },
    FieldSpec {
        section: SettingsSection::Experimental,
        label: "Tool loop recovery",
        row: "experimentalSettings",
        path: "toolLoopRecoveryPolicy",
        kind: FieldKind::Choice(TOOL_LOOP_CHOICES),
    },
    FieldSpec {
        section: SettingsSection::Experimental,
        label: "Tool loop max resample retries",
        row: "experimentalSettings",
        path: "toolLoopMaxResampleRetries",
        kind: FieldKind::Number,
    },
];

#[derive(Debug, Clone)]
struct SettingsDraft {
    baseline: HashMap<String, Value>,
    draft: HashMap<String, Value>,
}

impl SettingsDraft {
    fn load_blocking() -> Result<Self, String> {
        let repo = crate::state::try_get_settings_repository()
            .ok_or_else(|| "Settings not ready yet".to_string())?;
        let rows = tauri::async_runtime::block_on(repo.list())
            .map_err(|e| format!("Failed to list settings: {e}"))?;
        let mut map = HashMap::new();
        for row in rows {
            let value = serde_json::from_str(&row.value).unwrap_or(Value::Null);
            map.insert(row.key, value);
        }
        seed_structured_defaults(&mut map);
        Ok(Self {
            baseline: map.clone(),
            draft: map,
        })
    }

    fn is_dirty(&self) -> bool {
        self.draft != self.baseline
    }

    fn section_dirty(&self, section: SettingsSection) -> bool {
        fields_for(section)
            .iter()
            .any(|field| self.draft.get(field.row) != self.baseline.get(field.row))
    }

    fn dirty_section_count(&self) -> usize {
        SettingsSection::all()
            .iter()
            .filter(|s| self.section_dirty(**s))
            .count()
    }

    fn discard(&mut self) {
        self.draft = self.baseline.clone();
    }

    fn save_blocking(&mut self) -> Result<(), String> {
        let repo = crate::state::try_get_settings_repository()
            .ok_or_else(|| "Settings not ready yet".to_string())?;
        let mut changed = HashMap::new();
        for (key, value) in &self.draft {
            if self.baseline.get(key) != Some(value) {
                changed.insert(key.clone(), value.clone());
            }
        }
        if changed.is_empty() {
            return Ok(());
        }
        let system = changed.get("systemSettings").cloned();
        tauri::async_runtime::block_on(repo.set_many(changed))
            .map_err(|e| format!("Failed to save settings: {e}"))?;
        if let Some(value) = system {
            apply_system_side_effects(&value);
        }
        self.baseline = self.draft.clone();
        Ok(())
    }

    fn get_path(&self, row: &str, path: &str) -> Value {
        let root = self.draft.get(row).cloned().unwrap_or(Value::Null);
        if path.is_empty() {
            return root;
        }
        walk_get(&root, path).cloned().unwrap_or(Value::Null)
    }

    fn set_path(&mut self, row: &str, path: &str, value: Value) {
        if path.is_empty() {
            self.draft.insert(row.to_string(), value);
            return;
        }
        let mut root = self
            .draft
            .get(row)
            .cloned()
            .unwrap_or_else(|| Value::Object(Map::new()));
        walk_set(&mut root, path, value);
        self.draft.insert(row.to_string(), root);
    }
}

fn seed_structured_defaults(map: &mut HashMap<String, Value>) {
    if !map.contains_key("advancedSettings") {
        map.insert(
            "advancedSettings".into(),
            serde_json::to_value(AdvancedSettings::default()).unwrap_or(Value::Null),
        );
    }
    if !map.contains_key("systemSettings") {
        map.insert(
            "systemSettings".into(),
            serde_json::to_value(SystemSettings::default()).unwrap_or(Value::Null),
        );
    }
    if !map.contains_key("experimentalSettings") {
        map.insert(
            "experimentalSettings".into(),
            serde_json::to_value(ExperimentalSettings::default()).unwrap_or(Value::Null),
        );
    }
    if !map.contains_key("serviceConfigs") {
        map.insert("serviceConfigs".into(), Value::Object(Map::new()));
    }
    if !map.contains_key("preferredModel") {
        map.insert("preferredModel".into(), Value::Null);
    }
    if !map.contains_key("uiLanguage") {
        map.insert("uiLanguage".into(), json!("en"));
    }
    if !map.contains_key("contextStrategy") {
        map.insert("contextStrategy".into(), json!("full"));
    }
    if !map.contains_key("windowSize") {
        map.insert("windowSize".into(), json!(40));
    }
    if !map.contains_key("maxInputContext") {
        map.insert("maxInputContext".into(), json!(128000));
    }
    if !map.contains_key("toolCallGroupVisibleCount") {
        map.insert("toolCallGroupVisibleCount".into(), json!(3));
    }
    if !map.contains_key("temperatureOverrideEnabled") {
        map.insert("temperatureOverrideEnabled".into(), json!(false));
    }
    if !map.contains_key("temperature") {
        map.insert("temperature".into(), json!(0.7));
    }
}

fn apply_system_side_effects(value: &Value) {
    let settings = SystemSettings::from_stored(Some(&value.to_string()));
    crate::utils::keep_awake::set_user_preference(settings.prevent_sleep_during_agent_work);
    crate::logger::set_log_retention_days(settings.log_retention_days);
}

fn fields_for(section: SettingsSection) -> Vec<&'static FieldSpec> {
    FIELDS.iter().filter(|f| f.section == section).collect()
}

fn walk_get<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = root;
    for part in path.split('.') {
        cur = cur.as_object()?.get(part)?;
    }
    Some(cur)
}

fn walk_set(root: &mut Value, path: &str, value: Value) {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        *root = value;
        return;
    }
    if !root.is_object() {
        *root = Value::Object(Map::new());
    }
    let mut cur = root.as_object_mut().expect("object");
    for (i, part) in parts.iter().enumerate() {
        if i + 1 == parts.len() {
            cur.insert((*part).to_string(), value);
            return;
        }
        let next = cur
            .entry((*part).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !next.is_object() {
            *next = Value::Object(Map::new());
        }
        cur = next.as_object_mut().expect("object");
    }
}

fn mask_secret(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "(empty)".to_string();
    }
    if trimmed.chars().count() <= 4 {
        return "••••".to_string();
    }
    let tail: String = trimmed
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("••••{tail}")
}

fn display_value(field: &FieldSpec, value: &Value, reveal: bool) -> String {
    match field.kind {
        FieldKind::Secret => {
            let s = value.as_str().unwrap_or("");
            if reveal {
                if s.is_empty() {
                    "(empty)".into()
                } else {
                    s.to_string()
                }
            } else {
                mask_secret(s)
            }
        }
        FieldKind::Toggle => {
            if value.as_bool().unwrap_or(false) {
                "On".into()
            } else {
                "Off".into()
            }
        }
        FieldKind::Choice(choices) => {
            let key = match value {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            };
            choices
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, label)| (*label).to_string())
                .unwrap_or_else(|| {
                    if key.is_empty() {
                        "(not set)".into()
                    } else {
                        key
                    }
                })
        }
        FieldKind::ModelPick => match value {
            Value::Object(map) => {
                let provider = map.get("provider").and_then(Value::as_str).unwrap_or("?");
                let model = map.get("model").and_then(Value::as_str).unwrap_or("?");
                format!("{provider}/{model}")
            }
            Value::Null => "(not set)".into(),
            other => other.to_string(),
        },
        FieldKind::Text | FieldKind::Number => match value {
            Value::Null => "(not set)".into(),
            Value::String(s) if s.is_empty() => "(empty)".into(),
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            other => other.to_string(),
        },
    }
}

/// Provider configured enough to appear in model pickers (GUI parity).
pub fn is_provider_configured(provider_id: &str, service_configs: &Value) -> bool {
    let Some(cfg) = service_configs
        .as_object()
        .and_then(|m| m.get(provider_id))
        .and_then(Value::as_object)
    else {
        // Ollama with no row is still unconfigured.
        return false;
    };
    if provider_id == "openai"
        && cfg
            .get("use3rdParty")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && cfg
            .get("customModelId")
            .and_then(Value::as_str)
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
    {
        return true;
    }
    if cfg
        .get("apiKey")
        .and_then(Value::as_str)
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
    {
        return true;
    }
    if provider_id == "ollama"
        && cfg
            .get("baseUrl")
            .and_then(Value::as_str)
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
    {
        return true;
    }
    false
}

/// True when no builtin provider is ready for a first message.
pub fn no_provider_configured(draft: &HashMap<String, Value>) -> bool {
    let configs = draft.get("serviceConfigs").cloned().unwrap_or(json!({}));
    !BUILTIN_PROVIDERS
        .iter()
        .any(|p| is_provider_configured(p, &configs))
}

fn catalog_models_for(provider: &str) -> Vec<String> {
    let Ok(root) = serde_json::from_str::<Value>(LLM_CONFIG_JSON) else {
        return Vec::new();
    };
    let Some(models) = root
        .pointer(&format!("/providers/{provider}/models"))
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };
    models.keys().cloned().collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Sections,
    Form,
    ModelPick,
    LeaveConfirm,
    UpdateNotice,
}

#[derive(Debug, Clone)]
struct ModelPickState {
    rows: Vec<(String, String)>, // provider, model (model "" = custom sentinel)
    cursor: usize,
    custom: bool,
    custom_text: String,
}

/// In-memory settings overlay owned by [`crate::tui::chat::ChatState`].
#[derive(Debug, Clone)]
pub struct SettingsScreen {
    draft: SettingsDraft,
    view: View,
    section_cursor: usize,
    form_section: SettingsSection,
    form_cursor: usize,
    form_page: usize,
    editing: bool,
    edit_buf: String,
    reveal_secrets: bool,
    model_pick: Option<ModelPickState>,
    notice: Option<String>,
    approval_waiting: bool,
}

/// Result of a key while settings are open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsAction {
    None,
    Closed,
    /// Closed after a successful save (chat may show a notice).
    ClosedSaved,
    ClosedDiscarded,
}

const FORM_PAGE_SIZE: usize = 8;

impl SettingsScreen {
    pub fn open(section: Option<SettingsSection>) -> Result<Self, String> {
        let draft = SettingsDraft::load_blocking()?;
        let mut screen = Self {
            draft,
            view: View::Sections,
            section_cursor: 0,
            form_section: SettingsSection::AiModels,
            form_cursor: 0,
            form_page: 0,
            editing: false,
            edit_buf: String::new(),
            reveal_secrets: false,
            model_pick: None,
            notice: None,
            approval_waiting: false,
        };
        if let Some(section) = section {
            screen.open_section(section);
        }
        Ok(screen)
    }

    pub fn open_update_notice() -> Self {
        // Lightweight stub for P2 `/update` (About / Check for Updates).
        Self {
            draft: SettingsDraft {
                baseline: HashMap::new(),
                draft: HashMap::new(),
            },
            view: View::UpdateNotice,
            section_cursor: 0,
            form_section: SettingsSection::General,
            form_cursor: 0,
            form_page: 0,
            editing: false,
            edit_buf: String::new(),
            reveal_secrets: false,
            model_pick: None,
            notice: None,
            approval_waiting: false,
        }
    }

    pub fn set_approval_waiting(&mut self, waiting: bool) {
        self.approval_waiting = waiting;
    }

    fn open_section(&mut self, section: SettingsSection) {
        self.form_section = section;
        self.form_cursor = 0;
        self.form_page = 0;
        self.editing = false;
        self.edit_buf.clear();
        self.view = View::Form;
        if let Some(idx) = SettingsSection::all().iter().position(|s| *s == section) {
            self.section_cursor = idx;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> SettingsAction {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match self.view {
            View::UpdateNotice => match key.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => SettingsAction::Closed,
                _ => SettingsAction::None,
            },
            View::LeaveConfirm => self.handle_leave_confirm(key),
            View::ModelPick => self.handle_model_pick(key),
            View::Sections => self.handle_sections(key, ctrl),
            View::Form => self.handle_form(key, ctrl),
        }
    }

    fn handle_leave_confirm(&mut self, key: KeyEvent) -> SettingsAction {
        match key.code {
            KeyCode::Char('s') | KeyCode::Char('S') => match self.draft.save_blocking() {
                Ok(()) => SettingsAction::ClosedSaved,
                Err(err) => {
                    self.notice = Some(err);
                    self.view = View::Sections;
                    SettingsAction::None
                }
            },
            KeyCode::Char('d') | KeyCode::Char('D') => {
                self.draft.discard();
                SettingsAction::ClosedDiscarded
            }
            KeyCode::Esc | KeyCode::Char('c') | KeyCode::Char('C') => {
                self.view = View::Sections;
                SettingsAction::None
            }
            _ => SettingsAction::None,
        }
    }

    fn handle_sections(&mut self, key: KeyEvent, ctrl: bool) -> SettingsAction {
        if ctrl && matches!(key.code, KeyCode::Char('s') | KeyCode::Char('S')) {
            return self.save_action();
        }
        let sections = SettingsSection::all();
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.section_cursor > 0 {
                    self.section_cursor -= 1;
                }
                SettingsAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.section_cursor + 1 < sections.len() {
                    self.section_cursor += 1;
                }
                SettingsAction::None
            }
            KeyCode::Enter => {
                self.open_section(sections[self.section_cursor]);
                SettingsAction::None
            }
            KeyCode::Esc => self.request_close(),
            KeyCode::Char('s') if !ctrl => self.save_action(),
            _ => SettingsAction::None,
        }
    }

    fn handle_form(&mut self, key: KeyEvent, ctrl: bool) -> SettingsAction {
        if ctrl && matches!(key.code, KeyCode::Char('s') | KeyCode::Char('S')) {
            return self.save_action();
        }
        if ctrl && matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R')) {
            self.reveal_secrets = !self.reveal_secrets;
            return SettingsAction::None;
        }
        if ctrl && matches!(key.code, KeyCode::Left) {
            if self.form_page > 0 {
                self.form_page -= 1;
                self.form_cursor = self.form_page * FORM_PAGE_SIZE;
            }
            return SettingsAction::None;
        }
        if ctrl && matches!(key.code, KeyCode::Right) {
            let fields = fields_for(self.form_section);
            let max_page = fields.len().saturating_sub(1) / FORM_PAGE_SIZE;
            if self.form_page < max_page {
                self.form_page += 1;
                self.form_cursor = self.form_page * FORM_PAGE_SIZE;
            }
            return SettingsAction::None;
        }

        if self.editing {
            return self.handle_editing(key);
        }

        let fields = fields_for(self.form_section);
        if fields.is_empty() {
            if matches!(key.code, KeyCode::Esc) {
                self.view = View::Sections;
            }
            return SettingsAction::None;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => {
                if self.form_cursor > 0 {
                    self.form_cursor -= 1;
                    self.form_page = self.form_cursor / FORM_PAGE_SIZE;
                }
                SettingsAction::None
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                if self.form_cursor + 1 < fields.len() {
                    self.form_cursor += 1;
                    self.form_page = self.form_cursor / FORM_PAGE_SIZE;
                }
                SettingsAction::None
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                let field = fields[self.form_cursor];
                self.begin_edit(field);
                SettingsAction::None
            }
            KeyCode::Esc => {
                self.view = View::Sections;
                SettingsAction::None
            }
            _ => SettingsAction::None,
        }
    }

    fn begin_edit(&mut self, field: &FieldSpec) {
        match field.kind {
            FieldKind::Toggle => {
                let cur = self.draft.get_path(field.row, field.path);
                let next = !cur.as_bool().unwrap_or(false);
                self.draft
                    .set_path(field.row, field.path, Value::Bool(next));
            }
            FieldKind::Choice(choices) => {
                let cur = self
                    .draft
                    .get_path(field.row, field.path)
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let idx = choices.iter().position(|(k, _)| *k == cur).unwrap_or(0);
                let next = choices[(idx + 1) % choices.len()].0;
                self.draft
                    .set_path(field.row, field.path, Value::String(next.to_string()));
            }
            FieldKind::ModelPick => self.open_model_pick(),
            FieldKind::Text | FieldKind::Secret | FieldKind::Number => {
                let cur = self.draft.get_path(field.row, field.path);
                self.edit_buf = match cur {
                    Value::String(s) => s,
                    Value::Number(n) => n.to_string(),
                    Value::Null => String::new(),
                    other => other.to_string(),
                };
                self.editing = true;
            }
        }
    }

    fn handle_editing(&mut self, key: KeyEvent) -> SettingsAction {
        match key.code {
            KeyCode::Esc => {
                self.editing = false;
                self.edit_buf.clear();
                SettingsAction::None
            }
            KeyCode::Enter => {
                let fields = fields_for(self.form_section);
                let field = fields[self.form_cursor];
                let value = match field.kind {
                    FieldKind::Number => {
                        if let Ok(n) = self.edit_buf.trim().parse::<f64>() {
                            json!(n)
                        } else {
                            self.notice = Some("Enter a number".into());
                            return SettingsAction::None;
                        }
                    }
                    _ => Value::String(self.edit_buf.clone()),
                };
                self.draft.set_path(field.row, field.path, value);
                self.editing = false;
                self.edit_buf.clear();
                SettingsAction::None
            }
            KeyCode::Backspace => {
                self.edit_buf.pop();
                SettingsAction::None
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.edit_buf.push(ch);
                SettingsAction::None
            }
            _ => SettingsAction::None,
        }
    }

    fn open_model_pick(&mut self) {
        let configs = self
            .draft
            .draft
            .get("serviceConfigs")
            .cloned()
            .unwrap_or(json!({}));
        let mut rows = Vec::new();
        for provider in BUILTIN_PROVIDERS {
            if !is_provider_configured(provider, &configs) {
                continue;
            }
            for model in catalog_models_for(provider) {
                rows.push(((*provider).to_string(), model));
            }
            rows.push(((*provider).to_string(), String::new())); // custom id
        }
        if rows.is_empty() {
            self.notice = Some(
                "No provider configured. Add an API key below, save, then pick a model.".into(),
            );
            return;
        }
        self.model_pick = Some(ModelPickState {
            rows,
            cursor: 0,
            custom: false,
            custom_text: String::new(),
        });
        self.view = View::ModelPick;
    }

    fn handle_model_pick(&mut self, key: KeyEvent) -> SettingsAction {
        let Some(pick) = self.model_pick.as_mut() else {
            self.view = View::Form;
            return SettingsAction::None;
        };
        if pick.custom {
            match key.code {
                KeyCode::Esc => {
                    pick.custom = false;
                    pick.custom_text.clear();
                    SettingsAction::None
                }
                KeyCode::Enter => {
                    let provider = pick.rows[pick.cursor].0.clone();
                    let model = pick.custom_text.trim().to_string();
                    if model.is_empty() {
                        self.notice = Some("Enter a model id".into());
                        return SettingsAction::None;
                    }
                    self.draft.set_path(
                        "preferredModel",
                        "",
                        json!({ "provider": provider, "model": model }),
                    );
                    self.model_pick = None;
                    self.view = View::Form;
                    SettingsAction::None
                }
                KeyCode::Backspace => {
                    pick.custom_text.pop();
                    SettingsAction::None
                }
                KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    pick.custom_text.push(ch);
                    SettingsAction::None
                }
                _ => SettingsAction::None,
            }
        } else {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if pick.cursor > 0 {
                        pick.cursor -= 1;
                    }
                    SettingsAction::None
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if pick.cursor + 1 < pick.rows.len() {
                        pick.cursor += 1;
                    }
                    SettingsAction::None
                }
                KeyCode::Esc => {
                    self.model_pick = None;
                    self.view = View::Form;
                    SettingsAction::None
                }
                KeyCode::Enter => {
                    let (provider, model) = pick.rows[pick.cursor].clone();
                    if model.is_empty() {
                        pick.custom = true;
                        pick.custom_text.clear();
                        return SettingsAction::None;
                    }
                    self.draft.set_path(
                        "preferredModel",
                        "",
                        json!({ "provider": provider, "model": model }),
                    );
                    self.model_pick = None;
                    self.view = View::Form;
                    SettingsAction::None
                }
                _ => SettingsAction::None,
            }
        }
    }

    fn save_action(&mut self) -> SettingsAction {
        match self.draft.save_blocking() {
            Ok(()) => {
                self.notice = Some("Settings saved successfully".into());
                SettingsAction::None
            }
            Err(err) => {
                self.notice = Some(format!("Failed to save settings: {err}"));
                SettingsAction::None
            }
        }
    }

    fn request_close(&mut self) -> SettingsAction {
        if self.draft.is_dirty() {
            self.view = View::LeaveConfirm;
            SettingsAction::None
        } else {
            SettingsAction::Closed
        }
    }

    pub fn apply_command(&mut self, command: &SettingsCommand) -> SettingsAction {
        match command {
            SettingsCommand::Open => {
                self.view = View::Sections;
                SettingsAction::None
            }
            SettingsCommand::Section(section) => {
                self.open_section(*section);
                SettingsAction::None
            }
            SettingsCommand::Save => self.save_action(),
            SettingsCommand::Discard => {
                self.draft.discard();
                self.notice = Some("Changes discarded".into());
                SettingsAction::None
            }
            SettingsCommand::Update => {
                self.view = View::UpdateNotice;
                SettingsAction::None
            }
        }
    }
}

/// Draw the settings overlay.
pub fn draw_settings(frame: &mut Frame<'_>, screen: &SettingsScreen) {
    match screen.view {
        View::UpdateNotice => draw_update_notice(frame),
        View::LeaveConfirm => draw_leave_confirm(frame, screen),
        View::ModelPick => draw_model_pick(frame, screen),
        View::Sections => draw_sections(frame, screen),
        View::Form => draw_form(frame, screen),
    }
}

fn footer_line(screen: &SettingsScreen, base: &str) -> String {
    let mut parts = vec![base.to_string()];
    if screen.approval_waiting {
        parts.push("Approval waiting — Esc to return".into());
    }
    if let Some(notice) = &screen.notice {
        parts.push(notice.clone());
    }
    parts.join(" · ")
}

fn draw_sections(frame: &mut Frame<'_>, screen: &SettingsScreen) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(frame.area());

    let dirty = screen.draft.dirty_section_count();
    let title = if dirty > 0 {
        format!(" Settings  ({dirty} sections changed) ")
    } else {
        " Settings ".into()
    };
    frame.render_widget(
        Paragraph::new(title).block(Block::default().borders(Borders::BOTTOM)),
        chunks[0],
    );

    let items: Vec<ListItem> = SettingsSection::all()
        .iter()
        .enumerate()
        .map(|(i, section)| {
            let marker = if screen.draft.section_dirty(*section) {
                " *"
            } else {
                ""
            };
            let prefix = if i == screen.section_cursor { ">" } else { " " };
            ListItem::new(format!("{prefix} {}{marker}", section.title()))
        })
        .collect();
    frame.render_widget(
        List::new(items).block(Block::default().borders(Borders::NONE)),
        chunks[1],
    );

    frame.render_widget(
        Paragraph::new(footer_line(
            screen,
            "Enter open · Ctrl+S save · Esc back · /settings save|discard",
        ))
        .wrap(Wrap { trim: true }),
        chunks[2],
    );
}

fn draw_form(frame: &mut Frame<'_>, screen: &SettingsScreen) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(frame.area());

    let dirty = if screen.draft.section_dirty(screen.form_section) {
        " · changed"
    } else {
        ""
    };
    frame.render_widget(
        Paragraph::new(format!(" {}{dirty} ", screen.form_section.title()))
            .block(Block::default().borders(Borders::BOTTOM)),
        chunks[0],
    );

    let fields = fields_for(screen.form_section);
    let start = screen.form_page * FORM_PAGE_SIZE;
    let end = (start + FORM_PAGE_SIZE).min(fields.len());
    let mut lines = Vec::new();
    for (idx, field) in fields.iter().enumerate().take(end).skip(start) {
        let value = screen.draft.get_path(field.row, field.path);
        let shown = if screen.editing && idx == screen.form_cursor {
            format!("[{}]", screen.edit_buf)
        } else {
            display_value(field, &value, screen.reveal_secrets)
        };
        let prefix = if idx == screen.form_cursor { ">" } else { " " };
        lines.push(format!("{prefix} {}: {}", field.label, shown));
    }
    if fields.len() > FORM_PAGE_SIZE {
        let pages = fields.len().div_ceil(FORM_PAGE_SIZE);
        lines.push(format!(
            "  page {}/{} (Ctrl+←/→)",
            screen.form_page + 1,
            pages
        ));
    }
    frame.render_widget(
        Paragraph::new(lines.join("\n")).wrap(Wrap { trim: false }),
        chunks[1],
    );

    frame.render_widget(
        Paragraph::new(footer_line(
            screen,
            "Enter edit · Ctrl+S save · Ctrl+R reveal secrets · Esc back",
        ))
        .wrap(Wrap { trim: true }),
        chunks[2],
    );
}

fn draw_model_pick(frame: &mut Frame<'_>, screen: &SettingsScreen) {
    let Some(pick) = &screen.model_pick else {
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
    frame.render_widget(
        Paragraph::new(" Default AI Model ").block(Block::default().borders(Borders::BOTTOM)),
        chunks[0],
    );
    if pick.custom {
        frame.render_widget(
            Paragraph::new(format!(
                "Custom model id for {}:\n> {}",
                pick.rows[pick.cursor].0, pick.custom_text
            )),
            chunks[1],
        );
        frame.render_widget(Paragraph::new("Enter confirm · Esc cancel"), chunks[2]);
        return;
    }
    let items: Vec<ListItem> = pick
        .rows
        .iter()
        .enumerate()
        .map(|(i, (provider, model))| {
            let label = if model.is_empty() {
                format!("{provider} · Enter custom model id…")
            } else {
                format!("{provider}/{model}")
            };
            let prefix = if i == pick.cursor { ">" } else { " " };
            ListItem::new(format!("{prefix} {label}"))
        })
        .collect();
    frame.render_widget(List::new(items), chunks[1]);
    frame.render_widget(
        Paragraph::new(footer_line(screen, "Enter select · Esc back")),
        chunks[2],
    );
}

fn draw_leave_confirm(frame: &mut Frame<'_>, screen: &SettingsScreen) {
    let text =
        "Unsaved settings changes.\n\n[S] Save and leave\n[D] Discard and leave\n[C]/Esc Cancel";
    frame.render_widget(
        Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Leave settings "),
            )
            .wrap(Wrap { trim: false }),
        centered_rect(60, 40, frame.area()),
    );
    let _ = screen;
}

fn draw_update_notice(frame: &mut Frame<'_>) {
    let text = "Check for Updates is not available in the TUI yet (P2).\nUse the GUI About screen, or watch GitHub Releases.\n\nPress Esc / Enter to return.";
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(" /update "))
            .wrap(Wrap { trim: false }),
        centered_rect(70, 40, frame.area()),
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
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
        .split(vertical[1])[1]
}

/// Snapshot whether providers are configured without opening the full screen.
pub fn providers_ready_blocking() -> Result<bool, String> {
    let draft = SettingsDraft::load_blocking()?;
    Ok(!no_provider_configured(&draft.draft))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_settings_commands() {
        assert_eq!(
            parse_settings_command("/settings"),
            Some(SettingsCommand::Open)
        );
        assert_eq!(
            parse_settings_command("/settings ai-models"),
            Some(SettingsCommand::Section(SettingsSection::AiModels))
        );
        assert_eq!(
            parse_settings_command("/settings save"),
            Some(SettingsCommand::Save)
        );
        assert_eq!(
            parse_settings_command("/settings discard"),
            Some(SettingsCommand::Discard)
        );
        assert_eq!(
            parse_settings_command("/update"),
            Some(SettingsCommand::Update)
        );
        assert_eq!(parse_settings_command("/settings nope"), None);
        assert_eq!(parse_settings_command("//settings"), None);
    }

    #[test]
    fn mask_secret_hides_body() {
        assert_eq!(mask_secret(""), "(empty)");
        assert_eq!(mask_secret("abcd"), "••••");
        assert_eq!(mask_secret("sk-abcdefgh"), "••••efgh");
    }

    #[test]
    fn provider_configured_rules() {
        let empty = json!({});
        assert!(!is_provider_configured("openai", &empty));
        let with_key = json!({ "openai": { "apiKey": "sk-test" } });
        assert!(is_provider_configured("openai", &with_key));
        let ollama = json!({ "ollama": { "baseUrl": "http://localhost:11434" } });
        assert!(is_provider_configured("ollama", &ollama));
        let third = json!({
            "openai": { "use3rdParty": true, "customModelId": "my-model" }
        });
        assert!(is_provider_configured("openai", &third));
    }

    #[test]
    fn catalog_includes_openai_models() {
        let models = catalog_models_for("openai");
        assert!(!models.is_empty());
    }

    #[test]
    fn walk_set_nested_service_config() {
        let mut root = json!({});
        walk_set(&mut root, "openai.apiKey", json!("sk-1"));
        assert_eq!(root["openai"]["apiKey"], json!("sk-1"));
        walk_set(&mut root, "openai.baseUrl", json!("https://example"));
        assert_eq!(root["openai"]["apiKey"], json!("sk-1"));
        assert_eq!(root["openai"]["baseUrl"], json!("https://example"));
    }
}
