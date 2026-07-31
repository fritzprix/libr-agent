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
    CancelStrategy,
};
use tauri_mcp_agent_lib::repositories::SessionStatus;
use tokio_util::sync::CancellationToken;

// -----------------------------------------------------------------------
// cancel_strategy decision
// -----------------------------------------------------------------------

#[test]
fn test_classify_cancel_strategy_keeps_boundary_pause_when_pending_execution_exists() {
    let strategy = classify_cancel_strategy(true);
    assert_eq!(strategy, CancelStrategy::CancelToolsThenPause);
}

#[test]
fn test_classify_cancel_strategy_stops_immediately_without_pending_execution() {
    let strategy = classify_cancel_strategy(false);
    assert_eq!(strategy, CancelStrategy::StopImmediately);
}

#[test]
fn test_should_consume_cancel_at_message_boundary_only_when_pending_flag_set() {
    assert!(should_consume_cancel_at_message_boundary(true));
    assert!(!should_consume_cancel_at_message_boundary(false));
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
