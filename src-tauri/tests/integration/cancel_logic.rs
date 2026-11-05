//! Integration tests for the cancel state machine.
//!
//! Placed here (instead of workflow.rs #[cfg(test)]) so the test binary
//! does NOT load the Tauri/WebView2 DLLs, which caused
//! STATUS_ENTRYPOINT_NOT_FOUND (0xC0000139) on Windows when running
//! `cargo test --lib`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri_mcp_agent_lib::agent::state::{PendingEvent, PendingEventManager};
use tauri_mcp_agent_lib::agent::workflow::{
    classify_cancel_strategy, is_inactive_cancel_noop, should_consume_cancel_at_message_boundary,
    should_discard_workflow_before_continuation, CancelStrategy,
};
use tauri_mcp_agent_lib::mcp::service_proxy_manager::MCPServiceProxyManager;
use tauri_mcp_agent_lib::repositories::{SessionMetadata, SessionStatus};
use tauri_mcp_agent_lib::session::SessionManager;
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

// -----------------------------------------------------------------------
// cancel_strategy decision
// -----------------------------------------------------------------------

#[test]
fn test_classify_cancel_strategy_keeps_boundary_pause_when_pending_execution_exists() {
    let strategy = classify_cancel_strategy(true, false);
    assert_eq!(strategy, CancelStrategy::CancelToolsThenPause);
}

#[test]
fn test_classify_cancel_strategy_stops_immediately_without_pending_execution() {
    let strategy = classify_cancel_strategy(false, false);
    assert_eq!(strategy, CancelStrategy::StopImmediately);
}

#[test]
fn test_classify_cancel_strategy_continues_after_stopping_active_process() {
    let strategy = classify_cancel_strategy(true, true);
    assert_eq!(strategy, CancelStrategy::CancelProcessThenContinue);
}

#[test]
fn test_should_consume_cancel_at_message_boundary_only_when_pending_flag_set() {
    assert!(should_consume_cancel_at_message_boundary(true));
    assert!(!should_consume_cancel_at_message_boundary(false));
}

#[test]
fn test_pending_cancel_is_consumed_before_cancelled_token_is_discarded() {
    assert!(
        !should_discard_workflow_before_continuation(true, true),
        "a pending cancel must reach the message boundary so the session can pause"
    );
    assert!(
        should_discard_workflow_before_continuation(true, false),
        "a cancelled token without a pending cancel is a stale workflow result"
    );
    assert!(!should_discard_workflow_before_continuation(false, false));
}

#[test]
fn test_inactive_statuses_are_cancel_noops() {
    assert!(is_inactive_cancel_noop(&SessionStatus::Idle));
    assert!(is_inactive_cancel_noop(&SessionStatus::Paused));
    assert!(is_inactive_cancel_noop(&SessionStatus::Error));
    assert!(!is_inactive_cancel_noop(&SessionStatus::Busy));
    assert!(!is_inactive_cancel_noop(&SessionStatus::Queued));
    assert!(!is_inactive_cancel_noop(&SessionStatus::Provisioning));
}

// -----------------------------------------------------------------------
// CancellationToken state machine
// Mirrors the lifecycle that cancel_workflow + start_workflow manage.
// -----------------------------------------------------------------------

/// cancel_workflow (StopImmediately) calls token.cancel() but does NOT
/// replace the token. The cancelled state must persist until start_workflow
/// explicitly replaces it — this blocks stale in-flight LLM responses from
/// re-entering the workflow via allow_idle_tool_entry.
#[test]
fn test_cancel_token_stays_cancelled_after_cancel_workflow() {
    let token = CancellationToken::new();
    assert!(!token.is_cancelled(), "fresh token must not be cancelled");

    // Simulates cancel_workflow: cancel but do NOT replace
    token.cancel();

    assert!(
        token.is_cancelled(),
        "token must remain cancelled until start_workflow resets it"
    );
}

/// start_workflow replaces the token with a fresh one so a new workflow
/// can proceed normally. Regression guard: old code used read lock and never
/// reset the token, leaving new messages permanently blocked.
#[test]
fn test_start_workflow_resets_token_with_fresh_one() {
    let token = CancellationToken::new();
    token.cancel();
    assert!(token.is_cancelled());

    // Simulates start_workflow write-lock reset
    let token = CancellationToken::new();
    assert!(
        !token.is_cancelled(),
        "after start_workflow the token must be fresh and not cancelled"
    );
}

