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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEndReason {
    Delete,
    ClearAll,
    FactoryReset,
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
    pub stop_hook_active: bool,
    pub child_session_id: Option<String>,
    pub agent_id: Option<String>,
    pub compact_source: Option<String>,
    pub compact_trigger: Option<String>,
    pub custom_compact_instructions: Option<String>,
    pub notification_message: Option<String>,
    pub notification_type: Option<String>,
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
            stop_hook_active: false,
            child_session_id: None,
            agent_id: None,
            compact_source: None,
            compact_trigger: None,
            custom_compact_instructions: None,
            notification_message: None,
            notification_type: None,
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
        HookEvent::Stop => {
            obj.insert(
                "stop_hook_active".to_string(),
                json!(params.stop_hook_active),
            );
        }
        HookEvent::SubagentStop => {
            if let Some(id) = &params.agent_id {
                obj.insert("agent_id".to_string(), json!(id));
            }
            if let Some(id) = &params.child_session_id {
                obj.insert("child_session_id".to_string(), json!(id));
            }
        }
        HookEvent::PreCompact => {
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
    fn all_stub_events_include_extra_fields() {
        let cases: Vec<(HookEvent, fn(&mut HookEnvelopeParams))> = vec![
            (HookEvent::SessionEnd, |p| {
                p.session_end_reason = Some(SessionEndReason::Delete)
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
            (HookEvent::Stop, |p| p.stop_hook_active = true),
            (HookEvent::SubagentStop, |p| {
                p.agent_id = Some("agent-9".into());
                p.child_session_id = Some("child-1".into());
            }),
            (HookEvent::PreCompact, |p| {
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
