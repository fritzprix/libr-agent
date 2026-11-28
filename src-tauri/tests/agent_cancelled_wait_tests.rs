//! #3086 G1.2/G1.3: handler-level coverage for `agent__checkSession(wait=true)`
//! abnormal wait ends and the caller's concurrency-permit invariant.
//!
//! Root-level test binary so it runs on Windows CI (`integration_tests` is
//! `#![cfg(not(windows))]` because it links WebView).
//!
//! Covered invariants:
//! - G1.1: when the wait ends abnormally and the gate is full, the tool result
//!   is still delivered and the caller's active permit is *missing* (the LLM
//!   loop re-acquires it via `ensure_active_permit_for_llm` before the next
//!   request). The restore failure must never be swallowed silently.
//! - G1.3: a child that is `paused` when `wait=true` must be reported as the
//!   same cancelled failure the post-wait path reports — never as success.
//! - S1.1: the cancelled-wait result says "cancelled" with its own copy, not
//!   the paused builder's "will not make progress" text.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::{Duration, SystemTime};

use sea_orm::{ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement};
use sea_orm_migration::MigratorTrait;
use serde_json::json;
use tauri::AppHandle;
use tauri_mcp_agent_lib::agent::concurrency::ConcurrencyGate;
use tauri_mcp_agent_lib::agent::context::registry::ContextRegistry;
use tauri_mcp_agent_lib::agent::session_bus::SessionBus;
use tauri_mcp_agent_lib::agent::state::{
    AgentSession, CompactionPhase, CompactionRuntimeState, PendingEventManager,
};
use tauri_mcp_agent_lib::agent::{AgentRepositories, AgentSessionManager};
use tauri_mcp_agent_lib::execution_mode::ExecutionMode;
use tauri_mcp_agent_lib::mcp::builtin::agent::handlers::build_cancelled_wait_check_session_result_from_messages;
use tauri_mcp_agent_lib::mcp::builtin::agent::{handlers, AgentServer};
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::migration::Migrator;
use tauri_mcp_agent_lib::models::chat::Message;
use tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode;
use tauri_mcp_agent_lib::repositories::{
    MessageRepository, SessionMetadata, SessionRepository, SessionStatus, SqliteMessageRepository,
    SqliteSessionRepository,
};
use tauri_mcp_agent_lib::{
    get_active_sessions, get_concurrency_gate, get_message_repository, get_session_bus,
    init_active_sessions, init_concurrency_gate, init_session_bus, set_database_connection,
    set_message_repository, set_session_repository,
};
use tokio::sync::{oneshot, RwLock};
use tokio_util::sync::CancellationToken;

// ─── Shared runtime / globals ────────────────────────────────────────────────
//
// SeaORM's SQLite pool is runtime-affine, and the gate/bus/active-map globals
// are OnceLock singletons, so the whole binary shares exactly one tokio
// runtime and one set of global primitives. Tests serialize on TEST_GUARD
// because they share the global concurrency gate's slots.

static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
static TEST_GUARD: StdMutex<()> = StdMutex::new(());
static GLOBALS_ONCE: OnceLock<()> = OnceLock::new();
static DB: tokio::sync::OnceCell<DatabaseConnection> = tokio::sync::OnceCell::const_new();
static SESSION_REPO: tokio::sync::OnceCell<Arc<SqliteSessionRepository>> =
    tokio::sync::OnceCell::const_new();

fn rt() -> &'static tokio::runtime::Runtime {
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("shared test runtime")
    })
}

/// Active=2 lets a test hold one slot as a "third party" while the caller
/// suspends/resumes through the other; the filler task takes the freed slot
/// to make `SuspendedAgentGuard::try_resume()` fail (G1.1).
const MAX_ACTIVE_AGENTS: u32 = 2;

/// Serialize on TEST_GUARD and run `f` on the shared runtime.
fn run_test<F>(f: F)
where
    F: std::future::Future<Output = ()>,
{
    let guard = TEST_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    rt().block_on(async move {
        init_globals().await;
        f.await;
        drop(guard);
    });
}

async fn setup_isolated_db() -> DatabaseConnection {
    tauri_mcp_agent_lib::lifecycle::database::register_sqlite_vec();
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite should connect");
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "PRAGMA foreign_keys = ON".to_string(),
    ))
    .await
    .expect("foreign_keys pragma should apply");
    Migrator::up(&db, None)
        .await
        .expect("migrations should run");
    db
}