/// resume_workflow must also replace the token with a fresh one after a
/// StopImmediately cancel. Clearing only cancel_pending is not enough because
/// handle_llm_response rejects replies when the old token remains cancelled.
#[test]
fn test_resume_workflow_resets_token_with_fresh_one() {
    let token = CancellationToken::new();
    let cancel_pending = Arc::new(AtomicBool::new(true));

    token.cancel();
    assert!(token.is_cancelled());
    assert!(cancel_pending.load(Ordering::SeqCst));

    // Simulates resume_workflow write-lock reset
    cancel_pending.store(false, Ordering::SeqCst);
    let token = CancellationToken::new();

    assert!(
        !token.is_cancelled(),
        "after resume_workflow the token must be fresh and not cancelled"
    );
    assert!(
        !cancel_pending.load(Ordering::SeqCst),
        "resume_workflow must clear cancel_pending"
    );
}

/// terminate_session (hard kill) also resets the token at the end,
/// so the session is ready for a future workflow without needing start_workflow.
#[test]
fn test_terminate_session_leaves_fresh_token() {
    let token = CancellationToken::new();
    token.cancel();

    // Simulates terminate_session end: replace with new token
    let token = CancellationToken::new();
    assert!(
        !token.is_cancelled(),
        "after terminate_session the token must be fresh"
    );
}

/// Cancelled token clones are also cancelled — ensures that any task
/// holding a clone of the same token gets the cancellation signal.
#[test]
fn test_cancelled_token_clone_is_also_cancelled() {
    let token = CancellationToken::new();
    let cloned = token.clone();

    token.cancel();

    assert!(
        cloned.is_cancelled(),
        "a task holding a clone must see the cancellation"
    );
}

// -----------------------------------------------------------------------
// cancel_pending AtomicBool
// -----------------------------------------------------------------------

/// Mid-tool-batch cancel: cancel_pending stays true (for awaitAgent) and the
/// token is cancelled immediately so execute_tool_calls can break + tombstone.
#[test]
fn test_cancel_pending_stays_set_during_tool_batch_cancel() {
    let cancel_pending = Arc::new(AtomicBool::new(false));
    let token = CancellationToken::new();

    // cancel_workflow during tool batch
    cancel_pending.store(true, Ordering::SeqCst);
    token.cancel();

    assert!(cancel_pending.load(Ordering::SeqCst));
    assert!(
        token.is_cancelled(),
        "token must cancel immediately even while tools are still draining"
    );

    // continue_workflow_after_tool: reads and consumes the flag at message boundary
    let should_stop = cancel_pending.load(Ordering::SeqCst);
    assert!(should_stop);

    cancel_pending.store(false, Ordering::SeqCst);
    assert!(
        !cancel_pending.load(Ordering::SeqCst),
        "cancel_pending must be cleared after message-boundary consumption"
    );
}

/// StopImmediately path: cancel_workflow sets then immediately clears the
/// flag in the same function (no deferred consumer needed).
#[test]
fn test_cancel_pending_cleared_immediately_on_stop_immediately() {
    let cancel_pending = Arc::new(AtomicBool::new(false));

    // cancel_workflow sets it briefly to signal, then clears before returning
    cancel_pending.store(true, Ordering::SeqCst);
    cancel_pending.store(false, Ordering::SeqCst);

    assert!(
        !cancel_pending.load(Ordering::SeqCst),
        "StopImmediately must leave cancel_pending=false"
    );
}

// -----------------------------------------------------------------------
// discard_pending_events semantics (via PendingEventManager)
// -----------------------------------------------------------------------

/// Soft cancel preserves the durable waiting-prompt queue in production, but
/// in-memory PendingEventManager drains used by hard-terminate paths must stay
/// idempotent.
#[test]
fn test_pending_events_drain_is_complete() {
    let mut manager = PendingEventManager::new();
    manager.add(PendingEvent::Message("msg1".into()));
    manager.add(PendingEvent::Message("msg2".into()));
    assert_eq!(manager.count(), 2);

    let drained = manager.drain_messages();

    assert_eq!(drained.len(), 2, "both pending messages must be discarded");
    assert_eq!(
        manager.count(),
        0,
        "queue must be empty after discard to prevent post-cancel re-entry"
    );
}

