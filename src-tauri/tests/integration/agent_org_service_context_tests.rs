use crate::common;

use tauri_mcp_agent_lib::repositories::{
    build_explicit_org_layer_context, SessionMetadata, SessionRepository, SessionStatus,
    SqliteSessionRepository,
};

fn build_session(
    id: &str,
    name: &str,
    parent_session_id: Option<&str>,
    depth: Option<u32>,
    org_id: Option<&str>,
    org_name: Option<&str>,
    org_root_session_id: Option<&str>,
) -> SessionMetadata {
    SessionMetadata {
        id: id.to_string(),
        name: Some(name.to_string()),
        status: SessionStatus::Idle,
        model: "gpt-5.4".to_string(),
        provider: "openai".to_string(),
        agent_config: None,
        parent_session_id: parent_session_id.map(str::to_string),
        lineage_id: Some("lineage-1".to_string()),
        depth,
        max_depth: None,
        max_fanout: None,
        org_id: org_id.map(str::to_string),
        org_name: org_name.map(str::to_string),
        org_root_session_id: org_root_session_id.map(str::to_string),
        created_at: 1,
        updated_at: 1,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        yolo_mode: false,
        unsafe_mode: false,
        workspace_override: None,
    }
}

#[tokio::test]
async fn org_service_context_includes_only_local_org_layer_for_org_sessions() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSessionRepository::new(db.clone());

    repo.upsert_session(&build_session(
        "root",
        "Root Coordinator",
        None,
        Some(0),
        Some("org-alpha"),
        Some("Alpha Org"),
        Some("root"),
    ))
    .await
    .expect("root session should persist");
    repo.upsert_session(&build_session(
        "child-a",
        "Analyst",
        Some("root"),
        Some(1),
        Some("org-alpha"),
        Some("Alpha Org"),
        Some("root"),
    ))
    .await
    .expect("child session should persist");
    repo.upsert_session(&build_session(
        "child-b",
        "Writer",
        Some("root"),
        Some(1),
        Some("org-alpha"),
        Some("Alpha Org"),
        Some("root"),
    ))
    .await
    .expect("sibling session should persist");
    repo.upsert_session(&build_session(
        "other-depth",
        "Reviewer",
        Some("child-a"),
        Some(2),
        Some("org-alpha"),
        Some("Alpha Org"),
        Some("root"),
    ))
    .await
    .expect("different depth session should persist");

    let context = build_explicit_org_layer_context(&repo, "child-a")
        .await
        .expect("org layer context should build")
        .expect("org session should receive org layer context");

    assert!(
        context.contains("## Explicit Org Layer"),
        "org sessions should get an explicit org layer section"
    );
    assert!(context.contains("- Org: Alpha Org"));
    assert!(context.contains("- Depth: 1"));
    assert!(context.contains("- Parent: root — Root Coordinator"));
    assert!(
        context.contains("  - child-b — Writer"),
        "same-depth sibling should appear in local org layer"
    );
    assert!(
        !context.contains("other-depth"),
        "deeper descendants must not leak into the local org layer"
    );
    assert!(
        !context.contains("None"),
        "prompt should not contain useless placeholder values"
    );
}

#[tokio::test]
async fn org_service_context_omits_org_section_for_non_org_sessions() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSessionRepository::new(db.clone());

    repo.upsert_session(&build_session(
        "solo",
        "Solo Session",
        None,
        Some(0),
        None,
        None,
        None,
    ))
    .await
    .expect("solo session should persist");

    let context = build_explicit_org_layer_context(&repo, "solo")
        .await
        .expect("org layer context should build");

    assert!(
        context.is_none(),
        "non-org sessions should not receive an org section"
    );
}

#[tokio::test]
async fn org_service_context_omits_org_section_when_root_session_id_is_missing() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSessionRepository::new(db.clone());

    repo.upsert_session(&build_session(
        "partial-org",
        "Partial Org Session",
        None,
        Some(0),
        Some("org-alpha"),
        Some("Alpha Org"),
        None,
    ))
    .await
    .expect("partial org session should persist");

    let context = build_explicit_org_layer_context(&repo, "partial-org")
        .await
        .expect("org layer context should build");

    assert!(
        context.is_none(),
        "sessions missing org_root_session_id must not receive org context"
    );
}
