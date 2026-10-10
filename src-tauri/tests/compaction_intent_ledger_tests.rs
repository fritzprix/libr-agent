//! Windows-safe coverage for post-compaction intent ledger (#2042).
//! Consolidated `integration_tests` is Linux/macOS-only (WebView link).

use tauri_mcp_agent_lib::agent::llm::completion::{
    collect_recent_external_user_requests, ensure_intent_sections_in_summary,
    format_recent_user_requests_section, RECENT_USER_REQUEST_LIMIT,
};
use tauri_mcp_agent_lib::models::chat::{Message, MessageSource};

fn external_user(id: &str, text: &str) -> Message {
    let mut message =
        Message::new_user_message("session".to_string(), text.to_string(), None, None);
    message.id = id.to_string();
    message.source = Some(MessageSource::Ui);
    message
}

#[test]
fn collect_recent_external_user_requests_keeps_last_n() {
    let mut messages = vec![
        external_user("u1", "First ask"),
        Message::new_user_message(
            "session".to_string(),
            "assistant reply".to_string(),
            None,
            None,
        ),
        external_user("u2", "Second ask"),
        external_user("u3", "Third ask"),
        external_user("u4", "Fourth ask"),
    ];
    messages[1].role = "assistant".to_string();

    let collected = collect_recent_external_user_requests(&messages, 3);
    assert_eq!(collected.len(), 3);
    assert_eq!(collected[0].message_id, "u2");
    assert_eq!(collected[2].message_id, "u4");
    assert!(format_recent_user_requests_section(&collected).contains("[`u3`] Third ask"));
    assert_eq!(RECENT_USER_REQUEST_LIMIT, 5);
}

#[test]
fn ensure_intent_sections_injects_working_intent_and_recent_requests() {
    let messages = vec![
        external_user("u1", "Ship the release notes"),
        external_user("u2", "Also fix the changelog link"),
    ];
    let summary = "### Active Request\n- None\n\n### Next Actions\n- Stop";

    let ensured = ensure_intent_sections_in_summary(summary, &messages);

    assert!(ensured.contains("### Working Intent"));
    assert!(ensured.contains("Also fix the changelog link"));
    assert!(ensured.contains("### Recent User Requests"));
    assert!(ensured.contains("[`u1`] Ship the release notes"));
    assert!(ensured.contains("[`u2`] Also fix the changelog link"));
}

#[test]
fn ensure_intent_sections_inserts_before_context_recovery_suffix() {
    let messages = vec![external_user("u1", "Recover me")];
    let summary = "### Active Request\n- Keep going\n\n---\n### Context recovery\n\
Before declaring done / calling `ui__reportResult(success)`, read \
`.libragent/pre_compaction_epoch_1.md` — empty Active Request alone is not a Done signal.";

    let ensured = ensure_intent_sections_in_summary(summary, &messages);
    let recent_idx = ensured
        .find("### Recent User Requests")
        .expect("recent ledger");
    let recovery_idx = ensured.find("### Context recovery").expect("recovery");
    assert!(recent_idx < recovery_idx);
    assert!(ensured.contains("[`u1`] Recover me"));
    assert!(ensured.contains("empty Active Request alone is not a Done signal"));
    assert!(ensured.contains("---\n### Context recovery"));
}

#[test]
fn ensure_intent_sections_handles_colon_heading_and_crlf_divider() {
    let messages = vec![external_user("u1", "Recover me")];
    let summary = "### Working Intent:\r\n- Keep user goal\r\n\r\n### Recent User Requests\r\n- stale\r\n\r\n---\r\n### Context recovery\r\nepoch";

    let ensured = ensure_intent_sections_in_summary(summary, &messages);
    assert_eq!(ensured.matches("### Working Intent").count(), 1);
    assert!(ensured.contains("Keep user goal"));
    assert!(!ensured.contains("- stale"));
    assert!(ensured.contains("[`u1`] Recover me"));
    assert!(ensured.contains("---\n### Context recovery"));
}
