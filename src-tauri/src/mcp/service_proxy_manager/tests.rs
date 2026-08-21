use super::*;
use crate::entity::{
    assistant, knowledge, mcp_server, planning_goal, planning_scratchpad, planning_todo, playbook,
    session,
};
use sea_orm::{ConnectionTrait, Database, EntityTrait, Schema, Set};
use serde_json::json;
use std::sync::Arc;

#[test]
fn test_decide_proxy_readiness_state_truth_table() {
    // Missing Proxy
    assert_eq!(
        decide_proxy_readiness_state(false, false),
        ProxyReadinessState::MissingProxy
    );
    assert_eq!(
        decide_proxy_readiness_state(false, true),
        ProxyReadinessState::MissingProxy
    );

    // Proxy exists, no readiness signal -> Immediately Ready
    assert_eq!(
        decide_proxy_readiness_state(true, false),
        ProxyReadinessState::Ready
    );

    // Proxy exists, has readiness signal -> Must await signal
    assert_eq!(
        decide_proxy_readiness_state(true, true),
        ProxyReadinessState::AwaitSignal
    );
}

struct TestHarness {
    _guard: tokio::sync::MutexGuard<'static, ()>,
    manager: Arc<MCPServiceProxyManager>,
}

fn test_session_active_model(id: impl Into<String>) -> session::ActiveModel {
    session::ActiveModel {
        id: Set(id.into()),
        created_at: Set(chrono::Utc::now().timestamp()),
        updated_at: Set(0),
        status: Set("idle".to_string()),
        model: Set("gpt-4".to_string()),
        provider: Set("openai".to_string()),
        execution_mode: Set("normal".to_string()),
        workspace_isolation: Set("host".to_string()),
        is_bookmarked: Set(false),
        ..Default::default()
    }
}

async fn insert_test_session(manager: &MCPServiceProxyManager, session_id: &str) {
    let assistant_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();

    assistant::Entity::insert(assistant::ActiveModel {
        id: Set(assistant_id.clone()),
        name: Set(format!("Test Assistant for {session_id}")),
        config: Set("{}".to_string()),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    })
    .exec(&*manager.db)
    .await
    .expect("failed to insert test assistant");

    let mut model = test_session_active_model(session_id);
    model.assistant_id = Set(Some(assistant_id));

    session::Entity::insert(model)
        .exec(&*manager.db)
        .await
        .expect("failed to insert test session");
}

async fn create_test_harness() -> TestHarness {
    let guard = crate::state::lock_test_global_state().await;
    crate::state::reset_state();

    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory database");

    let schema = Schema::new(db.get_database_backend());

    for create in [
        schema.create_table_from_entity(session::Entity),
        schema.create_table_from_entity(playbook::Entity),
        schema.create_table_from_entity(assistant::Entity),
        schema.create_table_from_entity(knowledge::Entity),
        schema.create_table_from_entity(planning_goal::Entity),
        schema.create_table_from_entity(planning_todo::Entity),
        schema.create_table_from_entity(planning_scratchpad::Entity),
        schema.create_table_from_entity(crate::entity::settings::Entity),
        schema.create_table_from_entity(mcp_server::Entity),
    ] {
        db.execute(db.get_database_backend().build(&create))
            .await
            .expect("Failed to create test table");
    }

    crate::lifecycle::repositories::init_repositories(&db).await;
    crate::state::init_session_bus(crate::agent::session_bus::SessionBus::new());
    crate::state::init_concurrency_gate(crate::agent::concurrency::ConcurrencyGate::new(
        crate::agent::concurrency::DEFAULT_MAX_ACTIVE_AGENTS,
        crate::agent::concurrency::DEFAULT_MAX_SUSPENDED_AGENTS,
        crate::agent::concurrency::DEFAULT_MAX_ACTIVE_PROCESSES,
        crate::agent::concurrency::DEFAULT_MAX_SUSPENDED_PROCESSES,
    ));

    // Create a minimal SessionManager
    let session_manager = Arc::new(crate::session::SessionManager::new().unwrap());

    TestHarness {
        _guard: guard,
        manager: Arc::new(MCPServiceProxyManager::new(Arc::new(db), session_manager)),
    }
}

