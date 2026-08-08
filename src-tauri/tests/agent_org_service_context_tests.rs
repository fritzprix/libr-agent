//! Org / child / active-session service-context composition.
//!
//! Standalone binary so Windows CI runs these cases too. Avoids:
//! - consolidated `tests/integration/` (AppHandle/WebView link → STATUS_ENTRYPOINT_NOT_FOUND)
//! - `common::setup_test_db*` / `reset_state()` (same crash on Windows)
//!
//! Uses `InMemorySessionRepository` and the same composition order as
//! `AgentServer::get_service_context` without constructing AgentServer.

use tauri_mcp_agent_lib::agent::ExecutionMode;
use tauri_mcp_agent_lib::mcp::builtin::agent::AGENT_DELEGATION_HEADER;
use tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode;
use tauri_mcp_agent_lib::repositories::{
    build_child_sessions_context, build_explicit_org_layer_context, format_active_sessions_notice,
    InMemorySessionRepository, SessionMetadata, SessionRepository, SessionStatus,
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
        assistant_id: None,
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
        execution_mode: ExecutionMode::Normal,
        workspace_override: None,
        workspace_isolation: WorkspaceIsolationMode::Host,
        docker_config: None,
        docker_container_name: None,
        docker_host_workspace_path: None,
    }
}

/// Mirrors `AgentServer::get_service_context` without constructing AgentServer.
async fn compose_service_context_prompt(
    repo: &InMemorySessionRepository,
    session_id: &str,
) -> String {
    let mut context_prompt = AGENT_DELEGATION_HEADER.to_string();

    if let Ok(Some(session)) = repo.get_session(session_id).await {
        if let Ok(children) = repo.get_child_sessions(session_id).await {
            if let Some(active_notice) = format_active_sessions_notice(&children) {
                context_prompt.push('\n');
                context_prompt.push_str(&active_notice);
            }
        }

        if session.org_id.is_some() {
            if let Ok(Some(org_layer_context)) =
                build_explicit_org_layer_context(repo, &session).await
            {
                context_prompt.push('\n');
                context_prompt.push_str(&org_layer_context);
            }
        }
    }

    context_prompt
}

#[tokio::test]
async fn org_service_context_includes_only_local_org_layer_for_org_sessions() {
    let repo = InMemorySessionRepository::new();

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
        context.contains("### Explicit Org Layer"),
        "org sessions should get an explicit org layer section"
    );
    assert!(context.contains("- Org: Alpha Org (ID: org-alpha)"));
    assert!(context.contains("- Depth: 1"));
    assert!(context.contains("- Parent: root — Root Coordinator"));
    assert!(
        context.contains("  - child-b — Writer [idle]"),
        "same-depth sibling should appear in local org layer with status"
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
    let repo = InMemorySessionRepository::new();

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
    let repo = InMemorySessionRepository::new();

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
    let repo = InMemorySessionRepository::new();

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
async fn service_context_composes_child_and_org_context() {
    let repo = InMemorySessionRepository::new();

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

    let prompt = compose_service_context_prompt(&repo, "parent-org-session").await;

    // Agent-facing ids are short tokens only (no `session-` prefix).
    // Fixture `child-org-session` (17 chars) → last 10: `rg-session`.
    assert!(prompt.contains("### Sub-Agent Sessions (1 total, reuse via messageToSession)"));
    assert!(prompt.contains("- **Ready to Reuse (Idle):**"));
    assert!(prompt.contains("  - `rg-session` (name: \"Child Analyst\")"));
    assert!(prompt.contains("### Explicit Org Layer"));
    assert!(prompt.contains("- Org: Beta Org (ID: org-beta)"));
    assert!(prompt.contains("## Agent Delegation"));
}

#[tokio::test]
async fn org_service_context_truncates_child_sessions_exceeding_max_limit() {
    let repo = InMemorySessionRepository::new();

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
    assert!(
        context.contains("- ... and 5 more omitted"),
        "should contain the truncation note indicating 5 more omitted"
    );
}

#[tokio::test]
async fn service_context_includes_active_sessions_notice() {
    let repo = InMemorySessionRepository::new();

    repo.upsert_session(&build_session(
        "parent-active-test",
        "Parent Coordinator",
        None,
        Some(0),
        None,
        None,
        None,
    ))
    .await
    .expect("parent session should persist");

    let mut child1 = build_session(
        "child-idle",
        "Active Child 1",
        Some("parent-active-test"),
        Some(1),
        None,
        None,
        None,
    );
    child1.status = SessionStatus::Idle;
    repo.upsert_session(&child1)
        .await
        .expect("child-idle should persist");

    let mut child2 = build_session(
        "child-paused",
        "Active Child 2",
        Some("parent-active-test"),
        Some(1),
        None,
        None,
        None,
    );
    child2.status = SessionStatus::Paused;
    repo.upsert_session(&child2)
        .await
        .expect("child-paused should persist");

    let mut child3 = build_session(
        "child-error",
        "Error Child",
        Some("parent-active-test"),
        Some(1),
        None,
        None,
        None,
    );
    child3.status = SessionStatus::Error;
    repo.upsert_session(&child3)
        .await
        .expect("child-error should persist");

    let prompt = compose_service_context_prompt(&repo, "parent-active-test").await;

    // `get_service_context` only appends the active-sessions notice (no ## Child Sessions).
    assert!(!prompt.contains("## Child Sessions"));

    assert!(prompt.contains("### Sub-Agent Sessions (3 total, reuse via messageToSession)"));
    assert!(prompt.contains("⚠️ **Reuse Existing Sessions First**:"));

    assert!(prompt.contains("- **Ready to Reuse (Idle):**"));
    assert!(prompt.contains("  These sessions are idle and ready for new instructions."));
    // Agent-facing ids are short tokens only (no `session-` prefix).
    //   child-idle (10) → child-idle; child-paused (12) → ild-paused; child-error (11) → hild-error
    assert!(prompt.contains("  - `child-idle` (name: \"Active Child 1\")"));

    assert!(prompt.contains("- **Suspended (Paused):**"));
    assert!(
        prompt.contains("  These sessions were suspended (e.g. waiting for input or approval).")
    );
    assert!(prompt.contains("  - `ild-paused` (name: \"Active Child 2\")"));

    assert!(prompt.contains("- **Failed (Error):**"));
    assert!(prompt.contains(
        "  These sessions encountered an error. Send a message to retry or recover them."
    ));
    assert!(prompt.contains("  - `hild-error` (name: \"Error Child\")"));
}
