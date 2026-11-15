//! TUI chat core: transcript, composer, streaming poll, and S5 prompts (#2762),
//! plus session slash commands (#2763).
//!
//! Drives the shared Session API from the TUI thread once the backend is live.
//! Layout and key rules follow `docs/specs/tui-parity.md` §3 (S1 + S2 + S5).

use crate::agent::session_manager::PendingApprovalSummary;
use crate::agent::state::{
    PendingUserPromptSummary, UserPromptKind, UserPromptPayload, UserQuestion,
};
use crate::server::shared_backend::SharedBackendClient;
use crate::tui::sessions::{
    create_new_session, delete_session, draw_picker, load_resume_picker, open_session_or_refuse,
    parse_session_command, rename_session, PickerAction, SessionCommand, SessionPicker,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use serde_json::{json, Map, Value};
use std::io::Stdout;
use std::time::{Duration, Instant};

const POLL_EVERY: Duration = Duration::from_millis(250);
const CTRL_C_QUIT_WINDOW: Duration = Duration::from_secs(2);
/// Max rows the S5 strip may grow to (approval / askUser bodies scroll inside).
const S5_MAX_ROWS: u16 = 8;

/// Where keyboard focus sits while an S5 prompt is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    #[default]
    Composer,
    Prompt,
}

/// One line shown in the transcript (role + text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptLine {
    pub role: String,
    pub text: String,
}

/// In-progress answers for a multi-question `askUser` prompt.
#[derive(Debug, Default, Clone)]
struct AskDraft {
    request_id: Option<String>,
    index: usize,
    /// Highlighted option row (0..options.len(), or options.len() for Other).
    cursor: usize,
    selected: Vec<String>,
    other_on: bool,
    text: String,
    /// Answers for questions already confirmed with Enter.
    committed: Map<String, Value>,
}

/// Mutable chat UI state. Pure updates live here so key handling is testable
/// without a terminal.
#[derive(Debug, Default)]
pub struct ChatState {
    pub session_id: Option<String>,
    pub session_title: String,
    pub composer: String,
    pub transcript: Vec<TranscriptLine>,
    pub status: String,
    pub notice: Option<String>,
    pub focus: Focus,
    pub pending_approval: Option<PendingApprovalSummary>,
    pub pending_prompt: Option<PendingUserPromptSummary>,
    /// Free-text / secret answer while the prompt has focus.
    pub prompt_input: String,
    /// Lines scrolled up from the newest transcript row. `0` follows the tail.
    pub scroll_from_bottom: u16,
    /// Scroll inside a tall S5 body (approval JSON / ask list).
    pub s5_scroll: u16,
    ask: AskDraft,
    /// S2 session picker (`/resume` / `/history`).
    pub picker: Option<SessionPicker>,
    /// Pending `/delete` confirmation (session id).
    pub confirm_delete: Option<String>,
    last_ctrl_c: Option<Instant>,
}

impl ChatState {
    pub fn new() -> Self {
        Self {
            session_title: "New Session".to_string(),
            status: "Waiting for backend…".to_string(),
            ..Self::default()
        }
    }

    /// Apply a key according to the parity focus rules. Returns true when the
    /// caller should quit.
    pub fn handle_key(&mut self, key: KeyEvent) -> ChatAction {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);