#[tokio::test]
async fn test_phase3_playbook_and_assistant_integration() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;

    // Create session 1 with all Phase 3 tools
    let session1 = "test-session-1".to_string();
    let tool_ids1 = vec!["playbook".to_string(), "agent".to_string()];

    // Insert session 1 into sessions table
    insert_test_session(&manager, &session1).await;

    manager
        .create_proxy(session1.clone(), tool_ids1, vec![], None)
        .await
        .unwrap();

    // Test 1: Save a playbook in session 1
    let playbook_result = manager
        .call_tool(
            &session1,
            "playbook__createPlaybook",
            json!({
                "goal": "Test Workflow",
                "initialCommand": "test",
                "workflow": [
                    {
                        "description": "Step 1",
                        "action": { "toolName": "test", "purpose": "test" },
                        "outputVariable": "out"
                    }
                ],
                "successCriteria": {
                    "description": "Success"
                }
            }),
        )
        .await
        .unwrap();

    assert!(
        playbook_result.error.is_none(),
        "Playbook save should succeed"
    );

    // Test 2: Create an assistant (global scope)
    let assistant_result = manager
        .call_tool(
            &session1,
            "agent__createAgent",
            json!({
                "name": "Test Assistant"
            }),
        )
        .await
        .unwrap();

    assert!(
        assistant_result.error.is_none(),
        "Assistant create should succeed: {:?}",
        assistant_result.error
    );
    let _created_assistant_id = assistant_result
        .result
        .as_ref()
        .and_then(|result| match result {
            crate::mcp::types::MCPResponseResult::ToolCall(tool_result) => tool_result
                .structured_content
                .as_ref()
                .and_then(|data| data.get("id"))
                .and_then(|id| id.as_str())
                .map(str::to_string),
            _ => None,
        })
        .expect("createAgent should return structured assistant id");

    // Create session 2 with same tools
    let session2 = "test-session-2".to_string();
    let tool_ids2 = vec!["playbook".to_string(), "agent".to_string()];

    // Insert session 2 into sessions table
    insert_test_session(&manager, &session2).await;

    manager
        .create_proxy(session2.clone(), tool_ids2, vec![], None)
        .await
        .unwrap();

    // Test 3: Verify playbook isolation (session 2 can't see session 1's playbook)
    let list_result = manager
        .call_tool(&session2, "playbook__listPlaybooks", json!({}))
        .await
        .unwrap();

    assert!(list_result.error.is_none());
    let result_data = list_result.result.unwrap();

    // Extract text content from ToolCall result
    let text_content = match result_data {
        crate::mcp::types::MCPResponseResult::ToolCall(ref result) => {
            if let Some(content) = &result.content {
                if let crate::mcp::types::MCPContent::Text { text, .. } = &content[0] {
                    text
                } else {
                    panic!("Expected Text content")
                }
            } else {
                panic!("Expected content")
            }
        }
        _ => panic!("Expected ToolCall result"),
    };
    assert!(
        text_content.contains("No playbooks found"),
        "Session 2 should have 0 playbooks, got: {}",
        text_content
    );

    // Test 4: Verify assistant is global (session 2 can see the assistant)
    let get_assistant_result = manager
        .call_tool(
            &session2,
            "agent__listAgents",
            json!({
                "type": "configs"
            }),
        )
        .await
        .unwrap();

    assert!(get_assistant_result.error.is_none());
    let assistant = get_assistant_result.result.unwrap();
    let assistant_text = match assistant {
        crate::mcp::types::MCPResponseResult::ToolCall(ref result) => {
            if let Some(content) = &result.content {
                if let crate::mcp::types::MCPContent::Text { text, .. } = &content[0] {
                    text
                } else {
                    panic!("Expected Text content")
                }
            } else {
                panic!("Expected content")
            }
        }
        _ => panic!("Expected ToolCall result"),
    };
    assert!(
        assistant_text.contains("Test Assistant"),
        "Session 2 should see the global assistant"
    );

    // Test 5: Save same playbook ID in session 2 (allowed due to composite PK)
    let playbook2_result = manager
        .call_tool(
            &session2,
            "playbook__createPlaybook",
            json!({
                "goal": "Session 2 Workflow",
                "initialCommand": "test2",
                "workflow": [
                    {
                        "description": "Step 1",
                        "action": { "toolName": "test", "purpose": "test" },
                        "outputVariable": "out"
                    }
                ],
                "successCriteria": {
                    "description": "Success"
                }
            }),
        )
        .await
        .unwrap();

    assert!(
        playbook2_result.error.is_none(),
        "Session 2 should save playbook with same ID"
    );

    // Test 6: Verify each session sees its own playbook
    let list_playbook1 = manager
        .call_tool(&session1, "playbook__listPlaybooks", json!({}))
        .await
        .unwrap();

    let playbook1_result = list_playbook1.result.unwrap();
    let playbook1_text = match playbook1_result {
        crate::mcp::types::MCPResponseResult::ToolCall(ref result) => {
            if let Some(content) = &result.content {
                if let crate::mcp::types::MCPContent::Text { text, .. } = &content[0] {
                    text
                } else {
                    panic!("Expected Text content")
                }
            } else {
                panic!("Expected content")
            }
        }
        _ => panic!("Expected ToolCall result"),
    };
    assert!(
        playbook1_text.contains("Test Workflow"),
        "Session 1 should see its own playbook"
    );

    let list_playbook2 = manager
        .call_tool(&session2, "playbook__listPlaybooks", json!({}))
        .await
        .unwrap();

    let playbook2_result = list_playbook2.result.unwrap();
    let playbook2_text = match playbook2_result {
        crate::mcp::types::MCPResponseResult::ToolCall(ref result) => {
            if let Some(content) = &result.content {
                if let crate::mcp::types::MCPContent::Text { text, .. } = &content[0] {
                    text
                } else {
                    panic!("Expected Text content")
                }
            } else {
                panic!("Expected content")
            }
        }
        _ => panic!("Expected ToolCall result"),
    };
    assert!(
        playbook2_text.contains("Session 2 Workflow"),
        "Session 2 should see its own playbook"
    );

    // Cleanup
    manager.destroy_proxy(&session1).await;
    manager.destroy_proxy(&session2).await;
}

