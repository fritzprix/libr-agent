//! Windows-safe coverage for checkSession wait-timeout → terminal promotion.
//! (integration/wait_session_complete_status_tests.rs is Linux/macOS-only.)

use serde_json::json;
use std::time::Duration;
use tauri_mcp_agent_lib::agent::session_bus::SessionBus;
use tauri_mcp_agent_lib::execution_mode::ExecutionMode;
use tauri_mcp_agent_lib::mcp::builtin::agent::handlers::build_terminal_check_session_result_from_messages;
use tauri_mcp_agent_lib::mcp::builtin::agent::utils::{
    extract_session_status, latest_session_output, promote_settled_session_after_wait_timeout,
    session_metadata_to_value, should_promote_wait_timeout_to_terminal,
};
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode;
use tauri_mcp_agent_lib::repositories::{SessionMetadata, SessionStatus};
use tokio::time::timeout;

fn idle_session_meta(id: &str) -> SessionMetadata {
    SessionMetadata {
        id: id.to_string(),
        name: Some("child".to_string()),
        status: SessionStatus::Idle,
        model: "gpt-test".to_string(),
        provider: "openai".to_string(),
        assistant_id: Some("asst-1".to_string()),
        parent_session_id: Some("parent-1".to_string()),
        lineage_id: Some("lineage-1".to_string()),
        depth: Some(1),
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        created_at: 1,
        updated_at: 2,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        execution_mode: ExecutionMode::Normal,
        workspace_override: None,
        workspace_isolation: WorkspaceIsolationMode::Host,
        docker_config: None,
        docker_container_name: None,
        docker_host_workspace_path: None,
    }
}

fn report_result_messages(body: &str) -> Vec<serde_json::Value> {
    vec![
        json!({
            "id": "tool-report",
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
                        "uri": "ui://result/promo-1",
                        "mimeType": "text/plain",
                        "text": ""
                    },
                    "serviceInfo": {
                        "serverName": "ui",
                        "toolName": "reportResult",
                        "backendType": "BuiltInRust"
                    }
                }
            ],
            "metadata": {
                "structuredContent": {
                    "type": "reportResult",
                    "status": "success",
                    "result": body
                }
            }
        }),
        json!({
            "id": "asst-call",
            "role": "assistant",
            "content": [],
            "tool_calls": [{"id": "call-1", "function": {"name": "ui__reportResult"}}]
        }),
    ]
}

fn extract_text(result: &tauri_mcp_agent_lib::mcp::types::MCPResult) -> String {
    result
        .content
        .as_ref()
        .expect("text content expected")
        .iter()
        .filter_map(|content| match content {
            MCPContent::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn wait_timeout_promotes_when_child_already_settled() {
    for status in ["idle", "Idle", "paused", "error", "terminated", "failed"] {
        assert!(
            should_promote_wait_timeout_to_terminal(status),
            "{status} must promote a wait timeout to terminal delivery"
        );
    }
    for status in ["busy", "unknown", "running"] {
        assert!(
            !should_promote_wait_timeout_to_terminal(status),
            "{status} must keep the timeout error path"
        );
    }
}

#[test]
fn promote_helper_accepts_serialized_idle_session_metadata() {
    let value = session_metadata_to_value(&idle_session_meta("child-idle-1")).expect("serialize");
    assert!(
        should_promote_wait_timeout_to_terminal(&extract_session_status(&value)),
        "serde SessionStatus must still promote (case-insensitive)"
    );

    let promoted = promote_settled_session_after_wait_timeout(Some(value)).expect("promote");
    assert_eq!(promoted.1, 1, "wake placeholder must stay 1");
    assert_eq!(
        extract_session_status(&promoted.0).to_ascii_lowercase(),
        "idle"
    );

    assert!(
        promote_settled_session_after_wait_timeout(None).is_none(),
        "missing fetch must not promote"
    );

    let busy = json!({ "id": "x", "status": "busy" });
    assert!(
        promote_settled_session_after_wait_timeout(Some(busy)).is_none(),
        "busy must not promote"
    );
}

#[test]
fn timeout_promotion_feeds_terminal_check_session_with_report_result_body() {
    // Simulates: wait Err(timeout) → promote idle session → build_terminal_…_from_messages
    let session = session_metadata_to_value(&idle_session_meta("child-promo")).expect("serialize");
    let (promoted_session, _) =
        promote_settled_session_after_wait_timeout(Some(session)).expect("promote idle");

    let status = extract_session_status(&promoted_session);
    let messages = report_result_messages("## Daily briefing deliverable");
    let result = build_terminal_check_session_result_from_messages(
        "child-promo",
        &status,
        3,
        &messages,
        None,
    );

    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content");
    assert_eq!(
        structured.get("responseStatus").and_then(|v| v.as_str()),
        Some("success")
    );
    assert_eq!(
        structured.get("result").and_then(|v| v.as_str()),
        Some("## Daily briefing deliverable")
    );
    assert!(extract_text(&result).contains("Daily briefing deliverable"));
    assert!(!extract_text(&result).contains("timed out"));
}

#[test]
fn latest_session_output_prefers_structured_report_result_over_spilled_text() {
    let report_tool = json!({
        "id": "tool-report-structured",
        "role": "tool",
        "content": [
            {
                "type": "text",
                "text": "Tool output spilled to `.libragent/tool-results/call_x.txt` (too large)."
            },
            {
                "type": "resource",
                "resource": {
                    "uri": "ui://result/structured-1",
                    "mimeType": "text/plain",
                    "text": ""
                },
                "serviceInfo": {
                    "serverName": "ui",
                    "toolName": "reportResult",
                    "backendType": "BuiltInRust"
                }
            }
        ],
        "metadata": {
            "structuredContent": {
                "type": "reportResult",
                "status": "success",
                "result": "## Structured deliverable body"
            }
        }
    });
    let earlier_asst = json!({
        "id": "asst-old",
        "role": "assistant",
        "content": [{"type": "text", "text": "Earlier assistant chatter should lose."}]
    });

    assert_eq!(
        latest_session_output(&[report_tool, earlier_asst]),
        "## Structured deliverable body"
    );
}

/// Documents that notify_waiters+notify_one may leave a residual permit after an
/// already-parked waiter is woken — the next wait loop iteration can wake once
/// spuriously, which is benign (status is re-checked).
#[tokio::test]
async fn session_bus_notify_may_spurious_wake_after_parked_waiter() {
    let bus = SessionBus::new();
    let notifier = bus.get_or_create("sess-double");

    let first = tokio::spawn({
        let notifier = notifier.clone();
        async move {
            notifier.notified().await;
        }
    });
    tokio::task::yield_now().await;
    bus.notify_status_change("sess-double");
    timeout(Duration::from_millis(100), first)
        .await
        .expect("join")
        .expect("first waiter");

    // Residual notify_one permit: next notified() completes without a new status change.
    let second = timeout(Duration::from_millis(50), notifier.notified()).await;
    assert!(
        second.is_ok(),
        "benign spurious wake from leftover notify_one permit is expected"
    );
}