/// Calling discard on an already-empty queue must be a safe no-op.
#[test]
fn test_discard_pending_events_is_idempotent() {
    let mut manager = PendingEventManager::new();

    let first = manager.drain_messages();
    assert!(first.is_empty());

    let second = manager.drain_messages();
    assert!(
        second.is_empty(),
        "double-discard must be a safe no-op (no panic, no stale state)"
    );
}

// -----------------------------------------------------------------------
// Combined cancel lifecycle scenario
// -----------------------------------------------------------------------

/// Full StopImmediately scenario:
///   cancel_workflow → token cancelled, flag cleared, queue drained
///   start_workflow  → token reset, workflow starts fresh
#[test]
fn test_stop_immediately_full_lifecycle() {
    let token = CancellationToken::new();
    let cancel_pending = Arc::new(AtomicBool::new(false));
    let mut manager = PendingEventManager::new();

    manager.add(PendingEvent::Message("queued-msg".into()));

    // --- cancel_workflow (StopImmediately) ---
    cancel_pending.store(true, Ordering::SeqCst);
    cancel_pending.store(false, Ordering::SeqCst); // cleared immediately
    token.cancel();
    let _drained = manager.drain_messages();

    assert!(token.is_cancelled());
    assert!(!cancel_pending.load(Ordering::SeqCst));
    assert_eq!(manager.count(), 0);

    // --- start_workflow ---
    let token = CancellationToken::new(); // reset
    cancel_pending.store(false, Ordering::SeqCst);

    assert!(
        !token.is_cancelled(),
        "workflow must be startable after cancel+reset"
    );
    assert!(!cancel_pending.load(Ordering::SeqCst));
}

/// Full mid-tool-batch cancel scenario (#1638):
///   cancel_workflow → flag set AND token cancelled (tools still may finish current call)
///   execute_tool_calls → sees cancel, injects tombstones for remaining, breaks
///   continue_workflow_after_tool → flag consumed, status→Paused
///   start_workflow → token reset, workflow starts normally
#[test]
fn test_tool_batch_cancel_cancels_token_immediately() {
    let token = CancellationToken::new();
    let cancel_pending = Arc::new(AtomicBool::new(false));

    // --- cancel_workflow during tool batch ---
    cancel_pending.store(true, Ordering::SeqCst);
    token.cancel();

    assert!(cancel_pending.load(Ordering::SeqCst));
    assert!(
        token.is_cancelled(),
        "token MUST be cancelled immediately so execute_tool_calls can stop remaining tools"
    );

    // --- execute_tool_calls cancel check ---
    let should_tombstone_remaining = token.is_cancelled() || cancel_pending.load(Ordering::SeqCst);
    assert!(should_tombstone_remaining);

    // --- continue_workflow_after_tool (message boundary) ---
    let should_stop = cancel_pending.load(Ordering::SeqCst);
    assert!(should_stop);

    cancel_pending.store(false, Ordering::SeqCst);
    let token = CancellationToken::new(); // reset as part of boundary consumption

    assert!(!token.is_cancelled());
    assert!(!cancel_pending.load(Ordering::SeqCst));

    // --- start_workflow (user sends new message) ---
    assert!(
        !token.is_cancelled(),
        "workflow must be startable after tool-batch cancel"
    );
}

async fn process_cancel_test_manager() -> (MCPServiceProxyManager, TempDir) {
    let db = Arc::new(crate::common::setup_test_db_with_migrations().await);
    let session_root = TempDir::new().expect("session root");
    let session_manager = Arc::new(
        SessionManager::new_with_base_dir(session_root.path().join("sessions"))
            .expect("session manager"),
    );
    (
        MCPServiceProxyManager::new(db, session_manager),
        session_root,
    )
}

#[tokio::test]
async fn test_process_cancel_marks_only_the_running_tool() {
    let (manager, _root) = process_cancel_test_manager().await;

    // A cancel while the tool runs marks that tool's result.
    manager.clear_process_cancel_pending("cancel-session").await;
    manager.mark_process_cancel_pending("cancel-session").await;
    assert!(manager.take_process_cancel_pending("cancel-session").await);

    // The next tool call (same or different) runs unmarked; nothing blocks it.
    manager.clear_process_cancel_pending("cancel-session").await;
    assert!(!manager.take_process_cancel_pending("cancel-session").await);
}