#[tokio::test]
async fn test_phase3_concurrent_operations() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;

    // Create 3 concurrent sessions
    let sessions = vec![
        "concurrent-1".to_string(),
        "concurrent-2".to_string(),
        "concurrent-3".to_string(),
    ];

    // Insert sessions into database and create proxies
    for session_id in &sessions {
        insert_test_session(&manager, session_id).await;

        let tool_ids = vec!["playbook".to_string(), "agent".to_string()];
        manager
            .create_proxy(session_id.clone(), tool_ids, vec![], None)
            .await
            .unwrap();
    }

    // Execute concurrent playbook saves
    let mut handles = vec![];
    for (idx, session_id) in sessions.iter().enumerate() {
        let mgr = manager.clone();
        let sid = session_id.clone();

        let handle = tokio::spawn(async move {
            mgr.call_tool(
                &sid,
                "playbook__createPlaybook",
                json!({
                    "goal": format!("Playbook {}", idx),
                    "initialCommand": format!("test {}", idx),
                    "workflow": [
                        {
                            "description": format!("Step {}", idx),
                            "action": { "toolName": "test", "purpose": "test" },
                            "outputVariable": "out"
                        }
                    ],
                    "successCriteria": {
                        "description": "Success"
                    }
                }),
            )
            .await
        });

        handles.push(handle);
    }

    // Wait for all operations to complete
    for handle in handles {
        let result = handle.await.unwrap().unwrap();
        assert!(
            result.error.is_none(),
            "Concurrent playbook save should succeed"
        );
    }

    // Verify each session has its own playbooks
    for (idx, session_id) in sessions.iter().enumerate() {
        let list_result = manager
            .call_tool(session_id, "playbook__listPlaybooks", json!({}))
            .await
            .unwrap();

        let result_data = list_result.result.unwrap();
        let text_content = match result_data {
            crate::mcp::types::MCPResponseResult::ToolCall(ref result) => {
                if let Some(content) = &result.content {
                    if let crate::mcp::types::MCPContent::Text { text, .. } = &content[0] {
                        text
                    } else {
                        panic!("Expected Text content")
                    }
                } else {
                    panic!("Expected content")
                }
            }
            _ => panic!("Expected ToolCall result"),
        };

        // Each session should have exactly 1 playbook
        assert!(
            text_content.contains("Found 1 playbook"),
            "Session {} should have exactly 1 playbook, got: {}",
            idx,
            text_content
        );
    }

    // Cleanup
    for session_id in &sessions {
        manager.destroy_proxy(session_id).await;
    }
}