        if ctrl && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C')) {
            return self.handle_ctrl_c();
        }
        if ctrl
            && matches!(key.code, KeyCode::Char('d') | KeyCode::Char('D'))
            && self.composer.is_empty()
            && self.focus == Focus::Composer
            && self.picker.is_none()
        {
            return ChatAction::Quit;
        }

        if let Some(id) = self.confirm_delete.clone() {
            return match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.confirm_delete = None;
                    ChatAction::Session(SessionCommand::Delete {
                        session_id: Some(id),
                    })
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.confirm_delete = None;
                    self.notice = Some("Delete cancelled".to_string());
                    ChatAction::None
                }
                _ => ChatAction::None,
            };
        }

        if let Some(picker) = self.picker.as_mut() {
            return match picker.handle_key(key) {
                PickerAction::None => ChatAction::None,
                PickerAction::Cancel => {
                    self.picker = None;
                    ChatAction::None
                }
                PickerAction::Open(id) => {
                    self.picker = None;
                    ChatAction::OpenSession(id)
                }
                PickerAction::Refused { reason, .. } => {
                    self.picker = None;
                    self.notice = Some(reason);
                    ChatAction::None
                }
            };
        }

        // Alt+Y / Alt+N always reach an open approval, even with composer focus.
        if self.pending_approval.is_some() {
            if alt && matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y')) {
                return ChatAction::Approve;
            }
            if alt && matches!(key.code, KeyCode::Char('n') | KeyCode::Char('N')) {
                return ChatAction::Reject;
            }
        }

        match self.focus {
            Focus::Composer => self.handle_composer_key(key),
            Focus::Prompt => self.handle_prompt_key(key),
        }
    }

    fn handle_ctrl_c(&mut self) -> ChatAction {
        if !self.composer.is_empty() {
            self.composer.clear();
            self.last_ctrl_c = None;
            return ChatAction::None;
        }
        let now = Instant::now();
        if self
            .last_ctrl_c
            .is_some_and(|prev| now.duration_since(prev) <= CTRL_C_QUIT_WINDOW)
        {
            return ChatAction::Quit;
        }
        self.last_ctrl_c = Some(now);
        self.notice = Some("Press Ctrl+C again to quit".to_string());
        ChatAction::None
    }

    fn handle_composer_key(&mut self, key: KeyEvent) -> ChatAction {
        match key.code {
            KeyCode::Tab if self.has_s5() => {
                self.focus = Focus::Prompt;
                ChatAction::None
            }
            KeyCode::Enter => {
                let text = self.composer.trim().to_string();
                if text.is_empty() {
                    return ChatAction::None;
                }
                // `//` sends a literal leading slash (parity §3.2).
                if let Some(rest) = text.strip_prefix("//") {
                    self.composer.clear();
                    return ChatAction::Send(format!("/{rest}"));
                }
                if let Some(command) = parse_session_command(&text) {
                    self.composer.clear();
                    if let SessionCommand::Rename { name } = &command {
                        if name.is_empty() {
                            self.notice = Some("Usage: /rename <name>".to_string());
                            return ChatAction::None;
                        }
                    }
                    if let SessionCommand::Delete { session_id } = &command {
                        let id = match session_id.clone().or_else(|| self.session_id.clone()) {
                            Some(id) => id,
                            None => {
                                self.notice = Some("No open session to delete".to_string());
                                return ChatAction::None;
                            }
                        };
                        self.confirm_delete = Some(id.clone());
                        self.notice = Some(format!("Delete session {id} and its children? y/N"));
                        return ChatAction::None;
                    }
                    return ChatAction::Session(command);
                }
                if text.starts_with('/') {
                    self.notice = Some(format!("Unknown command {text}"));
                    return ChatAction::None;
                }
                self.composer.clear();
                ChatAction::Send(text)
            }
            KeyCode::Backspace => {
                self.composer.pop();
                ChatAction::None
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.composer.push(ch);
                ChatAction::None
            }
            // Up → older (away from tail); Down → newer; End → follow latest.
            KeyCode::Up => {
                self.scroll_from_bottom = self.scroll_from_bottom.saturating_add(1);
                ChatAction::None
            }
            KeyCode::Down => {
                self.scroll_from_bottom = self.scroll_from_bottom.saturating_sub(1);
                ChatAction::None
            }
            KeyCode::End => {
                self.scroll_from_bottom = 0;
                ChatAction::None
            }
            KeyCode::Esc if self.pending_approval.is_some() => ChatAction::Reject,
            _ => ChatAction::None,
        }
    }

    fn handle_prompt_key(&mut self, key: KeyEvent) -> ChatAction {
        match key.code {
            KeyCode::Tab => {
                self.focus = Focus::Composer;
                ChatAction::None
            }
            KeyCode::Char('a') | KeyCode::Char('A') if self.pending_approval.is_some() => {
                ChatAction::Approve
            }
            KeyCode::Char('r') | KeyCode::Char('R') if self.pending_approval.is_some() => {
                ChatAction::Reject
            }
            KeyCode::Up if self.pending_approval.is_some() => {
                self.s5_scroll = self.s5_scroll.saturating_sub(1);
                ChatAction::None
            }
            KeyCode::Down if self.pending_approval.is_some() => {
                self.s5_scroll = self.s5_scroll.saturating_add(1);
                ChatAction::None
            }
            _ if self.is_ask_user() => self.handle_ask_key(key),
            KeyCode::Enter if self.pending_prompt.is_some() => {
                let value = self.prompt_input.clone();
                self.prompt_input.clear();
                ChatAction::AnswerPrompt(value)
            }
            KeyCode::Esc if self.pending_prompt.is_some() => {
                // Secrets may cancel; askUser Esc does not cancel the request
                // (parity). Only cancel secrets here.
                if self
                    .pending_prompt
                    .as_ref()
                    .is_some_and(|p| p.kind == UserPromptKind::Secret)
                {
                    self.prompt_input.clear();
                    ChatAction::CancelPrompt
                } else {
                    ChatAction::None
                }
            }
            KeyCode::Backspace => {
                self.prompt_input.pop();
                ChatAction::None
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.prompt_input.push(ch);
                ChatAction::None
            }
            _ => ChatAction::None,
        }
    }

    fn is_ask_user(&self) -> bool {
        matches!(
            self.pending_prompt.as_ref().map(|p| &p.payload),
            Some(UserPromptPayload::Question { .. })
        )
    }

    fn ask_questions(&self) -> Option<&[UserQuestion]> {
        match self.pending_prompt.as_ref().map(|p| &p.payload) {
            Some(UserPromptPayload::Question { questions }) => Some(questions.as_slice()),
            _ => None,
        }
    }

    fn handle_ask_key(&mut self, key: KeyEvent) -> ChatAction {
        let Some(questions) = self.ask_questions().map(|q| q.to_vec()) else {
            return ChatAction::None;
        };
        if questions.is_empty() {
            return ChatAction::None;
        }
        if self.ask.index >= questions.len() {
            self.ask.index = questions.len() - 1;
        }
        let question = &questions[self.ask.index];
        let option_count = question.options.as_ref().map_or(0, |o| o.len());
        let row_count = option_count + usize::from(question.allow_custom);

        match key.code {
            KeyCode::Up if row_count > 0 => {
                self.ask.cursor = self.ask.cursor.saturating_sub(1);
                ChatAction::None
            }
            KeyCode::Down if row_count > 0 => {
                if self.ask.cursor + 1 < row_count {
                    self.ask.cursor += 1;
                }
                ChatAction::None
            }
            KeyCode::Char(' ') if option_count > 0 || question.allow_custom => {
                self.toggle_ask_row(question);
                ChatAction::None
            }
            KeyCode::Enter => self.advance_or_submit_ask(&questions),
            KeyCode::Backspace => {
                self.ask.text.pop();
                ChatAction::None
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Typing implies free-text / Other text.
                if question.options.is_none() || question.allow_custom {
                    if question.allow_custom {
                        self.ask.other_on = true;
                        self.ask.cursor = option_count;
                    }
                    self.ask.text.push(ch);
                }
                ChatAction::None
            }
            KeyCode::Esc => ChatAction::None,
            _ => ChatAction::None,
        }
    }

    fn toggle_ask_row(&mut self, question: &UserQuestion) {
        let options = question.options.as_deref().unwrap_or(&[]);
        if question.allow_custom && self.ask.cursor == options.len() {
            self.ask.other_on = !self.ask.other_on;
            return;
        }
        let Some(value) = options.get(self.ask.cursor).cloned() else {
            return;
        };
        if question.multi_select {
            if let Some(pos) = self.ask.selected.iter().position(|v| v == &value) {
                self.ask.selected.remove(pos);
            } else {
                self.ask.selected.push(value);
            }
        } else {
            self.ask.selected = vec![value];
            self.ask.other_on = false;
        }
    }

    fn advance_or_submit_ask(&mut self, questions: &[UserQuestion]) -> ChatAction {
        let question = &questions[self.ask.index];
        match current_ask_answer(question, &self.ask) {
            Ok(answer) => {
                self.ask.committed.insert(question.id.clone(), answer);
            }
            Err(msg) => {
                self.notice = Some(msg);
                return ChatAction::None;
            }
        }
        if self.ask.index + 1 < questions.len() {
            self.ask.index += 1;
            self.ask.cursor = 0;
            self.ask.selected.clear();
            self.ask.other_on = false;
            self.ask.text.clear();
            return ChatAction::None;
        }
        let payload = Value::Object(std::mem::take(&mut self.ask.committed));
        self.ask = AskDraft::default();
        match serde_json::to_string(&payload) {
            Ok(raw) => ChatAction::AnswerPrompt(raw),
            Err(err) => {
                self.notice = Some(format!("Failed to encode answers: {err}"));
                ChatAction::None
            }
        }
    }

    /// Reset ask wizard state when the pending prompt request changes.
    pub fn sync_ask_with_prompt(&mut self) {
        let request_id = self
            .pending_prompt
            .as_ref()
            .filter(|p| matches!(p.payload, UserPromptPayload::Question { .. }))
            .map(|p| p.request_id.clone());
        if self.ask.request_id != request_id {
            self.ask = AskDraft {
                request_id,
                ..AskDraft::default()
            };
            self.s5_scroll = 0;
        }
    }

    fn has_s5(&self) -> bool {
        self.pending_approval.is_some() || self.pending_prompt.is_some()
    }

    /// Rebuild the transcript from a Session API message list.
    ///
    /// The API returns newest-first (`ORDER BY rowid DESC`); the TUI shows
    /// oldest at the top, so this reverses the array.
    pub fn apply_messages(&mut self, messages: &[Value]) {
        self.transcript.clear();
        for message in messages.iter().rev() {
            let role = message
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let text = message_text(message);
            if text.is_empty() && role != "assistant" {
                continue;
            }
            let label = match role {
                "user" => "You",
                "assistant" => "AI",
                "tool" => "Tool",
                other => other,
            };
            self.transcript.push(TranscriptLine {
                role: label.to_string(),
                text: if text.is_empty() {
                    "…".to_string()
                } else {
                    text
                },
            });
        }
    }
}