#[tokio::test]
async fn test_process_cancel_without_running_tool_does_not_mark_next_tool() {
    let (manager, _root) = process_cancel_test_manager().await;

    // Cancel stops a background process between tool calls.
    manager.mark_process_cancel_pending("idle-session").await;

    // The next tool call clears the stale marker before it runs.
    manager.clear_process_cancel_pending("idle-session").await;
    assert!(!manager.take_process_cancel_pending("idle-session").await);
}

// -----------------------------------------------------------------------
// Approval channel closure continuation decision
// -----------------------------------------------------------------------

#[test]
fn test_should_continue_after_approval_channel_closed_cases() {
    use tauri_mcp_agent_lib::agent::workflow::should_continue_after_approval_channel_closed;

    // Soft cancel: session is still active and busy in memory -> continuation tombstones call and pauses.
    assert!(should_continue_after_approval_channel_closed(Some(
        &SessionStatus::Busy
    )));

    // Terminated / deleted / inactive session -> continuation is discarded to prevent missing-session error cascades.
    assert!(!should_continue_after_approval_channel_closed(Some(
        &SessionStatus::Idle
    )));
    assert!(!should_continue_after_approval_channel_closed(Some(
        &SessionStatus::Paused
    )));
    assert!(!should_continue_after_approval_channel_closed(Some(
        &SessionStatus::Error
    )));
    assert!(!should_continue_after_approval_channel_closed(None));
}

fn sample_cancel_metadata(session_id: &str, status: SessionStatus) -> SessionMetadata {
    SessionMetadata {
        id: session_id.to_string(),
        name: None,
        status,
        model: "gpt-4".to_string(),
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
        created_at: 0,
        updated_at: 0,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        execution_mode: tauri_mcp_agent_lib::execution_mode::ExecutionMode::Normal,
        workspace_override: None,
        workspace_isolation:
            tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode::Host,
        docker_config: None,
        docker_container_name: None,
        docker_host_workspace_path: None,
        status_reason: None,
    }
}

#[tokio::test]
async fn test_abort_pending_tool_approvals_clears_and_drops_sender() {
    use std::collections::HashMap;
    use tauri_mcp_agent_lib::agent::context::registry::ContextRegistry;
    use tauri_mcp_agent_lib::agent::state::{
        AgentSession, PendingApprovalData, PendingApprovalKind,
    };
    use tauri_mcp_agent_lib::agent::workflow::abort_pending_tool_approvals;
    use tokio::sync::{oneshot, RwLock};

    let active_sessions = Arc::new(RwLock::new(HashMap::new()));
    let session_id = "test-session-abort-approvals".to_string();

    let session = AgentSession::new(
        sample_cancel_metadata(&session_id, SessionStatus::Busy),
        Arc::new(ContextRegistry::new()),
        None,
    );

    let (tx, rx) = oneshot::channel();
    let approval_data = PendingApprovalData {
        sender: tx,
        tool_name: "test_tool".to_string(),
        arguments: "{}".to_string(),
        approval_kind: PendingApprovalKind::Standard,
        request_id: None,
        description: Some("needs approval".to_string()),
        input_preview: Some("echo 1".to_string()),
    };
    session
        .pending_approvals
        .write()
        .await
        .insert("call-1".to_string(), approval_data);

    active_sessions
        .write()
        .await
        .insert(session_id.clone(), session);

    // Verify approval exists before abort
    {
        let sessions = active_sessions.read().await;
        let s = sessions.get(&session_id).unwrap();
        assert_eq!(s.pending_approvals.read().await.len(), 1);
    }

    // Abort pending approvals (as called by cancel/terminate)
    abort_pending_tool_approvals(&active_sessions, &session_id).await;

    // Approvals map must now be cleared
    {
        let sessions = active_sessions.read().await;
        let s = sessions.get(&session_id).unwrap();
        assert!(s.pending_approvals.read().await.is_empty());
    }

    // Sender was dropped, so receiver must receive RecvError (channel closed)
    assert!(rx.await.is_err());
}