#[tokio::test]
async fn test_phase3_all_servers_integration() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;

    let session_id = "integration-test".to_string();

    insert_test_session(&manager, &session_id).await;

    // Create proxy with ALL builtin servers
    let all_tools = vec![
        "bootstrap".to_string(),
        "attachments".to_string(),
        "planning".to_string(),
        "playbook".to_string(),
        "agent".to_string(),
    ];

    manager
        .create_proxy(session_id.clone(), all_tools, vec![], None)
        .await
        .unwrap();

    // Test Bootstrap (stateless)
    let bootstrap_result = manager
        .call_tool(&session_id, "setup-wizard__detectPlatform", json!({}))
        .await
        .unwrap();
    assert!(bootstrap_result.error.is_none(), "Bootstrap should work");

    // Test Knowledge (session-scoped)
    let knowledge_result = manager
        .call_tool(
            &session_id,
            "attachments__addAttachment",
            json!({
                "title": "Test Knowledge",
                "content": "Integration test content",
                "tags": ["test", "integration"]
            }),
        )
        .await
        .unwrap();
    assert!(
        knowledge_result.error.is_none(),
        "Knowledge save should work"
    );

    // Test Planning (session-scoped)
    let planning_result = manager
        .call_tool(
            &session_id,
            "planning__createGoal",
            json!({
                "goal": "Complete Phase 3 integration"
            }),
        )
        .await
        .unwrap();
    assert!(planning_result.error.is_none(), "Planning should work");

    // Test Playbook (session-scoped)
    let playbook_result = manager
        .call_tool(
            &session_id,
            "playbook__createPlaybook",
            json!({
                "goal": "Integration Playbook",
                "initialCommand": "test",
                "workflow": [
                    {
                        "description": "Step 1",
                        "action": { "toolName": "test", "purpose": "test" },
                        "outputVariable": "out"
                    }
                ],
                "successCriteria": {
                    "description": "Success"
                }
            }),
        )
        .await
        .unwrap();
    assert!(playbook_result.error.is_none(), "Playbook should work");

    // Test Assistant (global-scoped)
    let assistant_result = manager
        .call_tool(
            &session_id,
            "agent__createAgent",
            json!({
                "name": "Integration Test Assistant"
            }),
        )
        .await
        .unwrap();
    assert!(assistant_result.error.is_none(), "Assistant should work");

    // Verify proxy has all servers
    let proxy = manager.get_proxy(&session_id).await.unwrap();
    assert_eq!(
        proxy.builtin_server_count(),
        5,
        "Should have all 5 builtin servers"
    );

    // Cleanup
    manager.destroy_proxy(&session_id).await;
    assert_eq!(
        manager.proxy_count().await,
        0,
        "All proxies should be destroyed"
    );
}

#[tokio::test]
async fn test_empty_mcp_server_ids_means_no_external_servers() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;

    let session_id = "no-external-test".to_string();

    insert_test_session(&manager, &session_id).await;

    // Create proxy with builtin tools but EMPTY mcp_server_ids
    // This should result in NO external MCP servers being loaded
    let tool_ids = vec!["bootstrap".to_string()];
    let mcp_server_ids = vec![]; // Empty = no external servers

    let proxy = manager
        .create_proxy(session_id.clone(), tool_ids, mcp_server_ids, None)
        .await
        .unwrap();

    // Verify: Only builtin tools, no external tools
    assert_eq!(
        proxy.builtin_server_count(),
        1,
        "Should have 1 builtin server"
    );

    // Verify: No external stdio tools
    let stdio_tools = proxy.get_session_stdio_tools().await;
    assert_eq!(
        stdio_tools.len(),
        0,
        "Should have 0 external stdio tools (mcp_server_ids is empty)"
    );

    // Verify: No external HTTP tools
    let http_tools = proxy.get_session_http_tools().await;
    assert_eq!(
        http_tools.len(),
        0,
        "Should have 0 external HTTP tools (mcp_server_ids is empty)"
    );

    log::info!("✅ Verified: Empty mcp_server_ids = no external servers loaded");

    // Cleanup
    manager.destroy_proxy(&session_id).await;
}

