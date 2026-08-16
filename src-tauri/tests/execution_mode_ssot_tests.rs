//! Windows-safe coverage for execution-mode SSOT helpers.
//!
//! Full `AgentSessionManager::set_execution_mode` needs a repository + Tauri app;
//! this binary covers the in-memory SSOT surface and the CAS restore used when a
//! later DB write fails after a concurrent writer has already moved mode again.

use std::sync::Arc;
use tauri_mcp_agent_lib::agent::context::registry::ContextRegistry;
use tauri_mcp_agent_lib::agent::session_manager::execution_mode::revert_mode_if_unchanged;
use tauri_mcp_agent_lib::agent::state::AgentSession;
use tauri_mcp_agent_lib::execution_mode::ExecutionMode;
use tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode;
use tauri_mcp_agent_lib::repositories::{SessionMetadata, SessionStatus};

fn sample_metadata(mode: ExecutionMode) -> SessionMetadata {
    let now = 1_i64;
    SessionMetadata {
        id: "session-ssot-1".to_string(),
        name: Some("ssot".to_string()),
        status: SessionStatus::Idle,
        model: "gpt-test".to_string(),
        provider: "openai".to_string(),
        assistant_id: None,
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
        execution_mode: mode,
        workspace_override: None,
        workspace_isolation: WorkspaceIsolationMode::Host,
        docker_config: None,
        docker_container_name: None,
        docker_host_workspace_path: None,
    }
}

#[test]
fn agent_session_execution_mode_is_metadata_ssot() {
    for mode in [
        ExecutionMode::Normal,
        ExecutionMode::Yolo,
        ExecutionMode::Unsafe,
    ] {
        let session = AgentSession::new(
            sample_metadata(mode),
            Arc::new(ContextRegistry::new()),
            None,
        );
        assert_eq!(session.execution_mode(), mode);
        assert_eq!(session.metadata.execution_mode, mode);
    }
}

#[test]
fn mutating_metadata_execution_mode_is_immediately_visible() {
    let mut session = AgentSession::new(
        sample_metadata(ExecutionMode::Normal),
        Arc::new(ContextRegistry::new()),
        None,
    );
    session.metadata.execution_mode = ExecutionMode::Unsafe;
    assert_eq!(session.execution_mode(), ExecutionMode::Unsafe);
}

#[test]
fn revert_restores_previous_when_still_expected() {
    assert_eq!(
        revert_mode_if_unchanged(
            ExecutionMode::Yolo,
            ExecutionMode::Yolo,
            ExecutionMode::Normal
        ),
        ExecutionMode::Normal
    );
}

#[test]
fn revert_skips_when_concurrent_writer_already_moved_mode() {
    assert_eq!(
        revert_mode_if_unchanged(
            ExecutionMode::Unsafe,
            ExecutionMode::Yolo,
            ExecutionMode::Normal
        ),
        ExecutionMode::Unsafe
    );
}
