use crate::services::hooks::schema::HookEvent;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookDecision {
    Allow,
    Block,
    Ask,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HookOutput {
    pub additional_context: Option<String>,
    pub decision: Option<HookDecision>,
    pub reason: Option<String>,
    /// `continue: false` stops processing after this hook (#3004).
    pub continue_loop: Option<bool>,
    /// `hookSpecificOutput.updatedInput` (#3005): the tool arguments a
    /// `PreToolUse` hook wants the call to run with. It replaces the whole
    /// input object, exactly as Claude Code applies it.
    pub updated_input: Option<serde_json::Value>,
    /// `stopReason`: the text shown to the user when `continue` is `false` (#3004).
    pub stop_reason: Option<String>,
    /// `hookSpecificOutput.displayContent`: the text a `MessageDisplay` hook
    /// wants shown instead of the message. Display only.
    pub display_content: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawHookStdout {
    #[serde(default, rename = "continue")]
    continue_loop: Option<bool>,
    /// `stopReason`; the camelCase rename above covers the documented spelling.
    stop_reason: Option<String>,
    additional_context: Option<String>,
    #[serde(rename = "additional_context")]
    additional_context_snake: Option<String>,
    decision: Option<String>,
    reason: Option<String>,
    hook_specific_output: Option<RawHookSpecificOutput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawHookSpecificOutput {
    additional_context: Option<String>,
    permission_decision: Option<String>,
    permission_decision_reason: Option<String>,
    updated_input: Option<serde_json::Value>,
    /// Claude Code `PermissionRequest` decision object:
    /// `{ "behavior": "allow" | "deny" | "ask", "message": "...", "updatedInput": {...} }` (#3006).
    decision: Option<RawPermissionDecision>,
    display_content: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPermissionDecision {
    behavior: Option<String>,
    message: Option<String>,
    updated_input: Option<serde_json::Value>,
}

/// Normalize hook stdout into structured output (alias for callers).
pub fn normalize(stdout: &str, event: HookEvent) -> HookOutput {
    parse_hook_stdout(event, stdout)
}

pub fn parse_hook_stdout(event: HookEvent, stdout: &str) -> HookOutput {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return HookOutput::default();
    }

    if let Ok(raw) = serde_json::from_str::<RawHookStdout>(trimmed) {
        return normalize_raw(raw);
    }

    if event.plain_stdout_is_context() {
        return HookOutput {
            additional_context: Some(trimmed.to_string()),
            ..HookOutput::default()
        };
    }

    HookOutput::default()
}

fn normalize_raw(raw: RawHookStdout) -> HookOutput {
    let additional_context = raw
        .hook_specific_output
        .as_ref()
        .and_then(|h| h.additional_context.clone())
        .or(raw.additional_context)
        .or(raw.additional_context_snake);

    let decision = raw
        .hook_specific_output
        .as_ref()
        .and_then(|h| {
            h.permission_decision
                .as_deref()
                .or_else(|| h.decision.as_ref().and_then(|d| d.behavior.as_deref()))
        })
        .or(raw.decision.as_deref())
        .and_then(parse_decision);

    let reason = raw
        .hook_specific_output
        .as_ref()
        .and_then(|h| {
            h.permission_decision_reason
                .clone()
                .or_else(|| h.decision.as_ref().and_then(|d| d.message.clone()))
        })
        .or(raw.reason);

    let continue_loop = raw.continue_loop;
    // `stopReason` only means something together with `continue: false`.
    let stop_reason = if continue_loop == Some(false) {
        raw.stop_reason
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty())
    } else {
        None
    };

    // `updatedInput` replaces the whole tool-argument object, and every
    // guard and tool assumes that object shape. A string, number, or array
    // would make key-reading policy checks see nothing, so only a JSON
    // object is accepted; anything else is ignored with a visible warning
    // (#3005 gatekeeper G1.4).
    let updated_input = raw
        .hook_specific_output
        .as_ref()
        .and_then(|h| {
            h.updated_input
                .clone()
                .or_else(|| h.decision.as_ref().and_then(|d| d.updated_input.clone()))
        })
        .filter(|value| {
            if value.is_object() {
                true
            } else {
                log::warn!(
                    "Ignoring PreToolUse updatedInput of type {} — expected a JSON object",
                    match value {
                        serde_json::Value::Null => "null",
                        serde_json::Value::Bool(_) => "boolean",
                        serde_json::Value::Number(_) => "number",
                        serde_json::Value::String(_) => "string",
                        serde_json::Value::Array(_) => "array",
                        serde_json::Value::Object(_) => "object",
                    }
                );
                false
            }
        });

    let display_content = raw
        .hook_specific_output
        .as_ref()
        .and_then(|h| h.display_content.clone());

    HookOutput {
        display_content,
        additional_context,
        decision,
        reason,
        continue_loop,
        updated_input,
        stop_reason,
    }
}

fn parse_decision(raw: &str) -> Option<HookDecision> {
    match raw.to_ascii_lowercase().as_str() {
        "allow" => Some(HookDecision::Allow),
        "block" | "deny" => Some(HookDecision::Block),
        "ask" => Some(HookDecision::Ask),
        _ => None,
    }
}

/// Parse stdout JSON without an event context (no plain-text fallback).
pub fn parse_hook_stdout_json(stdout: &str) -> HookOutput {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return HookOutput::default();
    }
    serde_json::from_str::<RawHookStdout>(trimmed)
        .map(normalize_raw)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_hook_specific_output_context() {
        let out = parse_hook_stdout_json(
            r#"{"additionalContext":"outer","hookSpecificOutput":{"additionalContext":"inner"}}"#,
        );
        assert_eq!(out.additional_context.as_deref(), Some("inner"));
    }

    #[test]
    fn accepts_snake_case_additional_context() {
        let out = parse_hook_stdout_json(r#"{"additional_context":"from cursor"}"#);
        assert_eq!(out.additional_context.as_deref(), Some("from cursor"));
    }

    #[test]
    fn maps_deny_to_block_and_reads_reason() {
        let out = parse_hook_stdout_json(
            r#"{"hookSpecificOutput":{"permissionDecision":"deny","permissionDecisionReason":"nope"}}"#,
        );
        assert_eq!(out.decision, Some(HookDecision::Block));
        assert_eq!(out.reason.as_deref(), Some("nope"));
    }

    #[test]
    fn top_level_decision_and_continue() {
        let out =
            parse_hook_stdout_json(r#"{"decision":"ask","reason":"confirm","continue":false}"#);
        assert_eq!(out.decision, Some(HookDecision::Ask));
        assert_eq!(out.reason.as_deref(), Some("confirm"));
        assert_eq!(out.continue_loop, Some(false));
    }

    #[test]
    fn parses_stop_reason_with_continue_false() {
        let out =
            parse_hook_stdout_json(r#"{"continue":false,"stopReason":"  budget exhausted  "}"#);
        assert_eq!(out.continue_loop, Some(false));
        assert_eq!(out.stop_reason.as_deref(), Some("budget exhausted"));
    }

    #[test]
    fn stop_reason_without_continue_false_is_ignored() {
        let out = parse_hook_stdout_json(r#"{"continue":true,"stopReason":"ignored"}"#);
        assert_eq!(out.continue_loop, Some(true));
        assert_eq!(out.stop_reason, None);

        let out = parse_hook_stdout_json(r#"{"stopReason":"ignored"}"#);
        assert_eq!(out.continue_loop, None);
        assert_eq!(out.stop_reason, None);
    }

    #[test]
    fn empty_stop_reason_is_dropped() {
        let out = parse_hook_stdout_json(r#"{"continue":false,"stopReason":"   "}"#);
        assert_eq!(out.continue_loop, Some(false));
        assert_eq!(out.stop_reason, None);
    }

    #[test]
    fn session_start_plain_text_is_context() {
        let out = parse_hook_stdout(HookEvent::SessionStart, "<EXTREMELY_IMPORTANT>\nbody");
        assert_eq!(
            out.additional_context.as_deref(),
            Some("<EXTREMELY_IMPORTANT>\nbody")
        );
    }

    #[test]
    fn non_session_start_plain_text_yields_empty_output() {
        let out = parse_hook_stdout(HookEvent::PreToolUse, "not json");
        assert_eq!(out, HookOutput::default());
    }

    #[test]
    fn superpowers_style_output_shape() {
        let json = r#"{
          "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": "bootstrap body"
          }
        }"#;
        let out = parse_hook_stdout(HookEvent::SessionStart, json);
        assert_eq!(out.additional_context.as_deref(), Some("bootstrap body"));
    }

    #[test]
    fn reads_updated_input_from_hook_specific_output() {
        let out = parse_hook_stdout_json(
            r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow","updatedInput":{"path":"b.txt","line":7}}}"#,
        );
        assert_eq!(out.decision, Some(HookDecision::Allow));
        assert_eq!(
            out.updated_input,
            Some(serde_json::json!({"path": "b.txt", "line": 7}))
        );
    }

    #[test]
    fn updated_input_absent_parses_as_none() {
        let out =
            parse_hook_stdout_json(r#"{"hookSpecificOutput":{"permissionDecision":"allow"}}"#);
        assert_eq!(out.updated_input, None);
    }

    /// #3005 gatekeeper G1.4: only a JSON object may replace the tool
    /// arguments. A string, number, or array makes every key-reading guard
    /// see nothing, so it is ignored (the call keeps its original arguments).
    #[test]
    fn non_object_updated_input_is_ignored() {
        for value in [r#""rm -rf /""#, "42", "true", r#"["cmd","rm"]"#, "null"] {
            let json = format!(
                r#"{{"hookSpecificOutput":{{"permissionDecision":"allow","updatedInput":{value}}}}}"#
            );
            let out = parse_hook_stdout_json(&json);
            assert_eq!(
                out.updated_input, None,
                "updatedInput {value} must be ignored, not applied"
            );
            // The allow decision itself still applies — only the rewrite is dropped.
            assert_eq!(out.decision, Some(HookDecision::Allow));
        }
    }

    #[test]
    fn parses_permission_request_decision_object() {
        let out = parse_hook_stdout_json(
            r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow","updatedInput":{"command":"echo safe"}}}}"#,
        );
        assert_eq!(out.decision, Some(HookDecision::Allow));
        assert_eq!(
            out.updated_input,
            Some(serde_json::json!({"command": "echo safe"}))
        );

        let denied = parse_hook_stdout_json(
            r#"{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny","message":"forbidden"}}}"#,
        );
        assert_eq!(denied.decision, Some(HookDecision::Block));
        assert_eq!(denied.reason.as_deref(), Some("forbidden"));
    }
}