// ── Regression tests for proxy_readiness / wait_until_proxy_ready ─────────────

/// Regression: builtin-only sessions must be considered immediately ready.
///
/// Before the fix, if `start_workflow` called `wait_until_proxy_ready` for a
/// session with no external servers it would find "no entry" in the map.
/// The method should treat that as ready and return `Ok(())` without blocking.
#[tokio::test]
async fn test_proxy_ready_immediately_for_builtin_only_sessions() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "readiness-builtin-only";

    insert_test_session(&manager, session_id).await;

    // No external MCP server IDs → background task is never spawned.
    manager
        .create_proxy(
            session_id.to_string(),
            vec!["setup-wizard".to_string()],
            vec![],
            None,
        )
        .await
        .unwrap();

    // Must resolve within a tight wall-clock budget (100 ms) — it should be instant.
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        manager.wait_until_proxy_ready(session_id, 10),
    )
    .await;

    assert!(
        result.is_ok(),
        "wait_until_proxy_ready must not block for builtin-only sessions"
    );
    assert!(
        result.unwrap().is_ok(),
        "wait_until_proxy_ready must succeed"
    );

    // No entry should exist in the readiness map.
    assert_eq!(
        manager.readiness_entry_count().await,
        0,
        "Builtin-only sessions must not add an entry to proxy_readiness"
    );

    manager.destroy_proxy(session_id).await;
}

/// Regression: wait_until_proxy_ready must reject missing proxies.
///
/// A missing readiness entry only means "builtin-only and already ready" when a
/// proxy actually exists. Destroyed or never-created sessions must not be treated
/// as ready, or workflow code will race into LLM/tool execution without external
/// MCP managers.
#[tokio::test]
async fn test_wait_fails_when_proxy_does_not_exist() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "readiness-missing-proxy";

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        manager.wait_until_proxy_ready(session_id, 1),
    )
    .await;

    assert!(
        result.is_ok(),
        "wait_until_proxy_ready must fail fast when no proxy exists"
    );
    assert!(
        result
            .unwrap()
            .is_err_and(|error| error.contains("No MCP proxy exists")),
        "wait_until_proxy_ready must report the missing proxy instead of pretending the session is ready"
    );
}

/// Regression: wait_until_proxy_ready must block until the background signal fires.
///
/// This verifies the core race-condition fix: a session with external MCP servers
/// has a pending readiness entry. start_workflow must wait until tool loading
/// completes before proceeding.
#[tokio::test]
async fn test_wait_blocks_until_ready_signal_fires() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "readiness-signal-test";

    insert_test_session(&manager, session_id).await;
    manager
        .create_proxy(
            session_id.to_string(),
            vec!["playbook".to_string()],
            vec![],
            None,
        )
        .await
        .unwrap();
    manager
        .mark_runtime_proxy_not_ready_for_test(session_id)
        .await;

    // Inject a pending (false) entry simulating a session with external servers
    // whose background loading has not finished yet.
    let tx = manager.inject_pending_readiness_for_test(session_id).await;

    assert_eq!(
        manager.readiness_entry_count().await,
        1,
        "Pending readiness entry should exist"
    );

    // Spawn a task that signals ready after a short delay.
    let tx_bg = tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;
        // send_replace updates even if no other receivers are currently subscribed.
        tx_bg.send_replace(true);
    });

    let start = std::time::Instant::now();
    let result = manager.wait_until_proxy_ready(session_id, 5).await;
    let elapsed = start.elapsed();

    assert!(
        result.is_ok(),
        "wait_until_proxy_ready must succeed after signal"
    );
    assert!(
        elapsed >= std::time::Duration::from_millis(40),
        "Must have waited for the signal (elapsed: {:?})",
        elapsed
    );
}

