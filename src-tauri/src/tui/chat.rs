//! TUI chat core: transcript, composer, streaming poll, and S5 prompts (#2762).
//!
//! Drives the shared Session API from the TUI thread once the backend is live.
//! Layout and key rules follow `docs/specs/tui-parity.md` §3 (S1 + S5).

use crate::agent::session_manager::PendingApprovalSummary;
use crate::agent::state::{PendingUserPromptSummary, UserPromptKind, UserPromptPayload};
use crate::server::shared_backend::SharedBackendClient;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use serde_json::{json, Value};
use std::io::Stdout;
use std::time::{Duration, Instant};

const POLL_EVERY: Duration = Duration::from_millis(250);
const CTRL_C_QUIT_WINDOW: Duration = Duration::from_secs(2);

/// Where keyboard focus sits while an S5 prompt is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Composer,
    Prompt,
}

/// One line shown in the transcript (role + text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptLine {
    pub role: String,
    pub text: String,
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
    pub scroll: u16,
    last_ctrl_c: Option<Instant>,
}

impl Default for Focus {
    fn default() -> Self {
        Self::Composer
    }
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
        if ctrl && matches!(key.code, KeyCode::Char('d') | KeyCode::Char('D')) {
            if self.composer.is_empty() && self.focus == Focus::Composer {
                return ChatAction::Quit;
            }
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
                let outbound = if let Some(rest) = text.strip_prefix("//") {
                    format!("/{rest}")
                } else if text.starts_with('/') {
                    self.notice = Some(format!("Unknown command {text} (chat core: /help later)"));
                    return ChatAction::None;
                } else {
                    text
                };
                self.composer.clear();
                ChatAction::Send(outbound)
            }
            KeyCode::Backspace => {
                self.composer.pop();
                ChatAction::None
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.composer.push(ch);
                ChatAction::None
            }
            KeyCode::Up => {
                self.scroll = self.scroll.saturating_add(1);
                ChatAction::None
            }
            KeyCode::Down => {
                self.scroll = self.scroll.saturating_sub(1);
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

    fn has_s5(&self) -> bool {
        self.pending_approval.is_some() || self.pending_prompt.is_some()
    }

    pub fn apply_messages(&mut self, messages: &[Value]) {
        self.transcript.clear();
        for message in messages {
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

/// Draw the S1 chat frame (and S5 strip when a prompt is open).
pub fn draw_chat(frame: &mut Frame<'_>, state: &ChatState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(if state.has_s5() { 3 } else { 0 }),
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
    frame.render_widget(
        Paragraph::new(body)
            .wrap(Wrap { trim: false })
            .scroll((state.scroll, 0))
            .block(Block::default().borders(Borders::NONE)),
        chunks[1],
    );

    if state.has_s5() {
        frame.render_widget(Paragraph::new(s5_line(state)), chunks[2]);
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

fn s5_line(state: &ChatState) -> String {
    if let Some(approval) = &state.pending_approval {
        let focus = if state.focus == Focus::Prompt {
            "focused · a/r or Alt+Y/N"
        } else {
            "Alt+Y approve · Alt+N reject · Tab to focus"
        };
        return format!(
            " Approve {}? {}  [{}]",
            approval.tool_name,
            truncate(&approval.command_preview, 60),
            focus
        );
    }
    if let Some(prompt) = &state.pending_prompt {
        match &prompt.payload {
            UserPromptPayload::Secret { label, .. } => {
                let masked: String = std::iter::repeat_n('*', state.prompt_input.len()).collect();
                format!(" Secret: {label}  [{masked}]  Enter submit · Esc cancel")
            }
            UserPromptPayload::Question { questions } => {
                let q = questions
                    .first()
                    .map(|q| q.question.as_str())
                    .unwrap_or("Question");
                format!(" Ask: {q}  [{}]  Enter submit", state.prompt_input)
            }
        }
    } else {
        String::new()
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let clipped: String = text.chars().take(max.saturating_sub(1)).collect();
        format!("{clipped}…")
    }
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
            return;
        }
    };

    let mut terminal = match Terminal::new(CrosstermBackend::new(stdout)) {
        Ok(terminal) => terminal,
        Err(err) => {
            eprintln!("error: ratatui terminal: {err}");
            return;
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
            if let Some(ref api) = client {
                if let Err(err) = runtime.block_on(api.ensure_attached()) {
                    state.notice = Some(err);
                } else {
                    runtime.block_on(poll_session(api, &mut state));
                }
            }
        }

        if terminal.draw(|frame| draw_chat(frame, &state)).is_err() {
            break;
        }

        if crossterm::event::poll(Duration::from_millis(50)).unwrap_or(false) {
            if let Ok(crossterm::event::Event::Key(key)) = crossterm::event::read() {
                if key.kind != crossterm::event::KeyEventKind::Press
                    && key.kind != crossterm::event::KeyEventKind::Repeat
                {
                    continue;
                }
                match state.handle_key(key) {
                    ChatAction::Quit => {
                        if let Some(handle) = crate::state::get_app_handle() {
                            handle.exit(0);
                        }
                        return;
                    }
                    ChatAction::Send(text) => {
                        if let Some(ref api) = client {
                            runtime.block_on(send_user_text(api, &mut state, &text));
                        } else {
                            state.notice = Some("Backend is not ready yet".to_string());
                        }
                    }
                    ChatAction::Approve => {
                        if let (Some(ref api), Some(approval)) =
                            (client.as_ref(), state.pending_approval.clone())
                        {
                            runtime.block_on(respond_approval(api, &mut state, &approval, true));
                        }
                    }
                    ChatAction::Reject => {
                        if let (Some(ref api), Some(approval)) =
                            (client.as_ref(), state.pending_approval.clone())
                        {
                            runtime.block_on(respond_approval(api, &mut state, &approval, false));
                        }
                    }
                    ChatAction::AnswerPrompt(value) => {
                        if let (Some(ref api), Some(prompt)) =
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
                        if let (Some(ref api), Some(prompt)) =
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
                .and_then(|list: Vec<PendingApprovalSummary>| {
                    // Hard approvals first (parity).
                    let hard = list.iter().find(|a| {
                        matches!(
                            a.approval_kind,
                            crate::agent::state::PendingApprovalKind::Hard
                        ) // hard approvals first (parity)
                    });
                    hard.cloned().or_else(|| list.into_iter().next())
                });
            state.pending_prompt = session
                .get("pendingUserPrompt")
                .and_then(|v| serde_json::from_value(v.clone()).ok());
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
        }
        Err(err) => state.notice = Some(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::state::PendingApprovalKind;

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
}
