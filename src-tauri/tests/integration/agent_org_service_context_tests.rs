use crate::common;

use tauri_mcp_agent_lib::repositories::{
    build_child_sessions_context, build_explicit_org_layer_context, SessionMetadata,
    SessionRepository, SessionStatus, SqliteSessionRepository,
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

    let session = repo.get_session("child-a").await.unwrap().unwrap();
    let context = build_explicit_org_layer_context(&repo, &session)
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

    let session = repo.get_session("solo").await.unwrap().unwrap();
    let context = build_explicit_org_layer_context(&repo, &session)
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

    let session = repo.get_session("partial-org").await.unwrap().unwrap();
    let context = build_explicit_org_layer_context(&repo, &session)
        .await
        .expect("org layer context should build");

    assert!(
        context.is_none(),
        "sessions missing org_root_session_id must not receive org context"
    );
}

#[tokio::test]
async fn org_service_context_includes_child_sessions_even_without_org() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSessionRepository::new(db.clone());

    // Parent session with no org info
    repo.upsert_session(&build_session(
        "parent-x",
        "Parent Coordinator",
        None,
        Some(0),
        None,
        None,
        None,
    ))
    .await
    .expect("parent session should persist");

    // Child session a
    repo.upsert_session(&build_session(
        "child-1",
        "Child Agent A",
        Some("parent-x"),
        Some(1),
        None,
        None,
        None,
    ))
    .await
    .expect("child session a should persist");

    // Child session b (with status: Busy to verify status formatting)
    let mut child2 = build_session(
        "child-2",
        "Child Agent B",
        Some("parent-x"),
        Some(1),
        None,
        None,
        None,
    );
    child2.status = SessionStatus::Busy;
    repo.upsert_session(&child2)
        .await
        .expect("child session b should persist");

    let context = build_child_sessions_context(&repo, "parent-x")
        .await
        .expect("child context should build")
        .expect("child context should not be empty");

    assert!(
        context.contains("## Child Sessions"),
        "context should contain Child Sessions header"
    );
    assert!(
        context.contains("- child-1 — Child Agent A (status: idle)"),
        "should contain child-1 with correct status formatting"
    );
    assert!(
        context.contains("- child-2 — Child Agent B (status: busy)"),
        "should contain child-2 with correct status formatting"
    );
}

#[tokio::test]
async fn agent_server_get_service_context_composes_child_and_org_context() {
    use std::sync::Arc;
    use tauri_mcp_agent_lib::mcp::builtin::agent::AgentServer;
    use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;

    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSessionRepository::new(db.clone());

    // 1. Create a parent session with org info
    repo.upsert_session(&build_session(
        "parent-org-session",
        "Parent Coordinator",
        None,
        Some(0),
        Some("org-beta"),
        Some("Beta Org"),
        Some("parent-org-session"),
    ))
    .await
    .expect("parent session should persist");

    // 2. Create a child session
    repo.upsert_session(&build_session(
        "child-org-session",
        "Child Analyst",
        Some("parent-org-session"),
        Some(1),
        Some("org-beta"),
        Some("Beta Org"),
        Some("parent-org-session"),
    ))
    .await
    .expect("child session should persist");

    // 3. Initialize AgentServer
    let server = AgentServer::new("parent-org-session".to_string(), Arc::new(db), None)
        .await
        .expect("AgentServer should initialize");

    // 4. Retrieve service context
    let context = server.get_service_context(None).await;
    let prompt = context.context_prompt;

    // 5. Assert it contains both child and org details
    assert!(prompt.contains("## Child Sessions"));
    assert!(prompt.contains("- child-org-session — Child Analyst (status: idle)"));
    assert!(prompt.contains("## Explicit Org Layer"));
    assert!(prompt.contains("- Org: Beta Org"));
}

#[tokio::test]
async fn org_service_context_truncates_child_sessions_exceeding_max_limit() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSessionRepository::new(db.clone());

    // Create a parent session
    repo.upsert_session(&build_session(
        "parent-limit",
        "Parent Coordinator",
        None,
        Some(0),
        None,
        None,
        None,
    ))
    .await
    .expect("parent session should persist");

    // Create 25 child sessions
    for i in 1..=25 {
        let child_id = format!("child-{}", i);
        let child_name = format!("Child Agent {}", i);
        repo.upsert_session(&build_session(
            &child_id,
            &child_name,
            Some("parent-limit"),
            Some(1),
            None,
            None,
            None,
        ))
        .await
        .expect("child session should persist");
    }

    let context = build_child_sessions_context(&repo, "parent-limit")
        .await
        .expect("child context should build")
        .expect("child context should not be empty");

    assert!(
        context.contains("## Child Sessions"),
        "context should contain Child Sessions header"
    );

    // Verify it contains the truncation note indicating 5 more omitted
    assert!(
        context.contains("- ... and 5 more omitted"),
        "should contain the truncation note indicating 5 more omitted"
    );
}
