use tauri_mcp_agent_lib::agent::state::PendingApprovalKind;
use tauri_mcp_agent_lib::agent::tool_approvals::{
    approval_request_for_runtime, pending_approval_is_auto_approvable_in_yolo, ToolApprovalRequest,
    ToolExecutionPolicyDecision,
};

#[test]
fn yolo_skips_normal_approval_but_keeps_hard_approval() {
    let normal = ToolExecutionPolicyDecision::RequireApproval(ToolApprovalRequest {
        description: "normal".to_string(),
        input_preview: "args".to_string(),
    });
    let hard = ToolExecutionPolicyDecision::RequireHardApproval(ToolApprovalRequest {
        description: "hard".to_string(),
        input_preview: "args".to_string(),
    });

    assert!(approval_request_for_runtime(&normal, false, false).is_some());
    assert!(approval_request_for_runtime(&normal, true, false).is_none());
    assert!(approval_request_for_runtime(&hard, false, false).is_some());
    assert!(approval_request_for_runtime(&hard, true, false).is_some());
}

#[test]
fn unsafe_mode_bypasses_normal_and_hard_approvals() {
    let normal = ToolExecutionPolicyDecision::RequireApproval(ToolApprovalRequest {
        description: "normal".to_string(),
        input_preview: "args".to_string(),
    });
    let hard = ToolExecutionPolicyDecision::RequireHardApproval(ToolApprovalRequest {
        description: "hard".to_string(),
        input_preview: "args".to_string(),
    });

    assert!(approval_request_for_runtime(&normal, false, true).is_none());
    assert!(approval_request_for_runtime(&hard, false, true).is_none());
}

#[test]
fn yolo_auto_approval_never_consumes_hard_pending_requests() {
    assert!(pending_approval_is_auto_approvable_in_yolo(
        PendingApprovalKind::Standard
    ));
    assert!(!pending_approval_is_auto_approvable_in_yolo(
        PendingApprovalKind::Hard
    ));
}