/// Side effects the event loop must run against the Session API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatAction {
    None,
    Quit,
    Send(String),
    Approve,
    Reject,
    AnswerPrompt(String),
    CancelPrompt,
    Session(SessionCommand),
    OpenSession(String),
}

/// How the chat front should leave when it is done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontExitKind {
    /// Backend already exposed an app handle — ask Tauri to exit (Exit restores).
    AppHandle,
    /// No handle yet — restore the terminal and exit the process.
    Process,
}

/// How long to wait for Tauri's graceful `RunEvent::Exit` before force-exiting.
pub const APP_HANDLE_EXIT_WAIT: Duration = Duration::from_secs(3);

/// Pick the exit path for the front thread. Pure so tests can cover G1.4 / G2.2.
pub fn decide_front_exit(has_app_handle: bool) -> FrontExitKind {
    if has_app_handle {
        FrontExitKind::AppHandle
    } else {
        FrontExitKind::Process
    }
}

/// True when the AppHandle path must wait for graceful Exit before `process::exit`.
pub fn app_handle_exit_waits(kind: FrontExitKind) -> bool {
    matches!(kind, FrontExitKind::AppHandle)
}

/// Request a clean exit. With an app handle, wait for `RunEvent::Exit` (restore +
/// unpublish) before falling back to `process::exit`.
fn leave_front() -> ! {
    let kind = decide_front_exit(crate::state::get_app_handle().is_some());
    if app_handle_exit_waits(kind) {
        if let Some(handle) = crate::state::get_app_handle() {
            handle.exit(0);
            // Let RunEvent::Exit restore the terminal and drop the backend
            // descriptor; only force-exit if that never arrives.
            std::thread::sleep(APP_HANDLE_EXIT_WAIT);
        }
    }
    crate::tui::restore_terminal();
    std::process::exit(0);
}

fn current_ask_answer(question: &UserQuestion, draft: &AskDraft) -> Result<Value, String> {
    let text = draft.text.trim();
    match question.options.as_ref() {
        None => {
            if text.is_empty() {
                return Err("Enter an answer".to_string());
            }
            Ok(Value::String(text.to_string()))
        }
        Some(_) if question.multi_select => {
            let mut values = draft.selected.clone();
            if question.allow_custom && draft.other_on {
                if text.is_empty() {
                    return Err("Enter text for Other".to_string());
                }
                values.push(text.to_string());
            }
            if values.is_empty() {
                return Err("Select at least one option (Space)".to_string());
            }
            Ok(Value::Array(
                values.into_iter().map(Value::String).collect(),
            ))
        }
        Some(_) => {
            if question.allow_custom && draft.other_on {
                if text.is_empty() {
                    return Err("Enter text for Other".to_string());
                }
                return Ok(Value::String(text.to_string()));
            }
            draft
                .selected
                .first()
                .cloned()
                .map(Value::String)
                .ok_or_else(|| "Select an option (Space)".to_string())
        }
    }
}

