mod common;

use tauri_mcp_agent_lib::mcp::builtin::agent::handlers::{
    create_org_preflight, existing_explicit_org_identity, CreateOrgPreflight,
};
use tauri_mcp_agent_lib::repositories::{
    SessionMetadata, SessionRepository, SessionStatus, SqliteSessionRepository,
};
use tauri_mcp_agent_lib::session::{
    provision_teamwork_workspace_for_session, teamwork_workspace_status, SessionManager,
    TeamworkWorkspaceStatus,
};

fn make_session(session_id: &str) -> SessionMetadata {
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("Org Root".to_string()),
        status: SessionStatus::Idle,
        model: "gpt-5.4".to_string(),
        provider: "openai".to_string(),
        agent_config: None,
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
        yolo_mode: false,
        workspace_override: None,
    }
}

#[test]
fn existing_org_identity_bypasses_new_org_workspace_enforcement() {
    let mut session = make_session("existing-org-root");
    session.org_id = Some("org-alpha".to_string());
    session.org_name = Some("Alpha Org".to_string());
    session.org_root_session_id = Some("existing-org-root".to_string());

    let existing = existing_explicit_org_identity(&session);
    assert_eq!(
        existing,
        Some((
            "org-alpha".to_string(),
            "Alpha Org".to_string(),
            "existing-org-root".to_string()
        ))
    );

    let workspace_status = TeamworkWorkspaceStatus {
        effective_workspace: std::path::PathBuf::from("/repo"),
        dedicated_workspace: std::path::PathBuf::from("/app/teamwork"),
    };
    assert_eq!(
        create_org_preflight(&session, &workspace_status),
        CreateOrgPreflight::ExistingOrg {
            org_id: "org-alpha".to_string(),
            org_name: "Alpha Org".to_string(),
            root_session_id: "existing-org-root".to_string(),
        }
    );
}

#[test]
fn new_org_requires_dedicated_teamwork_workspace_before_creation() {
    let session = make_session("new-org-root");
    let workspace_status = TeamworkWorkspaceStatus {
        effective_workspace: std::path::PathBuf::from("/repo"),
        dedicated_workspace: std::path::PathBuf::from("/app/teamwork"),
    };

    assert_eq!(
        create_org_preflight(&session, &workspace_status),
        CreateOrgPreflight::RequiresDedicatedWorkspace {
            effective_workspace: std::path::PathBuf::from("/repo"),
            dedicated_workspace: std::path::PathBuf::from("/app/teamwork"),
        }
    );
}

#[test]
fn new_org_can_proceed_once_dedicated_teamwork_workspace_is_active() {
    let session = make_session("new-org-root");
    let workspace_path = std::path::PathBuf::from("/app/teamwork");
    let workspace_status = TeamworkWorkspaceStatus {
        effective_workspace: workspace_path.clone(),
        dedicated_workspace: workspace_path,
    };

    assert_eq!(
        create_org_preflight(&session, &workspace_status),
        CreateOrgPreflight::Proceed
    );
}

#[test]
fn existing_org_still_reports_existing_org_even_when_workspace_mismatch_exists() {
    let mut session = make_session("existing-org-root");
    session.org_id = Some("org-alpha".to_string());
    session.org_name = Some("Alpha Org".to_string());
    session.org_root_session_id = Some("existing-org-root".to_string());

    let workspace_status = TeamworkWorkspaceStatus {
        effective_workspace: std::path::PathBuf::from("/repo"),
        dedicated_workspace: std::path::PathBuf::from("/app/teamwork"),
    };

    assert_eq!(
        create_org_preflight(&session, &workspace_status),
        CreateOrgPreflight::ExistingOrg {
            org_id: "org-alpha".to_string(),
            org_name: "Alpha Org".to_string(),
            root_session_id: "existing-org-root".to_string(),
        }
    );
}

#[tokio::test]
async fn org_root_defaults_to_session_workspace_until_teamwork_workspace_is_provisioned() {
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

    let status = teamwork_workspace_status(&session_manager, session_id);
    assert!(
        !status.uses_dedicated_teamwork_workspace(),
        "root sessions should not look teamwork-ready before explicit preparation"
    );
    assert!(
        status
            .effective_workspace
            .ends_with(std::path::Path::new("workspaces").join(session_id)),
        "expected default session workspace before preparation: {}",
        status.effective_workspace.display()
    );
    assert!(
        status
            .dedicated_workspace
            .ends_with(std::path::Path::new("teamwork-workspaces").join(session_id)),
        "expected dedicated teamwork workspace target: {}",
        status.dedicated_workspace.display()
    );
}

#[tokio::test]
async fn org_root_uses_dedicated_teamwork_workspace_after_preparation() {
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

    let teamwork_workspace =
        provision_teamwork_workspace_for_session(&repo, &session_manager, session_id)
            .await
            .expect("teamwork workspace should provision");

    let status = teamwork_workspace_status(&session_manager, session_id);
    assert!(
        status.uses_dedicated_teamwork_workspace(),
        "root sessions should become teamwork-ready after explicit preparation"
    );
    assert_eq!(status.effective_workspace, teamwork_workspace);
    assert_eq!(status.dedicated_workspace, teamwork_workspace);
}