/// One-time global init: DB + repositories + bus + gate + active-session map.
/// Repeated calls are no-ops (OnceLock semantics of the underlying state).
async fn init_globals() {
    if GLOBALS_ONCE.get().is_some() {
        return;
    }
    let db = DB.get_or_init(setup_isolated_db).await.clone();
    // Warm the shared session repo cell (used by tests for status updates).
    SESSION_REPO
        .get_or_init(|| async { Arc::new(SqliteSessionRepository::new(db.clone())) })
        .await;
    let message_repo = SqliteMessageRepository::new(db.clone());

    set_session_repository(SqliteSessionRepository::new(db.clone()));
    set_database_connection(db);
    set_message_repository(message_repo);

    init_session_bus(SessionBus::new());
    init_concurrency_gate(ConcurrencyGate::new(MAX_ACTIVE_AGENTS, 4, 10, 20));
    init_active_sessions(Arc::new(RwLock::new(HashMap::new())));
}

fn session_repo() -> Arc<SqliteSessionRepository> {
    SESSION_REPO.get().expect("globals initialized").clone()
}

// ─── Fixtures ────────────────────────────────────────────────────────────────

fn session_meta(id: &str, status: SessionStatus, parent: Option<&str>) -> SessionMetadata {
    let now = chrono::Utc::now().timestamp_millis();
    SessionMetadata {
        id: id.to_string(),
        name: Some(format!("session-{id}")),
        status,
        model: "gpt-test".to_string(),
        provider: "openai".to_string(),
        assistant_id: None,
        parent_session_id: parent.map(str::to_string),
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
        status_reason: None,
        forked_from: None,
    }
}

fn assistant_message(session_id: &str, id: &str, text: &str) -> Message {
    Message {
        id: id.to_string(),
        session_id: session_id.to_string(),
        role: "assistant".to_string(),
        content: vec![MCPContent::Text {
            text: text.to_string(),
        }],
        tool_calls: None,
        tool_call_id: None,
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage: None,
        prompt_tokens: None,
        created_at: 1,
        updated_at: 1,
        source: None,
        error: None,
        metadata: None,
    }
}

fn runtime_agent_session(metadata: SessionMetadata) -> AgentSession {
    AgentSession {
        metadata,
        is_running: true,
        active_permit: None,
        status_transition: Arc::new(RwLock::new(None)),
        transition_lock: Arc::new(tokio::sync::Mutex::new(())),
        cancellation_token: CancellationToken::new(),
        cancel_pending: Arc::new(AtomicBool::new(false)),
        pending_execution: None,
        messages: Arc::new(RwLock::new(Vec::new())),
        cache_initialized: Arc::new(AtomicBool::new(true)),
        last_synced_at: Arc::new(RwLock::new(Some(SystemTime::now()))),
        repeated_thinking_retry_count: Arc::new(RwLock::new(0)),
        repeated_text_loop_retry_count: Arc::new(RwLock::new(0)),
        reasoning_budget_retry_count: Arc::new(RwLock::new(0)),
        malformed_tool_markup_retry_count: Arc::new(RwLock::new(0)),
        invalid_tool_args_retry_count: Arc::new(RwLock::new(0)),
        pending_events: Arc::new(RwLock::new(PendingEventManager::new())),
        pending_approvals: Arc::new(RwLock::new(HashMap::new())),
        pending_user_prompt: tauri_mcp_agent_lib::agent::state::new_user_prompt_slot(),
        context_registry: Arc::new(ContextRegistry::new()),
        compact_context: Arc::new(RwLock::new(None)),
        compaction: CompactionRuntimeState::with_test_state(CompactionPhase::Idle, None),
        expected_response_id: Arc::new(RwLock::new(None)),
        cached_stable_prompt: Arc::new(RwLock::new(None)),
        last_completion_request: Arc::new(RwLock::new(None)),
        last_submitted_input_message_id: Arc::new(RwLock::new(None)),
        last_session_context_snapshot: Arc::new(RwLock::new(None)),
        session_context_turns_since_force_fresh: Arc::new(RwLock::new(0)),
        tool_loop_resample_attempts: Arc::new(RwLock::new(HashMap::new())),
        tool_poll_trackers: Arc::new(RwLock::new(HashMap::new())),
        max_input_context_override: None,
    }
}