fn message_text(message: &Value) -> String {
    if let Some(text) = message.get("content").and_then(Value::as_str) {
        return text.to_string();
    }
    let Some(parts) = message.get("content").and_then(Value::as_array) else {
        return String::new();
    };
    parts
        .iter()
        .filter_map(|part| match part.get("type").and_then(Value::as_str) {
            Some("text") => part.get("text").and_then(Value::as_str).map(str::to_string),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Prefer a hard approval when several wait (parity §3.1 S5).
pub fn pick_pending_approval(list: Vec<PendingApprovalSummary>) -> Option<PendingApprovalSummary> {
    let hard = list.iter().find(|a| {
        matches!(
            a.approval_kind,
            crate::agent::state::PendingApprovalKind::Hard
        )
    });
    hard.cloned().or_else(|| list.into_iter().next())
}

/// Paragraph scroll offset that pins to the bottom when `from_bottom` is 0.
pub fn transcript_scroll_offset(content_rows: u16, view_rows: u16, from_bottom: u16) -> u16 {
    let max_scroll = content_rows.saturating_sub(view_rows);
    max_scroll.saturating_sub(from_bottom.min(max_scroll))
}

/// How many terminal rows `text` needs at `width`, counted by the very widget
/// that draws it (`Paragraph` + `Wrap { trim: false }`), so the count cannot
/// drift from the renderer on double-width text or word boundaries.
pub fn paragraph_row_count(text: &str, width: u16) -> u16 {
    Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .line_count(width.max(1))
        .try_into()
        .unwrap_or(u16::MAX)
}

fn s5_body_lines(state: &ChatState) -> Vec<String> {
    if let Some(approval) = &state.pending_approval {
        let focus = if state.focus == Focus::Prompt {
            "focused · a/r or Alt+Y/N · ↑↓ scroll"
        } else {
            "Alt+Y approve · Alt+N reject · Tab to focus"
        };
        let kind = match approval.approval_kind {
            crate::agent::state::PendingApprovalKind::Hard => "Hard approve",
            crate::agent::state::PendingApprovalKind::Standard => "Approve",
        };
        let mut lines = vec![format!("{kind} {}?", approval.tool_name)];
        for line in approval.command_preview.lines() {
            lines.push(line.to_string());
        }
        if approval.command_preview.is_empty() {
            lines.push(String::new());
        }
        lines.push(format!("[{focus}]"));
        return lines;
    }
    if let Some(prompt) = &state.pending_prompt {
        return match &prompt.payload {
            UserPromptPayload::Secret { label, .. } => {
                let masked: String = std::iter::repeat_n('*', state.prompt_input.len()).collect();
                vec![format!(
                    "Secret: {label}  [{masked}]  Enter submit · Esc cancel"
                )]
            }
            UserPromptPayload::Question { questions } => ask_user_lines(state, questions),
        };
    }
    Vec::new()
}

fn ask_user_lines(state: &ChatState, questions: &[UserQuestion]) -> Vec<String> {
    if questions.is_empty() {
        return vec!["Ask: (no questions)".to_string()];
    }
    let index = state.ask.index.min(questions.len() - 1);
    let question = &questions[index];
    let mut lines = vec![format!(
        "Ask ({}/{}): {}",
        index + 1,
        questions.len(),
        question.question
    )];
    if let Some(desc) = question.description.as_deref() {
        lines.push(desc.to_string());
    }
    if let Some(options) = question.options.as_ref() {
        for (i, opt) in options.iter().enumerate() {
            let marked = state.ask.selected.iter().any(|v| v == opt);
            let cursor = if state.ask.cursor == i { ">" } else { " " };
            let box_mark = if question.multi_select {
                if marked {
                    "[x]"
                } else {
                    "[ ]"
                }
            } else if marked {
                "(•)"
            } else {
                "( )"
            };
            lines.push(format!("{cursor} {box_mark} {opt}"));
        }
        if question.allow_custom {
            let cursor = if state.ask.cursor == options.len() {
                ">"
            } else {
                " "
            };
            let mark = if state.ask.other_on { "[x]" } else { "[ ]" };
            lines.push(format!("{cursor} {mark} Other: {}", state.ask.text));
        }
    } else {
        lines.push(format!("> {}", state.ask.text));
    }
    let hint = if index + 1 < questions.len() {
        "Space toggle · Enter next · ↑↓"
    } else {
        "Space toggle · Enter submit · ↑↓"
    };
    lines.push(format!("[{hint}]"));
    lines
}

/// S5 strip height and body text for `width`, with a clip marker when needed.
pub fn s5_layout(state: &ChatState, width: u16) -> (u16, String) {
    if !state.has_s5() {
        return (0, String::new());
    }
    let body = s5_body_lines(state).join("\n");
    let total = paragraph_row_count(&body, width);
    if total <= S5_MAX_ROWS {
        return (total.clamp(1, S5_MAX_ROWS), body);
    }
    // Reserve the last row for the marker; content scrolls in the rest.
    let content_rows = S5_MAX_ROWS.saturating_sub(1).max(1);
    let hidden = total.saturating_sub(content_rows);
    let marker = format!("… {hidden} more rows (Tab, ↑↓)");
    (S5_MAX_ROWS, format!("{body}\n{marker}"))
}

/// Draw the S1 chat frame (and S5 strip when a prompt is open).
pub fn draw_chat(frame: &mut Frame<'_>, state: &ChatState) {
    if let Some(picker) = &state.picker {
        draw_picker(frame, picker);
        return;
    }

    let width = frame.area().width.max(1);
    let (s5_rows, s5_text) = s5_layout(state, width);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(s5_rows),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let header = format!(" {} ", state.session_title);
    frame.render_widget(
        Paragraph::new(header).block(Block::default().borders(Borders::BOTTOM)),
        chunks[0],
    );

    let body: String = state
        .transcript
        .iter()
        .map(|line| format!("{}  {}", line.role, line.text))
        .collect::<Vec<_>>()
        .join("\n");
    let content_rows = paragraph_row_count(&body, chunks[1].width.max(1));
    let view_rows = chunks[1].height.max(1);
    let scroll_y = transcript_scroll_offset(content_rows, view_rows, state.scroll_from_bottom);
    frame.render_widget(
        Paragraph::new(body)
            .wrap(Wrap { trim: false })
            .scroll((scroll_y, 0))
            .block(Block::default().borders(Borders::NONE)),
        chunks[1],
    );

    if s5_rows > 0 {
        let clipped = s5_text.contains("more rows (Tab");
        if clipped {
            let (content, marker) = s5_text.rsplit_once('\n').unwrap_or((s5_text.as_str(), ""));
            let content_height = s5_rows.saturating_sub(1).max(1);
            let s5_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(content_height), Constraint::Length(1)])
                .split(chunks[2]);
            frame.render_widget(
                Paragraph::new(content)
                    .wrap(Wrap { trim: false })
                    .scroll((state.s5_scroll, 0)),
                s5_chunks[0],
            );
            frame.render_widget(Paragraph::new(marker), s5_chunks[1]);
        } else {
            frame.render_widget(
                Paragraph::new(s5_text)
                    .wrap(Wrap { trim: false })
                    .scroll((state.s5_scroll, 0)),
                chunks[2],
            );
        }
    }

    let composer_title = if state.focus == Focus::Composer {
        " composer "
    } else {
        " composer (Tab: prompt) "
    };
    frame.render_widget(
        Paragraph::new(format!("> {}", state.composer))
            .block(Block::default().borders(Borders::ALL).title(composer_title)),
        chunks[3],
    );

    let status = state.notice.as_deref().unwrap_or(state.status.as_str());
    frame.render_widget(Paragraph::new(format!(" {status}")), chunks[4]);
}

/// Run the chat UI on an already-prepared alternate screen.
pub fn run_chat_front(stdout: &mut Stdout) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("error: tokio runtime: {err}");
            leave_front();
        }
    };

    let mut terminal = match Terminal::new(CrosstermBackend::new(stdout)) {
        Ok(terminal) => terminal,
        Err(err) => {
            eprintln!("error: ratatui terminal: {err}");
            leave_front();
        }
    };

    let mut state = ChatState::new();
    let mut client: Option<SharedBackendClient> = None;
    let mut last_poll = Instant::now() - POLL_EVERY;

    loop {
        if client.is_none() {
            if let Some(descriptor) = crate::server::backend_descriptor::find_live_backend() {
                match SharedBackendClient::new(descriptor.port) {
                    Ok(c) => {
                        state.status = format!("Backend on port {}", descriptor.port);
                        client = Some(c);
                    }
                    Err(err) => state.status = err,
                }
            }
        }

        if last_poll.elapsed() >= POLL_EVERY {
            last_poll = Instant::now();
            if let Some(api) = client.as_ref() {
                if let Err(err) = runtime.block_on(api.ensure_attached()) {
                    state.notice = Some(err);
                } else {
                    runtime.block_on(poll_session(api, &mut state));
                }
            }
        }

        if terminal.draw(|frame| draw_chat(frame, &state)).is_err() {
            leave_front();
        }

        if crossterm::event::poll(Duration::from_millis(50)).unwrap_or(false) {
            if let Ok(crossterm::event::Event::Key(key)) = crossterm::event::read() {
                if key.kind != crossterm::event::KeyEventKind::Press
                    && key.kind != crossterm::event::KeyEventKind::Repeat
                {
                    continue;
                }
                match state.handle_key(key) {
                    ChatAction::Quit => leave_front(),
                    ChatAction::Send(text) => {
                        if let Some(api) = client.as_ref() {
                            runtime.block_on(send_user_text(api, &mut state, &text));
                        } else {
                            state.notice = Some("Backend is not ready yet".to_string());
                        }
                    }
                    ChatAction::Session(command) => {
                        if let Some(api) = client.as_ref() {
                            runtime.block_on(run_session_command(api, &mut state, command));
                        } else {
                            state.notice = Some("Backend is not ready yet".to_string());
                        }
                    }
                    ChatAction::OpenSession(id) => {
                        if let Some(api) = client.as_ref() {
                            runtime.block_on(switch_to_session(api, &mut state, &id));
                        } else {
                            state.notice = Some("Backend is not ready yet".to_string());
                        }
                    }
                    ChatAction::Approve => {
                        if let (Some(api), Some(approval)) =
                            (client.as_ref(), state.pending_approval.clone())
                        {
                            runtime.block_on(respond_approval(api, &mut state, &approval, true));
                        }
                    }
                    ChatAction::Reject => {
                        if let (Some(api), Some(approval)) =
                            (client.as_ref(), state.pending_approval.clone())
                        {
                            runtime.block_on(respond_approval(api, &mut state, &approval, false));
                        }
                    }
                    ChatAction::AnswerPrompt(value) => {
                        if let (Some(api), Some(prompt)) =
                            (client.as_ref(), state.pending_prompt.clone())
                        {
                            runtime.block_on(respond_user_prompt(
                                api,
                                &mut state,
                                &prompt.request_id,
                                "answer",
                                Some(value),
                            ));
                        }
                    }
                    ChatAction::CancelPrompt => {
                        if let (Some(api), Some(prompt)) =
                            (client.as_ref(), state.pending_prompt.clone())
                        {
                            runtime.block_on(respond_user_prompt(
                                api,
                                &mut state,
                                &prompt.request_id,
                                "cancel",
                                None,
                            ));
                        }
                    }
                    ChatAction::None => {}
                }
            }
        }
    }
}

