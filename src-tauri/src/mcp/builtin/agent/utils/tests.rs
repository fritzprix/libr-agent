use super::*;
use serde_json::{json, Value};

fn assistant_json(id: &str, text: &str) -> Value {
    json!({
        "id": id,
        "role": "assistant",
        "content": [{"type": "text", "text": text}]
    })
}

#[test]
fn build_agent_session_tool_data_emits_display_and_storage_ids() {
    let data = build_agent_session_tool_data(
        "checkSession",
        "session-1735123456789012345",
        "ok",
        "idle",
        "success",
        1,
        vec![],
    );

    assert_eq!(
        data.get("sessionId").and_then(|v| v.as_str()),
        Some("6789012345")
    );
    assert_eq!(
        data.get("storageSessionId").and_then(|v| v.as_str()),
        Some("session-1735123456789012345")
    );
    assert_eq!(
        data.get("resourceId").and_then(|v| v.as_str()),
        Some("6789012345")
    );
}

#[test]
fn select_preferred_prefers_cache_when_db_lags_behind_terminal_assistant() {
    let db_messages = vec![assistant_json("asst-1", "older answer")];
    let cached_messages = vec![
        assistant_json("asst-2", "final answer"),
        assistant_json("asst-1", "older answer"),
    ];

    let selected = select_preferred_session_messages(db_messages, Some(cached_messages));

    assert_eq!(latest_session_output(&selected), "final answer");
}

#[test]
fn select_preferred_keeps_db_when_cache_is_stale() {
    let db_messages = vec![assistant_json("asst-2", "authoritative answer")];
    let cached_messages = vec![assistant_json("asst-1", "stale cache")];

    let selected = select_preferred_session_messages(db_messages, Some(cached_messages));

    assert_eq!(latest_session_output(&selected), "authoritative answer");
}

#[test]
fn latest_assistant_message_text_scans_past_empty_tool_call_turns() {
    // Message #2 (latest): tool call turn with no text content
    let latest_empty_asst = json!({
        "id": "asst-2",
        "role": "assistant",
        "content": [],
        "tool_calls": [{"id": "call-1", "function": {"name": "agent__checkSession"}}]
    });
    // Message #1 (earlier): contains the real final answer text
    let earlier_asst = assistant_json(
        "asst-1",
        "Consensus delegation review completed successfully.",
    );

    let messages = vec![latest_empty_asst, earlier_asst];

    let (msg_id, output) = latest_assistant_message_text(&messages, None).unwrap();
    assert_eq!(msg_id, "asst-1");
    assert_eq!(
        output,
        "Consensus delegation review completed successfully."
    );
    assert_eq!(
        latest_session_output(&messages),
        "Consensus delegation review completed successfully."
    );
}

#[test]
fn latest_session_output_prefers_report_result_over_earlier_assistant_text() {
    // Mirrors production: child called ui__reportResult, then parent checkSession
    // used to return older assistant narration instead of the report body.
    let report_tool = json!({
        "id": "tool-report",
        "role": "tool",
        "content": [
            {
                "type": "text",
                "text": "Final result reported (status=success).\nTitle: Result\nResult:\n## Verdict: approve-with-caveats\n## Confidence: high\n\nSTOP: Do not call any more tools. The task outcome is already delivered. End your turn now with at most a one-sentence confirmation."
            },
            {
                "type": "resource",
                "resource": {
                    "uri": "ui://result/abc-123",
                    "mimeType": "text/html",
                    "text": "<html></html>"
                },
                "serviceInfo": {
                    "serverName": "ui",
                    "toolName": "reportResult",
                    "backendType": "BuiltInRust"
                }
            }
        ]
    });
    let empty_asst = json!({
        "id": "asst-report-call",
        "role": "assistant",
        "content": [],
        "tool_calls": [{"id": "call-report", "function": {"name": "ui__reportResult"}}]
    });
    let earlier_asst = assistant_json(
        "asst-progress",
        "The persist_terminal test passes in isolation — continuing the review.",
    );

    let messages = vec![report_tool, empty_asst, earlier_asst];

    assert_eq!(
        latest_session_output(&messages),
        "## Verdict: approve-with-caveats\n## Confidence: high"
    );
}

#[test]
fn latest_session_output_falls_back_to_report_result_text_without_markers() {
    // When the summary format drifts (no Result:/STOP markers), still prefer
    // the reportResult tool text over earlier assistant narration.
    let report_tool = json!({
        "id": "tool-report-plain",
        "role": "tool",
        "content": [
            {
                "type": "text",
                "text": "Deliverable without wrapper markers: approve-with-caveats"
            },
            {
                "type": "resource",
                "resource": {
                    "uri": "ui://result/plain-1",
                    "mimeType": "text/html",
                    "text": "<html></html>"
                },
                "serviceInfo": {
                    "serverName": "ui",
                    "toolName": "reportResult",
                    "backendType": "BuiltInRust"
                }
            }
        ]
    });
    let earlier_asst = assistant_json("asst-old", "Earlier assistant chatter should lose.");

    let messages = vec![report_tool, earlier_asst];

    assert_eq!(
        latest_session_output(&messages),
        "Deliverable without wrapper markers: approve-with-caveats"
    );
}

