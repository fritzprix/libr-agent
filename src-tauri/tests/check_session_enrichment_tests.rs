//! Windows-safe unit tests for checkSession response metadata enrichment (#1639).
//! (Not behind cfg(not(windows)) — no Tauri WebView link.)

use serde_json::json;
use tauri_mcp_agent_lib::execution_mode::ExecutionMode;
use tauri_mcp_agent_lib::mcp::builtin::agent::handlers::{
    append_check_session_context_to_message, apply_check_session_enrichment,
    build_terminal_check_session_result_from_messages,
    build_user_stopped_check_session_result_from_messages, check_session_enrichment_from_metadata,
    format_check_session_context_text,
};
use tauri_mcp_agent_lib::mcp::builtin::error_guidance::SuccessHint;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode;
use tauri_mcp_agent_lib::repositories::{SessionMetadata, SessionStatus};

const METADATA_FENCE_HEADER: &str =
    "[Metadata — identity/routing only; not the child session's answer]";

fn sample_meta() -> SessionMetadata {
    SessionMetadata {
        id: "child-1".to_string(),
        name: Some("  Draft release notes  ".to_string()),
        status: SessionStatus::Busy,
        model: "gpt-test".to_string(),
        provider: "openai".to_string(),
        assistant_id: Some("asst-1".to_string()),
        parent_session_id: Some("parent-1".to_string()),
        lineage_id: Some("lineage-1".to_string()),
        depth: Some(1),
        max_depth: None,
        max_fanout: None,
        org_id: Some("org-9".to_string()),
        org_name: Some("Team".to_string()),
        org_root_session_id: Some("root-1".to_string()),
        created_at: 1_700_000_000,
        updated_at: 1_700_000_100,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        execution_mode: ExecutionMode::Normal,
        workspace_override: Some("/shared/workspace".to_string()),
        workspace_isolation: WorkspaceIsolationMode::Host,
        docker_config: None,
        docker_container_name: None,
        docker_host_workspace_path: Some("/docker/host/path".to_string()),
    }
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
fn check_session_enrichment_omits_name_and_mirrors_fenced_context_into_text() {
    let meta = sample_meta();
    let enrichment = check_session_enrichment_from_metadata(&meta, Some("Researcher".to_string()));

    assert_eq!(enrichment.assistant_id.as_deref(), Some("asst-1"));
    assert_eq!(enrichment.assistant_name.as_deref(), Some("Researcher"));
    assert_eq!(
        enrichment.workspace_path.as_deref(),
        Some("/shared/workspace")
    );

    let context = format_check_session_context_text(&enrichment).expect("context text");
    assert!(context.starts_with("---\n"));
    assert!(context.contains(METADATA_FENCE_HEADER));
    assert!(context.contains("assistant: Researcher (asst-1)"));
    assert!(context.contains("workspace: /shared/workspace"));
    assert!(context.contains("orgId: org-9"));
    assert!(!context.contains("createdAt"));
    assert!(!context.contains("Draft release notes"));

    let result = build_terminal_check_session_result_from_messages(
        "child-1",
        "idle",
        1,
        &[json!({
            "role": "assistant",
            "content": [{"type": "text", "text": "done"}]
        })],
        Some(&enrichment),
    );

    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content expected");
    assert!(structured.get("name").is_none());
    assert!(structured.get("task").is_none());
    assert_eq!(
        structured
            .get("assistantName")
            .and_then(|value| value.as_str()),
        Some("Researcher")
    );
    assert_eq!(
        structured
            .get("workspacePath")
            .and_then(|value| value.as_str()),
        Some("/shared/workspace")
    );
    assert_eq!(
        structured.get("createdAt").and_then(|value| value.as_i64()),
        Some(1_700_000_000)
    );

    let text = extract_text(&result);
    assert!(text.contains("Session child-1 is terminal (idle)."));
    assert!(text.contains("Result:\ndone"));
    let status_idx = text
        .find("Session child-1 is terminal (idle).")
        .expect("status line expected");
    let meta_idx = text
        .find(METADATA_FENCE_HEADER)
        .expect("metadata fence expected");
    let result_idx = text.find("Result:").expect("Result section expected");
    assert!(status_idx < meta_idx);
    assert!(meta_idx < result_idx);
    assert!(text.contains("assistant: Researcher (asst-1)"));
    assert!(text.contains("workspace: /shared/workspace"));
    assert!(!text.contains("createdAt"));
    assert!(!text.contains("Draft release notes"));

    let docker_only = SessionMetadata {
        workspace_override: None,
        ..meta
    };
    let docker_enrichment = check_session_enrichment_from_metadata(&docker_only, None);
    let mut map = serde_json::Map::new();
    apply_check_session_enrichment(&mut map, &docker_enrichment);
    assert_eq!(
        map.get("workspacePath").and_then(|value| value.as_str()),
        Some("/docker/host/path")
    );
    assert!(map.get("assistantName").is_none());
    assert!(map.get("name").is_none());
}

#[test]
fn check_session_metadata_fields_collapse_newlines() {
    let mut meta = sample_meta();
    meta.assistant_id = Some("asst\n-1".to_string());
    meta.org_id = Some("org\r\n9".to_string());
    meta.workspace_override = Some("/shared/\nworkspace".to_string());

    let enrichment =
        check_session_enrichment_from_metadata(&meta, Some("Researcher\nName".to_string()));

    assert_eq!(enrichment.assistant_id.as_deref(), Some("asst-1"));
    assert_eq!(enrichment.assistant_name.as_deref(), Some("ResearcherName"));
    assert_eq!(
        enrichment.workspace_path.as_deref(),
        Some("/shared/workspace")
    );
    assert_eq!(enrichment.org_id.as_deref(), Some("org9"));

    let context = format_check_session_context_text(&enrichment).expect("context text");
    assert!(!context.contains('\r'));
    assert_eq!(context.matches('\n').count(), 4); // --- + header + 3 field lines
    assert!(context.contains("assistant: ResearcherName (asst-1)"));
    assert!(context.contains("workspace: /shared/workspace"));
    assert!(context.contains("orgId: org9"));
}

#[test]
fn check_session_metadata_block_is_before_follow_ups_from_success_hint() {
    let enrichment =
        check_session_enrichment_from_metadata(&sample_meta(), Some("Researcher".to_string()));
    let message = append_check_session_context_to_message(
        "Session child-1 is currently busy (Turns elapsed: 2).",
        &enrichment,
    );
    let result = SuccessHint::new(message, vec!["wait".to_string()]).to_mcp_result();
    let text = extract_text(&result);
    let meta_idx = text
        .find(METADATA_FENCE_HEADER)
        .expect("metadata fence expected");
    let follow_up_idx = text
        .find("💡 Suggested Follow-ups:")
        .expect("follow-ups expected");
    assert!(meta_idx < follow_up_idx);
}

#[test]
fn check_session_metadata_stays_before_result_that_quotes_follow_ups_marker() {
    let enrichment =
        check_session_enrichment_from_metadata(&sample_meta(), Some("Researcher".to_string()));
    // Child answers can quote the Follow-ups marker (e.g. code reviews). Metadata is
    // inserted before Result, so quotes in the body cannot steal placement.
    let body = "\
Session child-1 is terminal (idle).\n\n\
Result:\n\
Review notes that mention inserting before the\n\n\
💡 Suggested Follow-ups:\" marker in inject_check_session_context_text.\n\n\
More analysis after the quoted marker.";
    let message = append_check_session_context_to_message(body, &enrichment);
    let result = SuccessHint::new(message, vec![]).to_mcp_result();
    let text = extract_text(&result);

    let meta_idx = text
        .find(METADATA_FENCE_HEADER)
        .expect("metadata fence expected");
    let result_idx = text.find("Result:").expect("Result section");
    let quoted_idx = text
        .find("quoted marker")
        .expect("quoted prose after fake marker");

    assert!(meta_idx < result_idx);
    assert!(result_idx < quoted_idx);
    assert!(text.contains("orgId: org-9"));
}

#[test]
fn user_stopped_check_session_result_does_not_auto_recover() {
    let result = build_user_stopped_check_session_result_from_messages(
        "session-user-stop-123",
        3,
        &[json!({
            "role": "assistant",
            "content": [
                {
                    "type": "text",
                    "text": "Still working on the delegated task."
                }
            ]
        })],
        None,
    );

    let text = extract_text(&result);
    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content expected");

    assert!(text.contains("was stopped by the user"));
    assert!(text.contains("Do not automatically resume"));
    assert!(!text.contains("Wake the paused child session"));
    assert_eq!(
        structured
            .get("responseStatus")
            .and_then(|value| value.as_str()),
        Some("cancelled")
    );
    assert_eq!(
        structured
            .get("terminatedByUser")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    assert_eq!(
        structured
            .get("recoverable")
            .and_then(|value| value.as_bool()),
        Some(false)
    );
    assert!(structured.get("nextActions").is_none());
}
