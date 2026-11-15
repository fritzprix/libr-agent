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
    pub continue_loop: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawHookStdout {
    #[serde(default, rename = "continue")]
    continue_loop: Option<bool>,
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

    if event == HookEvent::SessionStart {
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
        .and_then(|h| h.permission_decision.as_deref())
        .or(raw.decision.as_deref())
        .and_then(parse_decision);

    let reason = raw
        .hook_specific_output
        .as_ref()
        .and_then(|h| h.permission_decision_reason.clone())
        .or(raw.reason);

    let continue_loop = raw.continue_loop;

    HookOutput {
        additional_context,
        decision,
        reason,
        continue_loop,
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
}