fn report_result_tool(id: &str, body: &str) -> Value {
    json!({
        "id": id,
        "role": "tool",
        "content": [
            {
                "type": "text",
                "text": format!(
                    "Final result reported (status=success).\nTitle: Result\nResult:\n{body}\n\nSTOP: Do not call any more tools."
                )
            },
            {
                "type": "resource",
                "resource": {
                    "uri": format!("ui://result/{id}"),
                    "mimeType": "text/html",
                    "text": "<html></html>"
                },
                "serviceInfo": {
                    "serverName": "ui",
                    "toolName": "reportResult",
                    "backendType": "BuiltInRust"
                }
            }
        ]
    })
}

#[test]
fn latest_session_output_follow_up_assistant_overrides_stale_report_result() {
    // Parent messageToSession after reportResult: newer assistant text must win.
    // Newest-first: follow-up answer → user → confirmation → reportResult → call.
    let messages = vec![
        assistant_json(
            "asst-follow-up",
            "Here is the complete report:\n\n# Deep Research Report",
        ),
        json!({
            "id": "user-follow-up",
            "role": "user",
            "content": "Please output the full contents of the report."
        }),
        assistant_json("asst-confirm", "Done. The task outcome is delivered."),
        report_result_tool(
            "tool-report-stale",
            "Brief summary of outputs/rlcd_deep_dive.md",
        ),
        json!({
            "id": "asst-report-call",
            "role": "assistant",
            "content": [],
            "tool_calls": [{"id": "call-report", "function": {"name": "ui__reportResult"}}]
        }),
        assistant_json("asst-progress", "Still drafting the report…"),
    ];

    assert_eq!(
        latest_session_output(&messages),
        "Here is the complete report:\n\n# Deep Research Report"
    );
}

#[test]
fn latest_session_output_confirmation_without_user_does_not_override_report_result() {
    // One-sentence confirmation after reportResult (no new user turn) must not
    // shadow the deliverable — that was the original prefer-reportResult fix.
    let messages = vec![
        assistant_json("asst-confirm", "Done. The task outcome is delivered."),
        report_result_tool(
            "tool-report",
            "## Verdict: approve-with-caveats\n## Confidence: high",
        ),
        json!({
            "id": "asst-report-call",
            "role": "assistant",
            "content": [],
            "tool_calls": [{"id": "call-report", "function": {"name": "ui__reportResult"}}]
        }),
    ];

    assert_eq!(
        latest_session_output(&messages),
        "## Verdict: approve-with-caveats\n## Confidence: high"
    );
}

#[test]
fn latest_session_output_follow_up_without_assistant_text_keeps_report_result() {
    // Follow-up requested but child is still tool-calling — keep report until text arrives.
    let messages = vec![
        json!({
            "id": "asst-reading",
            "role": "assistant",
            "content": [],
            "tool_calls": [{"id": "call-read", "function": {"name": "workspace__readFile"}}]
        }),
        json!({
            "id": "user-follow-up",
            "role": "user",
            "content": "Please output the full contents."
        }),
        report_result_tool("tool-report", "Brief summary only"),
    ];

    assert_eq!(latest_session_output(&messages), "Brief summary only");
}

#[test]
fn latest_session_output_follow_up_with_confirmation_still_keeps_report_result_until_answered() {
    // Realistic sequence: reportResult → short confirmation → parent follow-up
    // → child still tool-calling. Must not return the pre-follow-up confirmation.
    let messages = vec![
        json!({
            "id": "asst-reading",
            "role": "assistant",
            "content": [],
            "tool_calls": [{"id": "call-read", "function": {"name": "workspace__readFile"}}]
        }),
        json!({
            "id": "user-follow-up",
            "role": "user",
            "content": "Please output the full contents."
        }),
        assistant_json("asst-confirm", "Done. Result delivered."),
        report_result_tool("tool-report", "Brief summary only"),
    ];

    assert_eq!(latest_session_output(&messages), "Brief summary only");
}

#[test]
fn latest_session_output_newer_report_result_wins_over_prior_follow_up() {
    // A second reportResult after a follow-up must become the new authoritative output.
    let messages = vec![
        report_result_tool("tool-report-2", "Updated deliverable after revision"),
        json!({
            "id": "asst-report-call-2",
            "role": "assistant",
            "content": [],
            "tool_calls": [{"id": "call-report-2", "function": {"name": "ui__reportResult"}}]
        }),
        assistant_json("asst-follow-up", "Here is the complete report (now stale)"),
        json!({
            "id": "user-follow-up",
            "role": "user",
            "content": "Please revise and report again."
        }),
        report_result_tool("tool-report-1", "Original brief summary"),
    ];

    assert_eq!(
        latest_session_output(&messages),
        "Updated deliverable after revision"
    );
}