async fn poll_session(api: &SharedBackendClient, state: &mut ChatState) {
    let Some(session_id) = state.session_id.clone() else {
        return;
    };
    match api.get_session(&session_id).await {
        Ok(session) => {
            if let Some(name) = session.get("name").and_then(Value::as_str) {
                if !name.is_empty() {
                    state.session_title = name.to_string();
                }
            }
            state.pending_approval = session
                .get("pendingApprovals")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .and_then(pick_pending_approval);
            state.pending_prompt = session
                .get("pendingUserPrompt")
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            state.sync_ask_with_prompt();
            if !state.has_s5() {
                state.focus = Focus::Composer;
            }
            let status = session
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            state.status = format!("{status} · session {session_id}");
        }
        Err(err) => state.notice = Some(err),
    }

    match api.get_messages(&session_id, Some(100)).await {
        Ok(payload) => {
            if let Some(messages) = payload.get("messages").and_then(Value::as_array) {
                state.apply_messages(messages);
            }
        }
        Err(err) => state.notice = Some(err),
    }
}

async fn send_user_text(api: &SharedBackendClient, state: &mut ChatState, text: &str) {
    state.notice = None;
    if state.session_id.is_none() {
        match ensure_session(api, text).await {
            Ok(id) => {
                state.session_id = Some(id);
                state.transcript.push(TranscriptLine {
                    role: "You".to_string(),
                    text: text.to_string(),
                });
            }
            Err(err) => state.notice = Some(err),
        }
        return;
    }
    let session_id = state.session_id.clone().expect("checked above");
    match api
        .send_message(&session_id, &json!({ "content": text }))
        .await
    {
        Ok(_) => {
            state.transcript.push(TranscriptLine {
                role: "You".to_string(),
                text: text.to_string(),
            });
        }
        Err(err) => state.notice = Some(err),
    }
}

