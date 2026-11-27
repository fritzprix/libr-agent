use crate::services::hooks::schema::HookEvent;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HookPlatform {
    Windows,
    Macos,
    Linux,
}

impl HookPlatform {
    pub fn host() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else {
            Self::Linux
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStartSource {
    Startup,
    Resume,
    Clear,
    Compact,
}

/// Why a session ended, using the Claude Code `reason` values (#3005).
///
/// Claude Code sends `clear`, `resume`, `logout`, `prompt_input_exit`, and
/// `other`. LibrAgent has no login session (`logout`), no interactive session
/// switch (`resume`), and no prompt-input exit path of its own, so only two
/// reasons fire today:
///
/// - `clear` — `/clear` wiped the transcript and context, the session stays.
/// - `other` — the session row was deleted, either for one session or by
///   "clear all sessions" / "reset all data". Claude Code has no per-session
///   delete, so these map to `other`.
///
/// <https://code.claude.com/docs/en/hooks#sessionend>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEndReason {
    Clear,
    Other,
}

impl SessionEndReason {
    /// The value a `SessionEnd` matcher is compared against.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone)]
pub struct HookEnvelopeParams {
    pub session_id: String,
    pub cwd: PathBuf,
    pub event: HookEvent,
    pub matcher: Option<String>,
    pub transcript_path: Option<PathBuf>,
    pub platform: HookPlatform,
    pub session_start_source: Option<SessionStartSource>,
    pub session_end_reason: Option<SessionEndReason>,
    pub prompt: Option<String>,
    pub tool_name: Option<String>,
    pub tool_input: Option<Value>,
    pub tool_response: Option<Value>,
    /// Failure message for `PostToolUseFailure` (#3006).
    pub tool_error: Option<String>,
    /// Whether the failure in `PostToolUseFailure` was caused by user interrupt/cancellation (#3006).
    pub is_interrupt: Option<bool>,
    /// Reason a tool call was denied for `PermissionDenied` (#3006).
    pub denial_reason: Option<String>,
    pub stop_hook_active: bool,
    pub child_session_id: Option<String>,
    pub agent_id: Option<String>,
    /// Agent name used for `SubagentStop` matcher filtering (#3003 G1.5).
    /// `agent_id` is the unique subagent instance; `agent_type` is the name.
    pub agent_type: Option<String>,
    pub compact_source: Option<String>,
    pub compact_trigger: Option<String>,
    pub custom_compact_instructions: Option<String>,
    pub notification_message: Option<String>,
    pub notification_type: Option<String>,
    /// Error class a `StopFailure` fired for (e.g. `rate_limit`), matched by
    /// the event matcher and surfaced in the envelope (#3007).
    pub error_type: Option<String>,
}

impl HookEnvelopeParams {
    pub fn new(session_id: impl Into<String>, cwd: PathBuf, event: HookEvent) -> Self {
        Self {
            session_id: session_id.into(),
            cwd,
            event,
            matcher: None,
            transcript_path: None,
            platform: HookPlatform::host(),
            session_start_source: None,
            session_end_reason: None,
            prompt: None,
            tool_name: None,
            tool_input: None,
            tool_response: None,
            tool_error: None,
            is_interrupt: None,
            denial_reason: None,
            stop_hook_active: false,
            child_session_id: None,
            agent_id: None,
            agent_type: None,
            compact_source: None,
            compact_trigger: None,
            custom_compact_instructions: None,
            notification_message: None,
            notification_type: None,
            error_type: None,
        }
    }
}

pub fn build_envelope_value(params: &HookEnvelopeParams) -> Value {
    let event_name = params.event.as_str();
    let cwd = params.cwd.to_string_lossy().into_owned();
    let transcript_path = params
        .transcript_path
        .as_ref()
        .map(|p| Value::String(p.to_string_lossy().into_owned()))
        .unwrap_or(Value::Null);

    let mut envelope = json!({
        "session_id": params.session_id,
        "cwd": cwd,
        "hook_event_name": event_name,
        "event": event_name,
        "matcher": params.matcher,
        "transcript_path": transcript_path,
        "platform": params.platform,
    });

    let obj = envelope
        .as_object_mut()
        .expect("envelope root is always an object");

    match params.event {
        HookEvent::SessionStart => {
            if let Some(source) = params.session_start_source {
                obj.insert("source".to_string(), json!(source));
            }
        }
        HookEvent::SessionEnd => {
            if let Some(reason) = params.session_end_reason {
                obj.insert("reason".to_string(), json!(reason));
            }
        }
        HookEvent::UserPromptSubmit => {
            if let Some(prompt) = &params.prompt {
                obj.insert("prompt".to_string(), json!(prompt));
            }
        }
        HookEvent::PreToolUse => {
            if let Some(name) = &params.tool_name {
                obj.insert("tool_name".to_string(), json!(name));
            }
            if let Some(input) = &params.tool_input {
                obj.insert("tool_input".to_string(), input.clone());
            }
        }
        HookEvent::PostToolUse => {
            if let Some(name) = &params.tool_name {
                obj.insert("tool_name".to_string(), json!(name));
            }
            if let Some(input) = &params.tool_input {
                obj.insert("tool_input".to_string(), input.clone());
            }
            if let Some(response) = &params.tool_response {
                obj.insert("tool_response".to_string(), response.clone());
            }
        }
        HookEvent::PostToolUseFailure => {
            if let Some(name) = &params.tool_name {
                obj.insert("tool_name".to_string(), json!(name));
            }
            if let Some(input) = &params.tool_input {
                obj.insert("tool_input".to_string(), input.clone());
            }
            if let Some(err) = &params.tool_error {
                obj.insert("error".to_string(), json!(err));
            }
            obj.insert(
                "is_interrupt".to_string(),
                json!(params.is_interrupt.unwrap_or(false)),
            );
            if let Some(response) = &params.tool_response {
                obj.insert("tool_response".to_string(), response.clone());
            }
        }
        HookEvent::PermissionRequest => {
            if let Some(name) = &params.tool_name {
                obj.insert("tool_name".to_string(), json!(name));
            }
            if let Some(input) = &params.tool_input {
                obj.insert("tool_input".to_string(), input.clone());
            }
        }
        HookEvent::PermissionDenied => {
            if let Some(name) = &params.tool_name {
                obj.insert("tool_name".to_string(), json!(name));
            }
            if let Some(input) = &params.tool_input {
                obj.insert("tool_input".to_string(), input.clone());
            }
            if let Some(reason) = &params.denial_reason {
                obj.insert("reason".to_string(), json!(reason));
            }
        }
        HookEvent::PostToolBatch => {}
        HookEvent::Stop => {
            obj.insert(
                "stop_hook_active".to_string(),
                json!(params.stop_hook_active),
            );
        }
        HookEvent::StopFailure => {
            if let Some(kind) = &params.error_type {
                obj.insert("error_type".to_string(), json!(kind));
            }
            if let Some(id) = &params.agent_id {
                obj.insert("agent_id".to_string(), json!(id));
            }
            if let Some(kind) = &params.agent_type {
                obj.insert("agent_type".to_string(), json!(kind));
            }
            if let Some(id) = &params.child_session_id {
                obj.insert("child_session_id".to_string(), json!(id));
            }
        }
        HookEvent::SubagentStart | HookEvent::SubagentStop => {
            if let Some(id) = &params.agent_id {
                obj.insert("agent_id".to_string(), json!(id));
            }
            if let Some(kind) = &params.agent_type {
                obj.insert("agent_type".to_string(), json!(kind));
            }
            if let Some(id) = &params.child_session_id {
                obj.insert("child_session_id".to_string(), json!(id));
            }
        }
        HookEvent::PreCompact | HookEvent::PostCompact => {
            if let Some(source) = &params.compact_source {
                obj.insert("source".to_string(), json!(source));
            }
            if let Some(trigger) = &params.compact_trigger {
                obj.insert("trigger".to_string(), json!(trigger));
            }
            if let Some(instr) = &params.custom_compact_instructions {
                obj.insert("custom_instructions".to_string(), json!(instr));
            }
        }
        HookEvent::Notification => {
            if let Some(msg) = &params.notification_message {
                obj.insert("message".to_string(), json!(msg));
            }
            if let Some(kind) = &params.notification_type {
                obj.insert("notification_type".to_string(), json!(kind));
            }
        }
    }

    envelope
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn base_params(event: HookEvent) -> HookEnvelopeParams {
        HookEnvelopeParams::new("sess-1", PathBuf::from("/workspace"), event)
    }

    #[test]
    fn common_envelope_fields_present() {
        let value = build_envelope_value(&base_params(HookEvent::SessionStart));
        assert_eq!(value["session_id"], "sess-1");
        assert_eq!(value["hook_event_name"], "SessionStart");
        assert_eq!(value["event"], "SessionStart");
        assert!(value.get("transcript_path").is_some());
        assert!(value["transcript_path"].is_null());
    }

    #[test]
    fn session_start_includes_source() {
        let mut p = base_params(HookEvent::SessionStart);
        p.session_start_source = Some(SessionStartSource::Startup);
        p.matcher = Some("startup|compact".to_string());
        let value = build_envelope_value(&p);
        assert_eq!(value["source"], "startup");
        assert_eq!(value["matcher"], "startup|compact");
    }

    #[test]
    fn session_end_reasons_use_the_claude_code_values() {
        let mut params =
            HookEnvelopeParams::new("s1", PathBuf::from("/tmp"), HookEvent::SessionEnd);
        params.session_end_reason = Some(SessionEndReason::Clear);
        let value = build_envelope_value(&params);
        assert_eq!(value["reason"], "clear");

        params.session_end_reason = Some(SessionEndReason::Other);
        let value = build_envelope_value(&params);
        assert_eq!(value["reason"], "other");

        // The matcher value a plugin filters on is the same string.
        assert_eq!(SessionEndReason::Clear.as_str(), "clear");
        assert_eq!(SessionEndReason::Other.as_str(), "other");
    }

    #[test]
    fn subagent_stop_includes_agent_type_and_instance_id() {
        let mut p = base_params(HookEvent::SubagentStop);
        p.agent_id = Some("child-1".into());
        p.agent_type = Some("my-plugin:reviewer".into());
        p.child_session_id = Some("child-1".into());
        let value = build_envelope_value(&p);
        assert_eq!(value["agent_type"], "my-plugin:reviewer");
        assert_eq!(value["agent_id"], "child-1");
    }

    #[test]
    fn all_stub_events_include_extra_fields() {
        let cases: Vec<(HookEvent, fn(&mut HookEnvelopeParams))> = vec![
            (HookEvent::SessionEnd, |p| {
                p.session_end_reason = Some(SessionEndReason::Other)
            }),
            (HookEvent::UserPromptSubmit, |p| {
                p.prompt = Some("hello".into());
            }),
            (HookEvent::PreToolUse, |p| {
                p.tool_name = Some("workspace__readFile".into());
                p.tool_input = Some(json!({"path": "a.txt"}));
            }),
            (HookEvent::PostToolUse, |p| {
                p.tool_name = Some("workspace__readFile".into());
                p.tool_input = Some(json!({"path": "a.txt"}));
                p.tool_response = Some(json!({"content": "ok"}));
            }),
            (HookEvent::PostToolUseFailure, |p| {
                p.tool_name = Some("workspace__runShell".into());
                p.tool_input = Some(json!({"command": "false"}));
                p.tool_error = Some("exit code 1".into());
                p.is_interrupt = Some(false);
            }),
            (HookEvent::PermissionRequest, |p| {
                p.tool_name = Some("workspace__runShell".into());
                p.tool_input = Some(json!({"command": "rm -rf /tmp/x"}));
            }),
            (HookEvent::PermissionDenied, |p| {
                p.tool_name = Some("workspace__runShell".into());
                p.tool_input = Some(json!({"command": "rm -rf /tmp/x"}));
                p.denial_reason = Some("blocked by policy".into());
            }),
            (HookEvent::PostToolBatch, |_| {}),
            (HookEvent::Stop, |p| p.stop_hook_active = true),
            (HookEvent::SubagentStop, |p| {
                p.agent_id = Some("agent-9".into());
                p.agent_type = Some("my-plugin:reviewer".into());
                p.child_session_id = Some("child-1".into());
            }),
            (HookEvent::SubagentStart, |p| {
                p.agent_id = Some("agent-9".into());
                p.agent_type = Some("my-plugin:reviewer".into());
                p.child_session_id = Some("child-1".into());
            }),
            (HookEvent::StopFailure, |p| {
                p.error_type = Some("rate_limit".into());
                p.agent_id = Some("agent-9".into());
                p.agent_type = Some("my-plugin:reviewer".into());
                p.child_session_id = Some("child-1".into());
            }),
            (HookEvent::PreCompact, |p| {
                p.compact_source = Some("auto".into());
                p.compact_trigger = Some("manual".into());
                p.custom_compact_instructions = Some("keep todos".into());
            }),
            (HookEvent::PostCompact, |p| {
                p.compact_source = Some("auto".into());
                p.compact_trigger = Some("manual".into());
                p.custom_compact_instructions = Some("keep todos".into());
            }),
            (HookEvent::Notification, |p| {
                p.notification_message = Some("idle".into());
                p.notification_type = Some("permission".into());
            }),
        ];

        for (event, fill) in cases {
            let mut p = base_params(event);
            fill(&mut p);
            let value = build_envelope_value(&p);
            assert_eq!(value["event"], event.as_str(), "event {event:?}");
            assert!(value.is_object());
        }
    }
}
