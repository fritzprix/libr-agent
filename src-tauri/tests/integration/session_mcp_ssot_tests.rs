//! Session MCP activation SSOT: session → assistant → mcpServerIds.
//!
//! Create-request payloads must not invent or override the assistant-bound list.

use crate::common;

use tauri_mcp_agent_lib::agent::{resolve_session_mcp_bindings, ExecutionMode};
use tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode;
use tauri_mcp_agent_lib::repositories::{SessionMetadata, SessionStatus};

fn session_with_assistant(session_id: &str, assistant_id: Option<&str>) -> SessionMetadata {
    let now = chrono::Utc::now().timestamp_millis();
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("MCP SSOT".to_string()),
        status: SessionStatus::Idle,
        model: "gpt-4".to_string(),
        provider: "openai".to_string(),
        assistant_id: assistant_id.map(str::to_string),
        parent_session_id: None,
        lineage_id: None,
        depth: None,
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        created_at: now,
        updated_at: now,
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

#[tokio::test]
async fn resolve_session_mcp_bindings_uses_assistant_mcp_server_ids() {
    let db = common::setup_test_db_with_migrations().await;
    let assistant_id = "asst-mcp-ssot-only-a";

    common::seed_test_assistant(
        &db,
        assistant_id,
        "SSOT Assistant",
        serde_json::json!({
            "systemPrompt": "You are a test assistant.",
            "mcpServerIds": ["server-a-only"],
            // Stale / client-side extras must never leak into resolution:
            // only mcpServerIds on the assistant row matter.
        }),
    )
    .await;

    let session = session_with_assistant("sess-mcp-ssot-1", Some(assistant_id));
    let (_tool_ids, mcp_ids) = resolve_session_mcp_bindings(&session)
        .await
        .expect("assistant-backed MCP bindings should resolve");

    assert_eq!(
        mcp_ids,
        vec!["server-a-only".to_string()],
        "external MCP activation must come only from assistant.mcpServerIds"
    );
}

#[tokio::test]
async fn resolve_session_mcp_bindings_ignores_absence_of_request_payload_ids() {
    let db = common::setup_test_db_with_migrations().await;
    let assistant_id = "asst-mcp-ssot-empty";

    common::seed_test_assistant(
        &db,
        assistant_id,
        "No External MCP",
        serde_json::json!({
            "systemPrompt": "You are a test assistant.",
            "mcpServerIds": []
        }),
    )
    .await;

    let session = session_with_assistant("sess-mcp-ssot-2", Some(assistant_id));
    let (_tool_ids, mcp_ids) = resolve_session_mcp_bindings(&session)
        .await
        .expect("assistant with empty mcpServerIds should resolve");

    assert!(
        mcp_ids.is_empty(),
        "empty assistant mcpServerIds must yield no external MCP servers \
         (create-request mcpServerIds must not be consulted)"
    );
}

#[tokio::test]
async fn resolve_session_mcp_bindings_disables_external_without_assistant() {
    // No assistant repo registration needed — path short-circuits on missing assistant_id.
    let _db = common::setup_test_db().await;

    let session = session_with_assistant("sess-mcp-ssot-3", None);
    let (_tool_ids, mcp_ids) = resolve_session_mcp_bindings(&session)
        .await
        .expect("no-assistant sessions should still resolve bindings");

    assert!(
        mcp_ids.is_empty(),
        "sessions without assistant_id must not activate external MCP servers"
    );
}

#[tokio::test]
async fn resolve_session_mcp_bindings_reads_live_assistant_row_not_stale_ids() {
    let db = common::setup_test_db_with_migrations().await;
    let assistant_id = "asst-mcp-ssot-live";

    common::seed_test_assistant(
        &db,
        assistant_id,
        "Live Assistant",
        serde_json::json!({
            "systemPrompt": "You are a test assistant.",
            "mcpServerIds": ["harbor-stale", "exa"]
        }),
    )
    .await;

    // Update the live assistant row — SSOT must pick this up.
    use tauri_mcp_agent_lib::repositories::{AssistantRepository, SqliteAssistantRepository};
    let repo = SqliteAssistantRepository::new(db.clone());
    repo.update_assistant(
        assistant_id,
        None,
        Some(
            serde_json::json!({
                "systemPrompt": "You are a test assistant.",
                "mcpServerIds": ["exa"]
            })
            .to_string(),
        ),
    )
    .await
    .expect("assistant update should succeed");

    let session = session_with_assistant("sess-mcp-ssot-4", Some(assistant_id));
    let (_tool_ids, mcp_ids) = resolve_session_mcp_bindings(&session)
        .await
        .expect("updated assistant bindings should resolve");

    assert_eq!(
        mcp_ids,
        vec!["exa".to_string()],
        "resolution must use the live assistants table row, not a stale create-time snapshot"
    );
    assert!(
        !mcp_ids.iter().any(|id| id == "harbor-stale"),
        "removed MCP server IDs must not remain active for new/resumed sessions"
    );
}
