use crate::common;

use tauri_mcp_agent_lib::agent::ExecutionMode;
use tauri_mcp_agent_lib::mcp::builtin::agent::handlers::{
    create_org_scaffold_preflight, existing_explicit_org_identity, inspect_teamwork_scaffold,
    org_child_inherited_docker_workspace, teamwork_scaffold_optional_host_script_guidance,
    teamwork_scaffold_writefile_guidance,
};
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::workspace_isolation::{
    DockerWorkspaceConfig, WorkspaceIsolationMode,
};
use tauri_mcp_agent_lib::repositories::{
    SessionMetadata, SessionRepository, SessionStatus, SqliteSessionRepository,
};
use tauri_mcp_agent_lib::session::{prepare_teamwork_artifact_dir_for_session, SessionManager};

fn make_session(session_id: &str) -> SessionMetadata {
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("Org Root".to_string()),
        status: SessionStatus::Idle,
        model: "gpt-5.4".to_string(),
        provider: "openai".to_string(),
        assistant_id: None,
        parent_session_id: None,
        lineage_id: Some(session_id.to_string()),
        depth: Some(0),
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        created_at: 1,
        updated_at: 1,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        execution_mode: ExecutionMode::Normal,
        workspace_override: None,
        workspace_isolation:
            tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode::Host,
        docker_config: None,
        docker_container_name: None,
        docker_host_workspace_path: None,
        status_reason: None,
    }
}

#[test]
fn existing_org_identity_returns_current_org_metadata() {
    let mut session = make_session("existing-org-root");
    session.org_id = Some("org-alpha".to_string());
    session.org_name = Some("Alpha Org".to_string());
    session.org_root_session_id = Some("existing-org-root".to_string());

    assert_eq!(
        existing_explicit_org_identity(&session),
        Some((
            "org-alpha".to_string(),
            "Alpha Org".to_string(),
            "existing-org-root".to_string(),
        ))
    );
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

#[tokio::test]
async fn preparing_teamwork_artifacts_does_not_change_org_root_effective_workspace() {
    common::register_sqlite_vec();
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSessionRepository::new(db);
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let session_manager =
        SessionManager::new_with_base_dir(temp_dir.path().join("session-root")).unwrap();
    let session_id = "org-root-session";

    repo.upsert_session(&make_session(session_id))
        .await
        .expect("session should persist");

    let original_workspace = session_manager.get_session_workspace_dir_by_id(session_id);
    let artifact_dir = prepare_teamwork_artifact_dir_for_session(&session_manager, session_id)
        .await
        .expect("artifact dir should provision");
    let effective_workspace_after = session_manager.get_session_workspace_dir_by_id(session_id);

    assert_eq!(
        original_workspace, effective_workspace_after,
        "org root should keep the same effective workspace after preparing teamwork artifacts"
    );
    assert!(
        artifact_dir.ends_with(std::path::Path::new("teamwork-artifacts").join(session_id)),
        "artifact dir should be app-local and separate from the effective workspace: {}",
        artifact_dir.display()
    );

    let persisted = repo
        .get_session(session_id)
        .await
        .expect("session lookup should succeed")
        .expect("session should exist");
    assert_eq!(
        persisted.workspace_override, None,
        "preparing teamwork artifacts must not persist a workspace override"
    );
}

#[test]
fn org_child_inherits_docker_attach_from_org_root() {
    let mut root = make_session("harbor-org-root");
    root.workspace_isolation = WorkspaceIsolationMode::Docker;
    root.docker_config = Some(DockerWorkspaceConfig {
        image: None,
        attach_container: Some("harbor-trial-abc".to_string()),
        workdir: Some("/app".to_string()),
        manage_lifecycle: Some(false),
        env: Default::default(),
        port_bindings: vec![],
    });

    let (mode, config) =
        org_child_inherited_docker_workspace(&root).expect("attach root should inherit");
    assert_eq!(mode, WorkspaceIsolationMode::Docker);
    assert_eq!(config.attach_container_name(), Some("harbor-trial-abc"));
    assert_eq!(config.workdir(), "/app");
    assert!(!config.manage_lifecycle());
}

#[test]
fn org_child_does_not_inherit_managed_docker_image() {
    let mut root = make_session("managed-docker-root");
    root.workspace_isolation = WorkspaceIsolationMode::Docker;
    root.docker_config = Some(DockerWorkspaceConfig {
        image: Some("libragent/workspace:latest".to_string()),
        attach_container: None,
        workdir: Some("/workspace".to_string()),
        manage_lifecycle: Some(true),
        env: Default::default(),
        port_bindings: vec![],
    });

    assert!(
        org_child_inherited_docker_workspace(&root).is_none(),
        "managed image docker must stay per-session (no second container sharing)"
    );
}

#[test]
fn org_child_does_not_inherit_host_isolation() {
    let root = make_session("host-org-root");
    assert!(org_child_inherited_docker_workspace(&root).is_none());
}

#[test]
fn org_child_attach_inherit_forces_manage_lifecycle_false() {
    let mut root = make_session("attach-lifecycle-true");
    root.workspace_isolation = WorkspaceIsolationMode::Docker;
    root.docker_config = Some(DockerWorkspaceConfig {
        image: None,
        attach_container: Some("external-box".to_string()),
        workdir: Some("/app".to_string()),
        // Misconfigured attach: lifecycle true would risk stopping Harbor's container.
        manage_lifecycle: Some(true),
        env: Default::default(),
        port_bindings: vec![],
    });

    let (_, config) =
        org_child_inherited_docker_workspace(&root).expect("attach root should inherit");
    assert!(
        !config.manage_lifecycle(),
        "inherited attach must never manage external container lifecycle"
    );
}

#[test]
fn create_org_preflight_rejects_missing_scaffold_with_precise_paths() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let scaffold = inspect_teamwork_scaffold(temp_dir.path());

    let result = create_org_scaffold_preflight(&scaffold)
        .expect_err("missing scaffold must block createOrg");
    let text = extract_text(&result);

    assert_eq!(result.is_error, Some(true));
    assert!(text.contains("agent__createOrg requires a complete org teamwork scaffold"));
    assert!(text.contains("@teamwork/"));
    assert!(text.contains("agents.md"));
    assert!(text.contains("coordination/KANBAN.md"));
    assert!(text.contains(".libragent/teamwork.json"));
    assert!(
        text.contains("workspace__writeFile"),
        "createOrg recovery must prefer writeFile under @teamwork/"
    );
    assert!(
        !text.contains("Prefer the teamwork skill")
            && !text.contains("Use the teamwork skill or init_task_force"),
        "createOrg recovery must not push shell+init_task_force as the primary path"
    );
}

#[test]
fn teamwork_scaffold_writefile_guidance_warns_against_container_shell_script() {
    let guidance = teamwork_scaffold_writefile_guidance();
    assert!(guidance.contains("workspace__writeFile"));
    assert!(guidance.contains("@teamwork/"));
    assert!(guidance.contains("init_task_force.py"));
    assert!(guidance.contains("Do not call workspace__runShell"));
}

#[test]
fn teamwork_scaffold_host_script_guidance_only_for_host_isolation() {
    assert!(
        teamwork_scaffold_optional_host_script_guidance(WorkspaceIsolationMode::Host).is_some()
    );
    assert!(
        teamwork_scaffold_optional_host_script_guidance(WorkspaceIsolationMode::Docker).is_none()
    );
}
