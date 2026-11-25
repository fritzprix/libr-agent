//! Chat state, transcript rows, composer drafts, and the actions the event
//! loop must run (#2886).

use super::render::{current_ask_answer, message_text};
use crate::agent::session_manager::PendingApprovalSummary;
use crate::agent::state::{
    PendingUserPromptSummary, UserPromptKind, UserPromptPayload, UserQuestion,
};
use crate::tui::sessions::{parse_session_command, PickerAction, SessionCommand, SessionPicker};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::{Map, Value};
use std::time::{Duration, Instant};

pub(super) const POLL_EVERY: Duration = Duration::from_millis(250);
pub(super) const CTRL_C_QUIT_WINDOW: Duration = Duration::from_secs(2);
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
pub(super) struct AskDraft {
    pub(super) request_id: Option<String>,
    pub(super) index: usize,
    /// Highlighted option row (0..options.len(), or options.len() for Other).
    pub(super) cursor: usize,
    pub(super) selected: Vec<String>,
    pub(super) other_on: bool,
    pub(super) text: String,
    /// Answers for questions already confirmed with Enter.
    pub(super) committed: Map<String, Value>,
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
    pub(super) ask: AskDraft,
    /// S2 session picker (`/resume` / `/history`).
    pub picker: Option<SessionPicker>,
    /// Pending `/delete` confirmation (session id).
    pub confirm_delete: Option<String>,
    pub(super) last_ctrl_c: Option<Instant>,
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

    pub(super) fn handle_ctrl_c(&mut self) -> ChatAction {
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

    pub(super) fn handle_composer_key(&mut self, key: KeyEvent) -> ChatAction {
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

    pub(super) fn handle_prompt_key(&mut self, key: KeyEvent) -> ChatAction {
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

    pub(super) fn is_ask_user(&self) -> bool {
        matches!(
            self.pending_prompt.as_ref().map(|p| &p.payload),
            Some(UserPromptPayload::Question { .. })
        )
    }

    pub(super) fn ask_questions(&self) -> Option<&[UserQuestion]> {
        match self.pending_prompt.as_ref().map(|p| &p.payload) {
            Some(UserPromptPayload::Question { questions }) => Some(questions.as_slice()),
            _ => None,
        }
    }

    pub(super) fn handle_ask_key(&mut self, key: KeyEvent) -> ChatAction {
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

    pub(super) fn toggle_ask_row(&mut self, question: &UserQuestion) {
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

    pub(super) fn advance_or_submit_ask(&mut self, questions: &[UserQuestion]) -> ChatAction {
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

    pub(super) fn has_s5(&self) -> bool {
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