/// Regression: discovery completion must mark ready even when no waiter holds a
/// watch receiver (soft get_available_tools timeout dropped its subscription).
///
/// tokio::watch::Sender::send does not update the value when receiver_count == 0,
/// which previously left resume/start waiting on a forever-false signal.
#[tokio::test]
async fn test_ready_signal_survives_zero_receivers() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "readiness-zero-receivers";

    insert_test_session(&manager, session_id).await;
    manager
        .create_proxy(
            session_id.to_string(),
            vec!["playbook".to_string()],
            vec![],
            None,
        )
        .await
        .unwrap();
    manager
        .mark_runtime_proxy_not_ready_for_test(session_id)
        .await;

    let tx = manager.inject_pending_readiness_for_test(session_id).await;

    // Reproduce production race: no live receivers when discovery completes.
    assert_eq!(tx.receiver_count(), 0);
    assert!(
        tx.send(true).is_err(),
        "watch::send must fail with zero receivers (tokio contract)"
    );
    assert!(
        !*tx.borrow(),
        "failed send must leave the readiness value false"
    );

    // Production fix path.
    tx.send_replace(true);
    assert!(
        *tx.borrow(),
        "send_replace must publish ready with zero receivers"
    );

    let result = manager.wait_until_proxy_ready(session_id, 1).await;
    assert!(
        result.is_ok(),
        "wait must observe ready after send_replace with zero prior receivers"
    );
}

/// Regression: wait_until_proxy_ready must not block the session forever when a
/// readiness signal never fires. If a proxy already exists, finalize Session Ready
/// (TimedOut pending servers) so UI/backend Ready stay aligned.
#[tokio::test]
async fn test_wait_proceeds_degraded_if_never_signaled() {
    use crate::agent::runtime_state::{SessionRuntimeServerStatus, SessionRuntimeTransport};

    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "readiness-timeout-test";

    insert_test_session(&manager, session_id).await;
    manager
        .create_proxy(
            session_id.to_string(),
            vec!["playbook".to_string()],
            vec![],
            None,
        )
        .await
        .unwrap();

    // Seed a non-terminal external server and clear Session Ready.
    {
        let mut state = manager.get_runtime_state(session_id).await;
        state.upsert_server(
            "slow-stdio",
            SessionRuntimeTransport::Stdio,
            SessionRuntimeServerStatus::DiscoveringTools,
            0,
            None,
        );
        state.proxy.ready = false;
        state.initialization.result =
            crate::agent::runtime_state::SessionRuntimeInitResult::Pending;
        state.phase = crate::agent::runtime_state::SessionRuntimePhase::Initializing;
        manager.set_runtime_state(session_id, state, None).await;
    }

    // Inject a pending entry but intentionally never send true.
    let _tx = manager.inject_pending_readiness_for_test(session_id).await;

    // Use a 1-second timeout so the test doesn't hang.
    let result = manager.wait_until_proxy_ready(session_id, 1).await;

    assert!(
        result.is_ok(),
        "Must proceed degraded when proxy exists but readiness never signals"
    );
    assert_eq!(
        manager.readiness_entry_count().await,
        0,
        "Timed-out readiness wait must be cleared so later calls do not re-block"
    );

    let state = manager.get_runtime_state(session_id).await;
    assert!(
        state.proxy.ready,
        "waiter timeout must raise Session Ready (proxy.ready)"
    );
    assert!(
        state.servers.iter().any(|server| {
            server.name == "slow-stdio" && server.status == SessionRuntimeServerStatus::TimedOut
        }),
        "pending servers must be marked TimedOut on waiter finalize"
    );
}

/// Without a proxy, readiness timeout remains a hard error (nothing to proceed with).
#[tokio::test]
async fn test_wait_errors_if_timed_out_without_proxy() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "readiness-timeout-no-proxy";

    let _tx = manager.inject_pending_readiness_for_test(session_id).await;

    let result = manager.wait_until_proxy_ready(session_id, 1).await;

    assert!(result.is_err(), "Must error when no proxy exists");
    assert!(
        result.unwrap_err().contains("timed out"),
        "Error message must mention timeout"
    );
}