/// Fake owned (Wry-typed) `AppHandle` for `AgentSessionManager` in tests.
///
/// `AppHandle<Wry>` embeds `Wry::Handle` (channel senders), so a
/// `MockRuntime` handle cannot be reinterpreted field-by-field. The
/// `checkSession` wait path never reads or drops the handle (it only touches
/// the gate, active-session map, bus, and repositories), so a zero-filled
/// value is inert — the tests leak the harness so the fake is never dropped.
/// If a future wait path emits through the handle, these tests crash loudly.
fn fake_app_handle() -> AppHandle {
    unsafe {
        let layout = std::alloc::Layout::new::<AppHandle>();
        let ptr = std::alloc::alloc_zeroed(layout) as *mut AppHandle;
        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        ptr.read()
    }
}

struct Harness {
    server: AgentServer,
    _tempdir: tempfile::TempDir,
}

async fn harness(caller_id: &str, child_id: &str, child_status: SessionStatus) -> Harness {
    let db = DB.get().expect("globals initialized").clone();
    let repo = session_repo();

    repo.upsert_session(&session_meta(caller_id, SessionStatus::Idle, None))
        .await
        .expect("caller session upsert");
    repo.upsert_session(&session_meta(child_id, child_status, Some(caller_id)))
        .await
        .expect("child session upsert");

    let message_repo = get_message_repository();
    message_repo
        .insert(&assistant_message(
            child_id,
            &format!("{child_id}-a1"),
            "child output",
        ))
        .await
        .expect("child message insert");

    // Manager shares the global active-sessions map so `get_session_cancel_pending`
    // and the wait path observe the same sessions the test mutates.
    let active_sessions = get_active_sessions().clone();
    active_sessions.write().await.insert(
        caller_id.to_string(),
        runtime_agent_session(session_meta(caller_id, SessionStatus::Busy, None)),
    );

    let tempdir = tempfile::TempDir::new().expect("temp dir");
    let session_manager = Arc::new(
        tauri_mcp_agent_lib::session::SessionManager::new_with_base_dir(
            tempdir.path().join("session-root"),
        )
        .expect("session manager"),
    );
    let proxy_manager = Arc::new(tauri_mcp_agent_lib::mcp::MCPServiceProxyManager::new(
        Arc::new(db.clone()),
        session_manager,
    ));

    let manager = AgentSessionManager::new_with_repositories(
        fake_app_handle(),
        proxy_manager,
        AgentRepositories::new(repo),
        active_sessions.clone(),
    );

    let server = AgentServer::new(caller_id.to_string(), Arc::new(db), Some(manager))
        .await
        .expect("agent server");

    Harness {
        server,
        _tempdir: tempdir,
    }
}

async fn caller_permit(caller_id: &str) {
    let permit = get_concurrency_gate()
        .acquire_active_agent()
        .await
        .expect("caller active permit");
    get_active_sessions()
        .write()
        .await
        .get_mut(caller_id)
        .expect("caller in active map")
        .active_permit = Some(permit);
}

async fn caller_permit_held(caller_id: &str) -> bool {
    get_active_sessions()
        .read()
        .await
        .get(caller_id)
        .and_then(|session| session.active_permit.as_ref())
        .is_some()
}