async fn run_session_command(
    api: &SharedBackendClient,
    state: &mut ChatState,
    command: SessionCommand,
) {
    state.notice = None;
    match command {
        SessionCommand::New { assistant_id } => {
            match create_new_session(api, assistant_id.as_deref()).await {
                Ok((id, name)) => {
                    state.session_id = Some(id.clone());
                    state.session_title = name;
                    state.transcript.clear();
                    state.pending_approval = None;
                    state.pending_prompt = None;
                    state.scroll_from_bottom = 0;
                    state.status = format!("idle · session {id}");
                    state.notice = Some(format!("Started session {id}"));
                }
                Err(err) => state.notice = Some(err),
            }
        }
        SessionCommand::Resume {
            session_id: Some(id),
        } => {
            switch_to_session(api, state, &id).await;
        }
        SessionCommand::Resume { session_id: None } => match load_resume_picker(api).await {
            Ok(picker) => {
                if picker.rows.is_empty() {
                    state.notice = Some("No sessions to resume".to_string());
                } else {
                    state.picker = Some(picker);
                }
            }
            Err(err) => state.notice = Some(err),
        },
        SessionCommand::Rename { name } => {
            let Some(session_id) = state.session_id.clone() else {
                state.notice = Some("No open session to rename".to_string());
                return;
            };
            match rename_session(api, &session_id, &name).await {
                Ok(()) => {
                    state.session_title = name.clone();
                    state.notice = Some(format!("Renamed to {name}"));
                }
                Err(err) => state.notice = Some(err),
            }
        }
        SessionCommand::Delete { session_id } => {
            let Some(id) = session_id.or_else(|| state.session_id.clone()) else {
                state.notice = Some("No session to delete".to_string());
                return;
            };
            match delete_session(api, &id).await {
                Ok(()) => {
                    if state.session_id.as_deref() == Some(id.as_str()) {
                        state.session_id = None;
                        state.session_title = "New Session".to_string();
                        state.transcript.clear();
                        state.pending_approval = None;
                        state.pending_prompt = None;
                        state.status = "Session deleted".to_string();
                    }
                    state.notice = Some(format!("Deleted {id}"));
                }
                Err(err) => state.notice = Some(err),
            }
        }
    }
}

async fn switch_to_session(api: &SharedBackendClient, state: &mut ChatState, session_id: &str) {
    if let Err(err) = open_session_or_refuse(api, session_id).await {
        state.notice = Some(err);
        return;
    }
    // Resume a paused workflow when the session is not already driven.
    if let Ok(session) = api.get_session(session_id).await {
        let status = session
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if matches!(status, "paused" | "error") {
            if let Err(err) = api.resume_session(session_id).await {
                state.notice = Some(err);
                return;
            }
        }
        if let Some(name) = session.get("name").and_then(Value::as_str) {
            if !name.is_empty() {
                state.session_title = name.to_string();
            }
        }
    }
    state.session_id = Some(session_id.to_string());
    state.transcript.clear();
    state.pending_approval = None;
    state.pending_prompt = None;
    state.scroll_from_bottom = 0;
    state.notice = Some(format!("Opened {session_id}"));
    poll_session(api, state).await;
}

async fn ensure_session(api: &SharedBackendClient, first_message: &str) -> Result<String, String> {
    let assistants = api.list_assistants().await?;
    let list = assistants
        .as_array()
        .or_else(|| assistants.get("assistants").and_then(Value::as_array))
        .ok_or_else(|| "Unexpected /api/assistants response".to_string())?;
    let assistant_id = list
        .first()
        .and_then(|row| row.get("id").or_else(|| row.get("assistantId")))
        .and_then(Value::as_str)
        .ok_or_else(|| "No assistant configured — open the GUI once to create one".to_string())?
        .to_string();

    let created = api
        .create_session(&json!({
            "assistantId": assistant_id,
            "request": first_message,
            "name": "TUI chat",
        }))
        .await?;
    created
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "create_session response missing id".to_string())
}

async fn respond_approval(
    api: &SharedBackendClient,
    state: &mut ChatState,
    approval: &PendingApprovalSummary,
    approved: bool,
) {
    let Some(session_id) = state.session_id.clone() else {
        return;
    };
    let behavior = if approved { "allow" } else { "deny" };
    let body = json!({
        "toolCallId": approval.tool_call_id,
        "requestId": approval.request_id,
        "behavior": behavior,
    });
    match api.respond_permission(&session_id, &body).await {
        Ok(_) => {
            state.pending_approval = None;
            state.focus = Focus::Composer;
            state.s5_scroll = 0;
        }
        Err(err) => state.notice = Some(err),
    }
}