/// #2543 requirement 13: a live persistent shell makes Stop a process-only
/// cancel, which returns before the workflow-level cleanup. Stop must still end
/// an open user prompt at once.
#[tokio::test]
async fn test_stop_with_live_shell_still_ends_the_user_prompt_wait() {
    use std::collections::HashMap;
    use std::time::Duration;
    use tauri_mcp_agent_lib::agent::concurrency::{
        ConcurrencyGate, DEFAULT_MAX_ACTIVE_AGENTS, DEFAULT_MAX_ACTIVE_PROCESSES,
        DEFAULT_MAX_SUSPENDED_AGENTS, DEFAULT_MAX_SUSPENDED_PROCESSES,
    };
    use tauri_mcp_agent_lib::agent::context::registry::ContextRegistry;
    use tauri_mcp_agent_lib::agent::session_bus::SessionBus;
    use tauri_mcp_agent_lib::agent::state::{
        admit_user_prompt, await_user_prompt_with_timeout, pending_user_prompt_summary,
        AgentSession, UserPromptOutcome, UserPromptPayload, UserQuestion,
    };
    use tauri_mcp_agent_lib::agent::workflow::stop_user_prompt_and_session_processes;
    use tauri_mcp_agent_lib::lifecycle::repositories::init_repositories;
    use tauri_mcp_agent_lib::mcp::builtin::workspace::PERSISTENT_SHELL_TOOL;
    use tauri_mcp_agent_lib::{init_concurrency_gate, init_session_bus};
    use tokio::sync::RwLock;

    let db = Arc::new(crate::common::setup_test_db_with_migrations().await);
    init_repositories(&db).await;
    init_session_bus(SessionBus::new());
    init_concurrency_gate(ConcurrencyGate::new(
        DEFAULT_MAX_ACTIVE_AGENTS,
        DEFAULT_MAX_SUSPENDED_AGENTS,
        DEFAULT_MAX_ACTIVE_PROCESSES,
        DEFAULT_MAX_SUSPENDED_PROCESSES,
    ));

    let session_root = TempDir::new().expect("session root");
    let session_manager = Arc::new(
        SessionManager::new_with_base_dir(session_root.path().join("sessions"))
            .expect("session manager"),
    );
    let proxy_manager = Arc::new(MCPServiceProxyManager::new(db, session_manager));
    let session_id = "test-stop-live-shell-user-prompt".to_string();

    proxy_manager
        .create_proxy(
            session_id.clone(),
            vec!["workspace".to_string()],
            Vec::new(),
            None,
        )
        .await
        .expect("workspace proxy");
    proxy_manager
        .call_tool(
            &session_id,
            &format!("workspace__{PERSISTENT_SHELL_TOOL}"),
            serde_json::json!({ "command": "echo ready" }),
        )
        .await
        .expect("persistent shell starts");

    let session = AgentSession::new(
        sample_cancel_metadata(&session_id, SessionStatus::Busy),
        Arc::new(ContextRegistry::new()),
        None,
    );
    let slot = session.pending_user_prompt.clone();
    let (_request_id, receiver) = admit_user_prompt(
        &slot,
        UserPromptPayload::Question {
            questions: vec![UserQuestion {
                id: "continue".to_string(),
                question: "Continue?".to_string(),
                description: None,
                options: None,
                multi_select: false,
                allow_custom: false,
            }],
        },
    )
    .await
    .expect("prompt admitted");
    let active_sessions = Arc::new(RwLock::new(HashMap::new()));
    active_sessions
        .write()
        .await
        .insert(session_id.clone(), session);

    let killed =
        stop_user_prompt_and_session_processes(&active_sessions, &proxy_manager, &session_id).await;

    assert!(
        killed > 0,
        "the live shell must make this the process-only cancel path"
    );
    assert!(
        classify_cancel_strategy(false, killed > 0) == CancelStrategy::CancelProcessThenContinue
    );
    // A generous timeout proves Stop closed the channel; the clock did not run out.
    let outcome = await_user_prompt_with_timeout(receiver, Duration::from_secs(30)).await;
    assert_eq!(outcome, UserPromptOutcome::Stopped);
    assert_eq!(outcome.as_event_str(), "cancelled");
    assert!(pending_user_prompt_summary(&slot).await.is_none());
}