async fn cleanup_active_sessions(caller_id: &str) {
    get_active_sessions().write().await.remove(caller_id);
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

// ─── Tests ───────────────────────────────────────────────────────────────────

/// G1.1 + G1.3/S1.1: child goes Paused while the caller waits and the gate is
/// full (a filler task took the caller's freed slot). The tool result must be
/// delivered as a cancelled failure, and the caller's active permit must be
/// left missing rather than blocked-on or silently dropped.
#[test]
fn cancelled_wait_with_full_gate_delivers_failure_without_permit() {
    run_test(async {
        let caller = "caller-g11";
        let child = "child-g11";
        let env = harness(caller, child, SessionStatus::Busy).await;
        caller_permit(caller).await;

        // Third party occupies one active slot for the whole wait.
        let third = get_concurrency_gate()
            .acquire_active_agent()
            .await
            .expect("third-party permit");

        // Filler takes the slot the caller frees on suspend, so the abnormal
        // end-of-wait `try_resume()` finds the gate full.
        let (release_tx, release_rx) = oneshot::channel::<()>();
        let filler = rt().spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let permit = get_concurrency_gate()
                .acquire_active_agent()
                .await
                .expect("filler permit");
            let _ = release_rx.await;
            drop(permit);
        });

        // Paused child + status wake ends the wait abnormally.
        let pause_task = rt().spawn(async move {
            tokio::time::sleep(Duration::from_millis(250)).await;
            session_repo()
                .update_status(child, SessionStatus::Paused, None)
                .await
                .expect("pause child");
            get_session_bus().notify_status_change(child);
        });

        let result = handlers::check_session(
            &env.server,
            json!({ "sessionId": child, "wait": true, "timeout": 30 }),
            caller,
        )
        .await
        .expect("wait must return a tool result even with a full gate");

        let structured = result
            .structured_content
            .as_ref()
            .expect("structured content");
        assert_eq!(
            result.is_error,
            Some(true),
            "child-cancelled wait must be an error result"
        );
        assert_eq!(
            structured.get("status").and_then(|v| v.as_str()),
            Some("cancelled"),
            "G1.3: paused child with wait=true reports as cancelled"
        );
        assert_eq!(
            structured
                .get("abnormalTermination")
                .and_then(|v| v.as_bool()),
            Some(true),
            "cancelled wait is an abnormal termination"
        );
        let text = extract_text(&result);
        assert!(
            text.contains("cancelled before finishing"),
            "S1.1: own cancelled copy, got: {text}"
        );
        assert!(
            !text.contains("is paused and will not make progress"),
            "S1.1: must not reuse the paused builder copy, got: {text}"
        );

        // G1.1: gate was full → permit is missing; the loop re-acquires it
        // before the next LLM request (ensure_active_permit_for_llm).
        assert!(
            !caller_permit_held(caller).await,
            "permit must be absent (not silently restored) when the gate is full"
        );

        drop(release_tx);
        filler.await.expect("filler join");
        pause_task.await.expect("pause join");
        drop(third);
        cleanup_active_sessions(caller).await;
        // Never drop: the manager owns the zero-filled fake AppHandle.
        std::mem::forget(env);
    });
}

/// Caller-side cancel while waiting: the tool returns Err("...interrupted...")
/// and the caller's permit is restored (a slot was free via try_resume).
#[test]
fn caller_cancelled_wait_returns_interrupted_and_restores_permit() {
    run_test(async {
        let caller = "caller-g11b";
        let child = "child-g11b";
        let env = harness(caller, child, SessionStatus::Busy).await;
        caller_permit(caller).await;

        let (release_tx, release_rx) = oneshot::channel::<()>();
        let filler = rt().spawn(async move {
            let permit = get_concurrency_gate()
                .acquire_active_agent()
                .await
                .expect("filler permit");
            let _ = release_rx.await;
            drop(permit);
        });

        // Cancel the caller while it waits.
        let cancel_task = rt().spawn(async move {
            tokio::time::sleep(Duration::from_millis(150)).await;
            let sessions = get_active_sessions();
            let flag = {
                let active = sessions.read().await;
                active
                    .get("caller-g11b")
                    .expect("caller in active map")
                    .cancel_pending
                    .clone()
            };
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
            get_session_bus().notify_status_change("caller-g11b");
        });

        let err = handlers::check_session(
            &env.server,
            json!({ "sessionId": child, "wait": true, "timeout": 30 }),
            caller,
        )
        .await
        .expect_err("cancelled caller must surface an error, not a paused/success result");
        assert!(
            err.contains("interrupted"),
            "expected interrupted error, got: {err}"
        );

        // A slot was free → abnormal path restored the permit via try_resume.
        assert!(
            caller_permit_held(caller).await,
            "permit must be restored when a slot is free on abnormal wait end"
        );

        drop(release_tx);
        filler.await.expect("filler join");
        cancel_task.await.expect("cancel join");
        cleanup_active_sessions(caller).await;
        // Never drop: the manager owns the zero-filled fake AppHandle.
        std::mem::forget(env);
    });
}