async fn respond_user_prompt(
    api: &SharedBackendClient,
    state: &mut ChatState,
    request_id: &str,
    action: &str,
    value: Option<String>,
) {
    let Some(session_id) = state.session_id.clone() else {
        return;
    };
    let mut body = json!({
        "requestId": request_id,
        "action": action,
    });
    if let Some(value) = value {
        body["value"] = Value::String(value);
    }
    match api.respond_prompt(&session_id, &body).await {
        Ok(_) => {
            state.pending_prompt = None;
            state.focus = Focus::Composer;
            state.ask = AskDraft::default();
        }
        Err(err) => state.notice = Some(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::state::PendingApprovalKind;
    use ratatui::backend::TestBackend;

    fn approval() -> PendingApprovalSummary {
        PendingApprovalSummary {
            tool_call_id: "call_1".to_string(),
            tool_name: "workspace__runPowerShell".to_string(),
            reason: "run".to_string(),
            command_preview: "pnpm test".to_string(),
            approval_kind: PendingApprovalKind::Standard,
            request_id: Some("req_1".to_string()),
        }
    }

    fn hard_approval() -> PendingApprovalSummary {
        PendingApprovalSummary {
            tool_call_id: "call_hard".to_string(),
            tool_name: "workspace__runDangerous".to_string(),
            reason: "hard".to_string(),
            command_preview: "rm -rf /".to_string(),
            approval_kind: PendingApprovalKind::Hard,
            request_id: Some("req_hard".to_string()),
        }
    }

    fn question_prompt(questions: Vec<UserQuestion>) -> PendingUserPromptSummary {
        PendingUserPromptSummary {
            request_id: "prompt_1".to_string(),
            kind: UserPromptKind::Question,
            payload: UserPromptPayload::Question { questions },
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn plain_enter_sends_composer_text() {
        let mut state = ChatState::new();
        state.composer = "hello world".to_string();
        assert_eq!(
            state.handle_key(KeyEvent::from(KeyCode::Enter)),
            ChatAction::Send("hello world".to_string())
        );
        assert!(state.composer.is_empty());
    }

    #[test]
    fn slash_slash_sends_a_literal_leading_slash() {
        let mut state = ChatState::new();
        state.composer = "//not-a-command".to_string();
        assert_eq!(
            state.handle_key(KeyEvent::from(KeyCode::Enter)),
            ChatAction::Send("/not-a-command".to_string())
        );
    }

    #[test]
    fn unknown_slash_command_is_not_sent() {
        let mut state = ChatState::new();
        state.composer = "/foo".to_string();
        assert_eq!(
            state.handle_key(KeyEvent::from(KeyCode::Enter)),
            ChatAction::None
        );
        assert!(state.notice.as_deref().unwrap_or("").contains("/foo"));
    }

    #[test]
    fn slash_new_emits_session_command() {
        let mut state = ChatState::new();
        state.composer = "/new".to_string();
        assert_eq!(
            state.handle_key(KeyEvent::from(KeyCode::Enter)),
            ChatAction::Session(SessionCommand::New { assistant_id: None })
        );
        assert!(state.composer.is_empty());
    }

    #[test]
    fn slash_delete_asks_for_confirm_on_open_session() {
        let mut state = ChatState::new();
        state.session_id = Some("sess_1".into());
        state.composer = "/delete".to_string();
        assert_eq!(
            state.handle_key(KeyEvent::from(KeyCode::Enter)),
            ChatAction::None
        );
        assert_eq!(state.confirm_delete.as_deref(), Some("sess_1"));
        assert!(state.notice.as_deref().unwrap_or("").contains("children"));
        assert_eq!(
            state.handle_key(KeyEvent::from(KeyCode::Char('y'))),
            ChatAction::Session(SessionCommand::Delete {
                session_id: Some("sess_1".into())
            })
        );
    }

    #[test]
    fn slash_delete_with_id_also_asks_for_confirm_and_n_cancels() {
        let mut state = ChatState::new();
        state.composer = "/delete other_sess".to_string();
        assert_eq!(
            state.handle_key(KeyEvent::from(KeyCode::Enter)),
            ChatAction::None
        );
        assert_eq!(state.confirm_delete.as_deref(), Some("other_sess"));
        assert_eq!(
            state.handle_key(KeyEvent::from(KeyCode::Char('n'))),
            ChatAction::None
        );
        assert!(state.confirm_delete.is_none());
        assert!(state.notice.as_deref().unwrap_or("").contains("cancelled"));
    }

    #[test]
    fn slash_rename_without_name_shows_usage() {
        let mut state = ChatState::new();
        state.composer = "/rename".to_string();
        assert_eq!(
            state.handle_key(KeyEvent::from(KeyCode::Enter)),
            ChatAction::None
        );
        assert!(state
            .notice
            .as_deref()
            .unwrap_or("")
            .contains("/rename <name>"));
    }

    #[test]
    fn alt_y_approves_without_prompt_focus() {
        let mut state = ChatState::new();
        state.pending_approval = Some(approval());
        state.focus = Focus::Composer;
        let mut key = KeyEvent::from(KeyCode::Char('y'));
        key.modifiers = KeyModifiers::ALT;
        assert_eq!(state.handle_key(key), ChatAction::Approve);
    }

    #[test]
    fn bare_a_does_not_approve_while_composer_focused() {
        let mut state = ChatState::new();
        state.pending_approval = Some(approval());
        state.focus = Focus::Composer;
        assert_eq!(
            state.handle_key(KeyEvent::from(KeyCode::Char('a'))),
            ChatAction::None
        );
        assert_eq!(state.composer, "a");
    }

    #[test]
    fn tab_then_a_approves() {
        let mut state = ChatState::new();
        state.pending_approval = Some(approval());
        assert_eq!(
            state.handle_key(KeyEvent::from(KeyCode::Tab)),
            ChatAction::None
        );
        assert_eq!(state.focus, Focus::Prompt);
        assert_eq!(
            state.handle_key(KeyEvent::from(KeyCode::Char('a'))),
            ChatAction::Approve
        );
    }

    #[test]
    fn apply_messages_reverses_api_newest_first_order() {
        let mut state = ChatState::new();
        let messages = vec![
            json!({"role": "assistant", "content": "second"}),
            json!({"role": "user", "content": "first"}),
        ];
        state.apply_messages(&messages);
        assert_eq!(
            state.transcript,
            vec![
                TranscriptLine {
                    role: "You".to_string(),
                    text: "first".to_string(),
                },
                TranscriptLine {
                    role: "AI".to_string(),
                    text: "second".to_string(),
                },
            ]
        );
    }

    #[test]
    fn pick_pending_approval_prefers_hard() {
        let picked = pick_pending_approval(vec![approval(), hard_approval()]).unwrap();
        assert_eq!(picked.tool_call_id, "call_hard");
        assert_eq!(picked.approval_kind, PendingApprovalKind::Hard);
    }

    #[test]
    fn transcript_scroll_follows_tail_and_up_moves_to_older() {
        // 10 content rows in a 4-row view → max scroll 6; from_bottom 0 pins to end.
        assert_eq!(transcript_scroll_offset(10, 4, 0), 6);
        assert_eq!(transcript_scroll_offset(10, 4, 2), 4);
        assert_eq!(transcript_scroll_offset(10, 4, 100), 0);

        let mut state = ChatState::new();
        state.handle_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(state.scroll_from_bottom, 1);
        state.handle_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(state.scroll_from_bottom, 2);
        state.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(state.scroll_from_bottom, 1);
        state.handle_key(KeyEvent::from(KeyCode::End));
        assert_eq!(state.scroll_from_bottom, 0);
    }

    #[test]
    fn decide_front_exit_uses_process_without_app_handle() {
        assert_eq!(decide_front_exit(false), FrontExitKind::Process);
        assert!(!app_handle_exit_waits(FrontExitKind::Process));
        assert_eq!(decide_front_exit(true), FrontExitKind::AppHandle);
        assert!(app_handle_exit_waits(FrontExitKind::AppHandle));
        assert!(APP_HANDLE_EXIT_WAIT > Duration::ZERO);
    }

    #[test]
    fn focus_defaults_to_composer() {
        assert_eq!(Focus::default(), Focus::Composer);
    }

    #[test]
    fn paragraph_row_count_counts_display_width_like_the_renderer() {
        // Double-width text: 204 Korean chars are 408 columns, so 80 columns
        // fit 40 per row — six rows, where a Unicode scalar count saw three.
        assert_eq!(paragraph_row_count(&"가".repeat(204), 80), 6);
        // ASCII keeps the previous counts.
        assert_eq!(paragraph_row_count(&"x".repeat(120), 80), 2);
        assert_eq!(paragraph_row_count(&"x".repeat(400), 80), 5);
        assert_eq!(paragraph_row_count("", 80), 1);
    }

    #[test]
    fn s5_layout_keeps_clip_marker_for_long_approval() {
        // Header + footer + ~10 wrapped command rows at width 80 → over S5_MAX_ROWS.
        let long = "x".repeat(800);
        let mut state = ChatState::new();
        state.pending_approval = Some(PendingApprovalSummary {
            command_preview: long,
            ..approval()
        });
        let (height, text) = s5_layout(&state, 80);
        assert_eq!(height, S5_MAX_ROWS);
        assert!(
            text.contains("more rows (Tab"),
            "clipped S5 must show a scroll hint, got: {text}"
        );
        assert!(
            text.contains("Approve workspace__runPowerShell?"),
            "header stays in the body"
        );
    }

    #[test]
    fn draw_chat_shows_full_long_approval_preview() {
        let long = "x".repeat(120);
        let mut state = ChatState::new();
        state.pending_approval = Some(PendingApprovalSummary {
            command_preview: long.clone(),
            ..approval()
        });
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| draw_chat(frame, &state))
            .expect("draw");
        let buffer = terminal.backend().buffer().clone();
        let rendered: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(
            rendered.contains(&long),
            "full command preview must appear without truncation"
        );
    }

    #[test]
    fn draw_chat_keeps_marker_and_newest_reply_visible() {
        let long = "x".repeat(800);
        let mut state = ChatState::new();
        state.pending_approval = Some(PendingApprovalSummary {
            command_preview: long,
            ..approval()
        });
        // One logical line that wraps to many rows — follow-tail must still
        // land on the newest content.
        state.transcript = vec![TranscriptLine {
            role: "AI".to_string(),
            text: format!("{}\nTAIL", "paragraph ".repeat(40)),
        }];
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| draw_chat(frame, &state))
            .expect("draw");
        let buffer = terminal.backend().buffer().clone();
        let rendered: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(
            rendered.contains("more rows (Tab"),
            "S5 clip marker must stay on screen"
        );
        assert!(
            rendered.contains("TAIL"),
            "follow-tail must keep the newest transcript row visible"
        );
    }

    #[test]
    fn draw_chat_keeps_newest_korean_reply_and_s5_footer_visible() {
        // Double-width text without spaces (the shipped ko locale): scalar
        // counting would miss the clip marker entirely and follow-tail would
        // stop short of the newest rows.
        let mut state = ChatState::new();
        state.pending_approval = Some(PendingApprovalSummary {
            command_preview: "한국어".repeat(100),
            ..approval()
        });
        state.transcript = vec![TranscriptLine {
            role: "AI".to_string(),
            text: format!("{}\nTAIL", "한국어 ".repeat(600)),
        }];
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| draw_chat(frame, &state))
            .expect("draw");
        let buffer = terminal.backend().buffer().clone();
        let rendered: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(
            rendered.contains("more rows (Tab"),
            "double-width approval body must be recognized as clipped"
        );
        assert!(
            rendered.contains("TAIL"),
            "follow-tail must keep the newest Korean row visible"
        );
    }

    #[test]
    fn ask_user_single_choice_submits_json_object() {
        let mut state = ChatState::new();
        state.pending_prompt = Some(question_prompt(vec![UserQuestion {
            id: "lang".to_string(),
            question: "Language?".to_string(),
            description: None,
            options: Some(vec!["Rust".to_string(), "TS".to_string()]),
            multi_select: false,
            allow_custom: false,
        }]));
        state.sync_ask_with_prompt();
        state.focus = Focus::Prompt;
        state.handle_key(KeyEvent::from(KeyCode::Char(' ')));
        let action = state.handle_key(KeyEvent::from(KeyCode::Enter));
        match action {
            ChatAction::AnswerPrompt(raw) => {
                let parsed: Value = serde_json::from_str(&raw).unwrap();
                assert_eq!(parsed["lang"], json!("Rust"));
            }
            other => panic!("expected AnswerPrompt, got {other:?}"),
        }
    }

    #[test]
    fn ask_user_multi_select_toggles_and_submits_array() {
        let mut state = ChatState::new();
        state.pending_prompt = Some(question_prompt(vec![UserQuestion {
            id: "targets".to_string(),
            question: "Targets?".to_string(),
            description: None,
            options: Some(vec![
                "linux".to_string(),
                "mac".to_string(),
                "win".to_string(),
            ]),
            multi_select: true,
            allow_custom: false,
        }]));
        state.sync_ask_with_prompt();
        state.focus = Focus::Prompt;
        state.handle_key(KeyEvent::from(KeyCode::Char(' '))); // linux
        state.handle_key(KeyEvent::from(KeyCode::Down));
        state.handle_key(KeyEvent::from(KeyCode::Char(' '))); // mac
        let action = state.handle_key(KeyEvent::from(KeyCode::Enter));
        match action {
            ChatAction::AnswerPrompt(raw) => {
                let parsed: Value = serde_json::from_str(&raw).unwrap();
                assert_eq!(parsed["targets"], json!(["linux", "mac"]));
            }
            other => panic!("expected AnswerPrompt, got {other:?}"),
        }
    }

    #[test]
    fn ask_user_two_questions_submit_together() {
        let mut state = ChatState::new();
        state.pending_prompt = Some(question_prompt(vec![
            UserQuestion {
                id: "name".to_string(),
                question: "Name?".to_string(),
                description: None,
                options: None,
                multi_select: false,
                allow_custom: false,
            },
            UserQuestion {
                id: "lang".to_string(),
                question: "Language?".to_string(),
                description: None,
                options: Some(vec!["Rust".to_string()]),
                multi_select: false,
                allow_custom: false,
            },
        ]));
        state.sync_ask_with_prompt();
        state.focus = Focus::Prompt;
        for ch in "demo".chars() {
            state.handle_key(KeyEvent::from(KeyCode::Char(ch)));
        }
        assert_eq!(
            state.handle_key(KeyEvent::from(KeyCode::Enter)),
            ChatAction::None
        );
        assert_eq!(state.ask.index, 1);
        state.handle_key(KeyEvent::from(KeyCode::Char(' ')));
        let action = state.handle_key(KeyEvent::from(KeyCode::Enter));
        match action {
            ChatAction::AnswerPrompt(raw) => {
                let parsed: Value = serde_json::from_str(&raw).unwrap();
                assert_eq!(parsed["name"], json!("demo"));
                assert_eq!(parsed["lang"], json!("Rust"));
            }
            other => panic!("expected AnswerPrompt, got {other:?}"),
        }
    }
}
