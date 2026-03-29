use std::collections::HashMap;

use tauri_mcp_agent_lib::agent::state::PendingApprovalData;
use tauri_mcp_agent_lib::agent::tool_approvals::{
    find_pending_approval_tool_call_id, parse_channel_permission_behavior,
};

#[test]
fn parse_channel_permission_behavior_accepts_allow_and_deny() {
    assert_eq!(parse_channel_permission_behavior("allow"), Ok(true));
    assert_eq!(parse_channel_permission_behavior("deny"), Ok(false));
}

#[test]
fn parse_channel_permission_behavior_rejects_invalid_values() {
    let error =
        parse_channel_permission_behavior("maybe").expect_err("invalid behavior should fail");
    assert_eq!(
        error,
        "Invalid channel permission behavior: maybe (expected 'allow' or 'deny')"
    );
}

#[test]
fn find_pending_approval_tool_call_id_matches_request_id() {
    let (tx1, _rx1) = tokio::sync::oneshot::channel();
    let (tx2, _rx2) = tokio::sync::oneshot::channel();

    let approvals = HashMap::from([
        (
            "tool-call-1".to_string(),
            PendingApprovalData {
                sender: tx1,
                tool_name: "workspace__writeFile".to_string(),
                arguments: "{}".to_string(),
                request_id: Some("abcde".to_string()),
                description: Some("first".to_string()),
                input_preview: Some("{}".to_string()),
            },
        ),
        (
            "tool-call-2".to_string(),
            PendingApprovalData {
                sender: tx2,
                tool_name: "workspace__writeFile".to_string(),
                arguments: "{}".to_string(),
                request_id: Some("fghij".to_string()),
                description: Some("second".to_string()),
                input_preview: Some("{}".to_string()),
            },
        ),
    ]);

    assert_eq!(
        find_pending_approval_tool_call_id(&approvals, "fghij"),
        Some("tool-call-2".to_string())
    );
    assert_eq!(
        find_pending_approval_tool_call_id(&approvals, "missing"),
        None
    );
}

#[test]
fn find_pending_approval_tool_call_id_ignores_entries_without_request_ids() {
    let (tx, _rx) = tokio::sync::oneshot::channel();
    let approvals = HashMap::from([(
        "tool-call-1".to_string(),
        PendingApprovalData {
            sender: tx,
            tool_name: "workspace__writeFile".to_string(),
            arguments: "{}".to_string(),
            request_id: None,
            description: None,
            input_preview: None,
        },
    )]);

    assert_eq!(
        find_pending_approval_tool_call_id(&approvals, "abcde"),
        None
    );
}