/// Real timeout (5s floor) with the gate full: a non-error timeout result is
/// delivered and the caller's permit is left missing.
#[test]
fn timeout_wait_with_full_gate_delivers_timeout_without_permit() {
    run_test(async {
        let caller = "caller-g11c";
        let child = "child-g11c";
        // Running child never settles → wait times out.
        let env = harness(caller, child, SessionStatus::Busy).await;
        caller_permit(caller).await;

        let third = get_concurrency_gate()
            .acquire_active_agent()
            .await
            .expect("third-party permit");

        let (release_tx, release_rx) = oneshot::channel::<()>();
        let filler = rt().spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let permit = get_concurrency_gate()
                .acquire_active_agent()
                .await
                .expect("filler permit");
            let _ = release_rx.await;
            drop(permit);
        });

        let result = handlers::check_session(
            &env.server,
            json!({ "sessionId": child, "wait": true, "timeout": 5 }),
            caller,
        )
        .await
        .expect("timeout must still deliver a tool result with a full gate");

        let structured = result
            .structured_content
            .as_ref()
            .expect("structured content");
        assert_eq!(
            structured.get("timeout").and_then(|v| v.as_bool()),
            Some(true),
            "timeout result expected"
        );
        assert_eq!(
            result.is_error,
            Some(false),
            "timeout is a normal (non-error) result"
        );
        assert!(
            !caller_permit_held(caller).await,
            "permit must be absent after timeout wait with a full gate"
        );

        drop(release_tx);
        filler.await.expect("filler join");
        drop(third);
        cleanup_active_sessions(caller).await;
        // Never drop: the manager owns the zero-filled fake AppHandle.
        std::mem::forget(env);
    });
}

/// G1.3: pre-paused child — `wait=true` must fail like the post-wait path,
/// `wait=false` keeps the paused-success behavior.
#[test]
fn pre_paused_child_wait_true_reports_cancelled_wait_false_reports_paused() {
    run_test(async {
        let caller = "caller-g13";
        let child = "child-g13";
        let env = harness(caller, child, SessionStatus::Paused).await;
        caller_permit(caller).await;

        let failed = handlers::check_session(
            &env.server,
            json!({ "sessionId": child, "wait": true, "timeout": 30 }),
            caller,
        )
        .await
        .expect("wait=true on paused child must return a result");
        assert_eq!(
            failed.is_error,
            Some(true),
            "G1.3: wait=true must not report a paused child as success"
        );
        assert_eq!(
            failed
                .structured_content
                .as_ref()
                .and_then(|s| s.get("status"))
                .and_then(|v| v.as_str()),
            Some("cancelled")
        );

        let paused = handlers::check_session(
            &env.server,
            json!({ "sessionId": child, "wait": false }),
            caller,
        )
        .await
        .expect("wait=false on paused child must return a result");
        assert_ne!(
            paused.is_error,
            Some(true),
            "wait=false keeps the paused-success semantics"
        );
        assert_eq!(
            paused
                .structured_content
                .as_ref()
                .and_then(|s| s.get("status"))
                .and_then(|v| v.as_str()),
            Some("paused")
        );
        assert!(
            extract_text(&paused).contains("is paused"),
            "wait=false keeps the paused copy"
        );

        cleanup_active_sessions(caller).await;
        // Never drop: the manager owns the zero-filled fake AppHandle.
        std::mem::forget(env);
    });
}

/// S1.1: builder-level contract for the cancelled-wait copy.
#[test]
fn cancelled_builder_uses_own_copy_and_error_fields() {
    let messages = vec![json!({ "id": "a1", "role": "assistant", "content": [] })];
    let result = build_cancelled_wait_check_session_result_from_messages(
        "child-builder",
        2,
        &messages,
        None,
    );

    assert_eq!(result.is_error, Some(true));
    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content");
    assert_eq!(
        structured.get("status").and_then(|v| v.as_str()),
        Some("cancelled")
    );
    assert_eq!(
        structured.get("responseStatus").and_then(|v| v.as_str()),
        Some("error")
    );
    assert_eq!(
        structured
            .get("abnormalTermination")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        structured.get("recoverable").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        structured.get("recoveryStrategy").and_then(|v| v.as_str()),
        Some("agent__messageToSession")
    );

    let text = extract_text(&result);
    assert!(
        text.contains("was cancelled before finishing the delegated task"),
        "got: {text}"
    );
    assert!(!text.contains("is paused"), "got: {text}");
}