/// Verify that lazy builtin proxies set `is_builtin_only() == true`
/// and full proxies set `is_builtin_only() == false`.
#[tokio::test]
async fn test_builtin_only_proxy_flag_and_upgrade() {
    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "builtin-only-flag-test";

    insert_test_session(&manager, session_id).await;

    // 1. Lazy builtin proxy must have is_builtin_only == true
    let lazy_proxy = manager.ensure_builtin_proxy(session_id).await.unwrap();
    assert!(
        lazy_proxy.is_builtin_only(),
        "Lazy builtin proxy must be marked as is_builtin_only"
    );

    // 2. Fully configured proxy must have is_builtin_only == false
    let configured_proxy = manager
        .create_proxy(
            session_id.to_string(),
            vec!["playbook".to_string()],
            vec![],
            None,
        )
        .await
        .unwrap();
    assert!(
        !configured_proxy.is_builtin_only(),
        "Fully configured proxy must NOT be marked as is_builtin_only"
    );
}

/// Concurrent create_proxy for the same session must single-flight to one Arc.
#[tokio::test]
async fn test_concurrent_create_proxy_same_session_single_flight() {
    let harness = create_test_harness().await;
    let manager = Arc::clone(&harness.manager);
    let session_id = "concurrent-create-same-session";

    insert_test_session(&manager, session_id).await;

    let manager_a = Arc::clone(&manager);
    let manager_b = Arc::clone(&manager);
    let session_a = session_id.to_string();
    let session_b = session_id.to_string();

    let (result_a, result_b) = tokio::join!(
        manager_a.create_proxy(session_a, vec!["playbook".to_string()], vec![], None),
        manager_b.create_proxy(session_b, vec!["playbook".to_string()], vec![], None),
    );

    let proxy_a = result_a.expect("first create_proxy must succeed");
    let proxy_b = result_b.expect("second create_proxy must succeed");
    assert!(
        Arc::ptr_eq(&proxy_a, &proxy_b),
        "concurrent create_proxy must return the same proxy Arc"
    );
    assert!(
        Arc::ptr_eq(
            &proxy_a,
            &manager.get_proxy(session_id).await.expect("proxy present")
        ),
        "map must hold the single published proxy"
    );
}

/// ensure_builtin_proxy and create_proxy on the same session must serialize;
/// final proxy must be the full (non-builtin-only) create path.
#[tokio::test]
async fn test_ensure_builtin_and_create_proxy_serialize() {
    let harness = create_test_harness().await;
    let manager = Arc::clone(&harness.manager);
    let session_id = "ensure-builtin-create-serialize";

    insert_test_session(&manager, session_id).await;

    let manager_a = Arc::clone(&manager);
    let manager_b = Arc::clone(&manager);
    let session_owned = session_id.to_string();

    let (ensure_result, create_result) = tokio::join!(
        manager_a.ensure_builtin_proxy(session_id),
        manager_b.create_proxy(session_owned, vec!["playbook".to_string()], vec![], None),
    );

    let ensure_proxy = ensure_result.expect("ensure_builtin_proxy must succeed");
    let create_proxy = create_result.expect("create_proxy must succeed");
    let final_proxy = manager
        .get_proxy(session_id)
        .await
        .expect("proxy must exist after both settle");

    assert!(
        !final_proxy.is_builtin_only(),
        "final proxy must match full create_proxy upgrade path"
    );
    assert!(
        Arc::ptr_eq(&final_proxy, &create_proxy),
        "map proxy must be the create_proxy result after serialization"
    );
    // ensure may have observed the pre-upgrade lazy proxy or the final one.
    assert!(
        Arc::ptr_eq(&ensure_proxy, &final_proxy) || ensure_proxy.is_builtin_only(),
        "ensure_builtin must return either the final proxy or a prior builtin-only Arc"
    );
}

/// Overlapping create_proxy and destroy_proxy must settle without panic or dangling managers.
#[tokio::test]
async fn test_create_proxy_overlapping_destroy_proxy() {
    let harness = create_test_harness().await;
    let manager = Arc::clone(&harness.manager);
    let session_id = "create-destroy-overlap";

    insert_test_session(&manager, session_id).await;

    // Warm path: establish a proxy first so destroy has work, then overlap recreate+destroy.
    manager
        .create_proxy(
            session_id.to_string(),
            vec!["playbook".to_string()],
            vec![],
            None,
        )
        .await
        .expect("initial create must succeed");

    let manager_create = Arc::clone(&manager);
    let manager_destroy = Arc::clone(&manager);
    let session_owned = session_id.to_string();

    let (create_result, ()) = tokio::join!(
        manager_create.create_proxy(session_owned, vec!["playbook".to_string()], vec![], None),
        manager_destroy.destroy_proxy(session_id),
    );

    // create may Ok (ran after destroy) or Ok (ran before / reused then destroyed).
    let _ = create_result;

    let proxy = manager.get_proxy(session_id).await;
    let has_stdio = manager.has_stdio_manager_for_test(session_id).await;
    let has_http = manager.has_http_manager_for_test(session_id).await;
    let has_guard = manager.has_creation_guard_for_test(session_id).await;

    match proxy {
        None => {
            assert!(
                !has_stdio && !has_http,
                "destroyed session must not leave dangling managers"
            );
            assert!(
                !has_guard,
                "destroy must remove creation guard when no proxy remains"
            );
        }
        Some(p) => {
            // Only valid if create started after destroy finished and re-published.
            assert!(
                has_stdio && has_http,
                "live proxy must have matching managers"
            );
            assert!(
                !p.is_builtin_only(),
                "post-destroy recreate must be a full proxy"
            );
            assert!(
                has_guard,
                "active session after recreate may retain creation guard"
            );
        }
    }

    // Second destroy must clean any surviving create-after-destroy proxy.
    manager.destroy_proxy(session_id).await;
    assert!(manager.get_proxy(session_id).await.is_none());
    assert!(!manager.has_stdio_manager_for_test(session_id).await);
    assert!(!manager.has_http_manager_for_test(session_id).await);
    assert!(!manager.has_creation_guard_for_test(session_id).await);
}

#[test]
fn test_promote_builtin_reuse_runtime_state() {
    use crate::agent::runtime_state::SessionRuntimeState;
    use super::creation::promote_builtin_reuse_runtime_state;

    let not_ready = SessionRuntimeState::default();
    assert!(!not_ready.proxy.ready);
    let promoted = promote_builtin_reuse_runtime_state(not_ready, false);
    assert!(promoted.proxy.ready);

    let ready = SessionRuntimeState::builtin_ready();
    let kept = promote_builtin_reuse_runtime_state(ready.clone(), false);
    assert_eq!(kept, ready);

    let external_unready = SessionRuntimeState::default();
    let unchanged = promote_builtin_reuse_runtime_state(external_unready.clone(), true);
    assert_eq!(unchanged, external_unready);
}

/// Reuse after ensure_builtin must promote a non-ready store back to ready so
/// open()/FE can leave Hydrating when the lazy path wrote ready without emit.
#[tokio::test]
async fn test_reuse_after_lazy_builtin_promotes_unready_runtime_state() {
    use crate::agent::runtime_state::SessionRuntimeState;

    let harness = create_test_harness().await;
    let manager = &harness.manager;
    let session_id = "reuse-promote-unready-runtime";

    insert_test_session(manager, session_id).await;

    let lazy_proxy = manager.ensure_builtin_proxy(session_id).await.unwrap();
    let tool_ids = lazy_proxy.builtin_tool_ids();
    assert!(
        manager.get_runtime_state(session_id).await.proxy.ready,
        "ensure_builtin_proxy must leave ready=true"
    );

    // Simulate lost/incomplete runtime snapshot while the proxy Arc remains.
    manager
        .set_runtime_state(session_id, SessionRuntimeState::default(), None)
        .await;
    assert!(!manager.get_runtime_state(session_id).await.proxy.ready);

    let reused = manager
        .create_proxy(session_id.to_string(), tool_ids, vec![], None)
        .await
        .unwrap();
    assert!(
        Arc::ptr_eq(&lazy_proxy, &reused),
        "matching builtin-only create_proxy must Reuse the lazy proxy"
    );
    assert!(
        manager.get_runtime_state(session_id).await.proxy.ready,
        "Reuse must promote builtin-only sessions to ready"
    );
}

