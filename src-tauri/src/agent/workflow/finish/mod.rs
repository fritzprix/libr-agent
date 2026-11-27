//! Centralized workflow settlement gate.
//!
//! All natural-completion and workflow-error finalization paths converge here so
//! terminal assistant messages are durable before parents or the UI observe
//! `Idle` / `Error`.
//!
//! See `docs/architecture/workflow-settlement.md` for the finish-window TOCTOU
//! scenario and why the success path performs two pending-event scans.

mod messages;
mod pending;
mod persist;
mod stop_hook;

pub use pending::{continue_workflow_if_pending_events, session_has_pending_events};
pub use persist::{persist_terminal_assistant_sync, settle_before_terminal_transition};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_subagent_matcher;

use crate::agent::events::{AgentEvent, AgentEventDispatcher, WorkflowCompletionReason};
use crate::agent::llm::types::{AgentRuntimeError, AgentRuntimeErrorType};
use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::{SessionRepository, SessionStatus};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

use messages::emit_settlement_agent_event;
use persist::{
    handle_terminal_persist_failure, transition_to_terminal_status_and_emit, TerminalTransition,
};
use stop_hook::{maybe_continue_from_stop_hook, subagent_matcher_value, StopHookTransition};

/// Starts one more LLM turn after settlement finds pending work or a stop hook
/// asks to continue.
///
/// The settlement path takes this as a parameter instead of calling the
/// completion entry point directly, so a test can own its restart stub. A
/// process-wide test hook would leak into release builds and race between
/// tests (#3028 G2.1/G2.2).
pub trait WorkflowRestart: Send + Sync {
    fn restart<'a>(
        &'a self,
        session_repo: &'a Arc<dyn SessionRepository>,
        active_sessions: &'a Arc<RwLock<HashMap<String, AgentSession>>>,
        proxy_manager: &'a Arc<MCPServiceProxyManager>,
        app_handle: &'a AppHandle,
        session_id: String,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>>;
}

/// The production restart: request another turn through the completion path.
pub struct LlmCompletionRestart;

impl WorkflowRestart for LlmCompletionRestart {
    fn restart<'a>(
        &'a self,
        session_repo: &'a Arc<dyn SessionRepository>,
        active_sessions: &'a Arc<RwLock<HashMap<String, AgentSession>>>,
        proxy_manager: &'a Arc<MCPServiceProxyManager>,
        app_handle: &'a AppHandle,
        session_id: String,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>> {
        Box::pin(async move {
            crate::agent::llm::request_llm_completion_with_recovery(
                session_repo,
                active_sessions,
                proxy_manager,
                app_handle,
                session_id,
            )
            .await
            .map_err(|error| error.to_string())
        })
    }
}

/// `WorkflowCompleted` for a turn a hook stopped (#3004 G3.1): the hook's
/// `stopReason` rides the event so the UI can show why the turn ended without
/// an error.
fn hook_stopped_completion_event(session_id: &str, stop_reason: &str) -> AgentEvent {
    AgentEvent::WorkflowCompleted {
        session_id: session_id.to_string(),
        reason: WorkflowCompletionReason::HookStopped,
        stop_reason: Some(stop_reason.to_string()),
    }
}

/// Shared tail of both hook-stop settlement paths (#3004 G4.2): the turn was
/// stopped by a `PreToolUse`/`PostToolUse` hook (`hook_stop_reason`) or by the
/// `Stop` hook (`StopHookTransition::StoppedByHook`). Reset the continuation
/// counter, settle to `Idle` with the reason, and surface it through
/// `WorkflowCompleted`.
async fn settle_hook_stopped_to_idle(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    stop_reason: &str,
) -> Result<(), String> {
    crate::services::hooks::reset_stop_hook_continuations(session_id);
    crate::agent::lifecycle::update_session_status_with_reason(
        session_repo,
        active_sessions,
        app_handle,
        session_id,
        SessionStatus::Idle,
        stop_reason,
    )
    .await?;
    emit_settlement_agent_event(
        dispatcher,
        session_id,
        hook_stopped_completion_event(session_id, stop_reason),
        "WorkflowCompleted",
    )
}

/// Surface the hook's `stopReason` when a queued prompt already restarted the
/// workflow (#3004 G1.2): uses `HookStoppedNotice` instead of `WorkflowCompleted`
/// so the UI displays the reason without flipping the continuing turn to `idle`.
/// Best-effort — the queued turn owns the session now, so a failed emit is
/// logged, not propagated.
fn emit_hook_stopped_notice(
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    stop_reason: &str,
) {
    if let Err(error) = dispatcher.emit_agent_event(AgentEvent::HookStoppedNotice {
        session_id: session_id.to_string(),
        stop_reason: stop_reason.to_string(),
    }) {
        log::error!(
            "Failed to emit HookStoppedNotice for session {}: {}",
            session_id,
            error
        );
    }
}

/// Shared Error terminal settlement for every path that leaves a session in
/// `Error` (#3007 / #3071 G1.1).
///
/// Order is fixed (#3071 G1.2): set `Error` → emit `WorkflowError` to the UI →
/// fire-and-forget `StopFailure`. Callers must not hand-roll status + emit.
pub async fn finalize_session_error_with_dispatcher(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    error: AgentRuntimeError,
) -> Result<(), String> {
    finalize_session_error_with_hook_sink(
        session_repo,
        active_sessions,
        dispatcher,
        session_id,
        error,
        spawn_stop_failure,
    )
    .await
}

pub(super) async fn finalize_session_error_with_hook_sink(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    error: AgentRuntimeError,
    emit_stop_failure: impl FnOnce(Arc<RwLock<HashMap<String, AgentSession>>>, String, &'static str),
) -> Result<(), String> {
    let error_type = error_type_label(&error.error_type);
    if let Err(status_error) = crate::agent::lifecycle::update_session_status_with_dispatcher(
        session_repo,
        active_sessions,
        dispatcher,
        session_id,
        SessionStatus::Error,
    )
    .await
    {
        // The Error event and StopFailure must still reach the UI and hooks:
        // every caller is a background path that discards this result, so
        // returning early here would hide the turn failure entirely
        // (#3071 G2.1).
        log::error!(
            "Session {} could not persist Error status before WorkflowError settlement: {}",
            session_id,
            status_error
        );
    }

    emit_settlement_agent_event(
        dispatcher,
        session_id,
        AgentEvent::WorkflowError {
            session_id: session_id.to_string(),
            error,
        },
        "WorkflowError",
    )?;

    // Do not await the hook: a slow or hanging StopFailure must not withhold
    // the error from the UI (#3071 G1.2).
    emit_stop_failure(
        Arc::clone(active_sessions),
        session_id.to_string(),
        error_type,
    );
    Ok(())
}

/// AppHandle wrapper for background paths that do not already hold a dispatcher.
pub async fn finalize_session_error(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    error: AgentRuntimeError,
) -> Result<(), String> {
    let dispatcher = crate::agent::tauri_events::TauriEventDispatcher::new(app_handle);
    finalize_session_error_with_dispatcher(
        session_repo,
        active_sessions,
        &dispatcher,
        session_id,
        error,
    )
    .await
}

/// Map a LibrAgent runtime error class to the stable snake_case label a
/// `StopFailure` matcher is compared against (#3007). Claude Code matches on
/// error type (`rate_limit`, …); these labels are LibrAgent's equivalents.
pub(crate) fn error_type_label(error: &AgentRuntimeErrorType) -> &'static str {
    match error {
        AgentRuntimeErrorType::RateLimitError => "rate_limit",
        AgentRuntimeErrorType::AuthenticationError => "authentication",
        AgentRuntimeErrorType::NetworkError => "network",
        AgentRuntimeErrorType::AiServiceError => "ai_service",
        AgentRuntimeErrorType::ContextLimitError => "context_limit",
        AgentRuntimeErrorType::McpError => "mcp",
        AgentRuntimeErrorType::ToolExecutionError => "tool_execution",
        AgentRuntimeErrorType::ValidationError => "validation",
        AgentRuntimeErrorType::MalformedFunctionCall => "malformed_function_call",
        AgentRuntimeErrorType::JsonParsingError => "json_parsing",
        AgentRuntimeErrorType::EmptySelectionError => "empty_selection",
    }
}

/// Fire-and-forget `StopFailure` for a session whose turn ended in error
/// (#3007 / #3071 G1.2). Child sessions carry the same agent identity
/// envelope fields as `SubagentStop`.
pub(super) fn spawn_stop_failure(
    active_sessions: Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: String,
    error_type: &'static str,
) {
    // Spawn on the caller's runtime, not tauri's global one: the detached
    // task reaches process-global stores (hook plugin dirs, session
    // workspace lookup). Pinning it to the caller's runtime keeps those
    // accesses on the runtime that owns them — the global runtime outliving
    // a caller-owned SQLite pool crashed `integration_tests` with SIGSEGV
    // (#3071 round 2 CI).
    tokio::spawn(async move {
        run_stop_failure(&active_sessions, &session_id, error_type).await;
    });
}

async fn run_stop_failure(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    error_type: &'static str,
) {
    let (parent_session_id, assistant_id, session_name) = {
        let active = active_sessions.read().await;
        active
            .get(session_id)
            .map(|s| {
                (
                    s.metadata.parent_session_id.clone(),
                    s.metadata.assistant_id.clone(),
                    s.metadata.name.clone(),
                )
            })
            .unwrap_or((None, None, None))
    };
    let mut params = crate::services::hooks::HookEnvelopeParams::new(
        session_id,
        std::path::PathBuf::from("."),
        crate::services::hooks::HookEvent::StopFailure,
    );
    params.error_type = Some(error_type.to_string());
    if parent_session_id.is_some() {
        params.child_session_id = Some(session_id.to_string());
        params.agent_id = Some(session_id.to_string());
        let agent_type = subagent_matcher_value(
            parent_session_id.as_deref(),
            assistant_id.as_deref(),
            session_name.as_deref(),
        )
        .await;
        if !agent_type.is_empty() {
            params.agent_type = Some(agent_type);
        }
    }
    let _ = crate::services::hooks::dispatch_hook_event(
        crate::services::hooks::HookEvent::StopFailure,
        error_type,
        &params,
        session_id,
    )
    .await;
}

/// Fire-and-forget `SubagentStart` for a child session (#3007 / #3071 G1.3).
/// No-op when `parent_session_id` is absent. Matcher value is the agent type.
pub fn spawn_subagent_start_with_dispatcher(
    session_id: &str,
    parent_session_id: &str,
    assistant_id: Option<&str>,
    session_name: Option<&str>,
    hook_dispatcher: Arc<dyn crate::services::hooks::execute::HookDispatcher>,
) {
    let child_session_id = session_id.to_string();
    let parent_id = parent_session_id.to_string();
    let assistant_id = assistant_id.map(str::to_string);
    let child_name = session_name.map(str::to_string);
    // Caller-runtime spawn for the same reason as `spawn_stop_failure`
    // (#3071 round 2 CI: detached hook work must not outlive the runtime
    // that owns the globals it reads).
    tokio::spawn(async move {
        let matcher_value = subagent_matcher_value(
            Some(&parent_id),
            assistant_id.as_deref(),
            child_name.as_deref(),
        )
        .await;
        let mut params = crate::services::hooks::HookEnvelopeParams::new(
            &child_session_id,
            std::path::PathBuf::from("."),
            crate::services::hooks::HookEvent::SubagentStart,
        );
        params.child_session_id = Some(child_session_id.clone());
        params.agent_id = Some(child_session_id.clone());
        if !matcher_value.is_empty() {
            params.agent_type = Some(matcher_value.clone());
        }
        let _ = hook_dispatcher
            .dispatch(
                crate::services::hooks::HookEvent::SubagentStart,
                &matcher_value,
                &params,
                &child_session_id,
            )
            .await;
    });
}

/// Finish a workflow only after terminal messages are durable and no pending work remains.
///
/// Returns `Ok(true)` when pending events restarted the workflow (caller must not treat
/// the session as terminal). Returns `Ok(false)` when the session settled to `Idle`.
///
/// `hook_stop_reason` is `Some` when a `PreToolUse`/`PostToolUse` hook stopped the turn
/// (`continue: false`, #3004 G3.2): settlement runs the same terminal persist and
/// pending-event scans as a normal finish so queued prompts keep draining, but the
/// `Stop` hook is skipped — a hook already stopped this turn — and the session settles
/// to `Idle` with the reason surfaced through `WorkflowCompleted.stop_reason` (#3004 G3.1).
/// When a queued prompt restarts the workflow instead, the reason is surfaced
/// *before* the restart is requested so the UI never sees a completion for the
/// continuing turn (#3004 G5.2).
#[allow(clippy::too_many_arguments)]
pub async fn settle_session_and_go_idle_with_dispatcher(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    restart: &dyn WorkflowRestart,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    terminal_message: Option<&Message>,
    completion_reason: WorkflowCompletionReason,
    hook_stop_reason: Option<&str>,
) -> Result<bool, String> {
    let workflow_restarted = match settle_before_terminal_transition(
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        restart,
        session_id,
        terminal_message,
    )
    .await
    {
        Ok(restarted) => restarted,
        Err(error) => {
            return handle_terminal_persist_failure(
                session_repo,
                active_sessions,
                dispatcher,
                session_id,
                error,
            )
            .await
            .map(|()| false);
        }
    };

    if workflow_restarted {
        if let Some(stop_reason) = hook_stop_reason {
            emit_hook_stopped_notice(dispatcher, session_id, stop_reason);
        }
        return Ok(true);
    }

    // Finish-window guard: a producer can enqueue into `pending_events` after
    // `settle_before_terminal_transition` returns `Ok(false)` but before we write
    // `Idle`. Without this second scan, parents polling `checkSession(wait=true)`
    // could observe an empty terminal result while another LLM turn is required.
    if continue_workflow_if_pending_events(
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        restart,
        session_id,
    )
    .await?
    {
        if let Some(stop_reason) = hook_stop_reason {
            emit_hook_stopped_notice(dispatcher, session_id, stop_reason);
        }
        return Ok(true);
    }

    // A `PreToolUse`/`PostToolUse` hook already stopped this turn (#3004 G3.2):
    // decide here that the `Stop` hook does not run, settle to `Idle` with the
    // hook's reason, and carry that reason in `WorkflowCompleted` so the UI can
    // show it on a non-error end of turn (#3004 G3.1).
    if let Some(stop_reason) = hook_stop_reason {
        settle_hook_stopped_to_idle(
            session_repo,
            active_sessions,
            app_handle,
            dispatcher,
            session_id,
            stop_reason,
        )
        .await?;
        return Ok(false);
    }

    // Stop / SubagentStop hook (#2925, #3004 G1.3): run before settling to Idle.
    // - `continue: false` settles and surfaces `stopReason` to the user.
    // - `decision: "block"` feeds the block reason back and continues the turn.
    match maybe_continue_from_stop_hook(
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        restart,
        session_id,
    )
    .await?
    {
        StopHookTransition::Continue => return Ok(true),
        StopHookTransition::StoppedByHook(reason) => {
            settle_hook_stopped_to_idle(
                session_repo,
                active_sessions,
                app_handle,
                dispatcher,
                session_id,
                &reason,
            )
            .await?;
            return Ok(false);
        }
        StopHookTransition::Settle => {}
    }

    crate::services::hooks::reset_stop_hook_continuations(session_id);

    transition_to_terminal_status_and_emit(
        session_repo,
        active_sessions,
        dispatcher,
        session_id,
        TerminalTransition {
            status: SessionStatus::Idle,
            event: AgentEvent::WorkflowCompleted {
                session_id: session_id.to_string(),
                reason: completion_reason,
                stop_reason: None,
            },
            event_label: "WorkflowCompleted",
        },
    )
    .await?;

    Ok(false)
}

/// Finish a workflow only after terminal messages are durable and no pending work remains.
///
/// Returns `Ok(true)` when pending events restarted the workflow (caller must not treat
/// the session as terminal). Returns `Ok(false)` when the session settled to `Idle`.
#[allow(clippy::too_many_arguments)]
pub async fn settle_session_and_go_idle(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    restart: &dyn WorkflowRestart,
    session_id: &str,
    terminal_message: Option<&Message>,
    completion_reason: WorkflowCompletionReason,
) -> Result<bool, String> {
    let dispatcher = crate::agent::tauri_events::TauriEventDispatcher::new(app_handle);
    settle_session_and_go_idle_with_dispatcher(
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        restart,
        &dispatcher,
        session_id,
        terminal_message,
        completion_reason,
        None,
    )
    .await
}

/// Finalize a failed workflow only after terminal messages are durable.
///
/// Unlike the success settlement path, this does not re-check `pending_events`.
/// Doing so would create an async recursion cycle through `handle_llm_error_with_outcome`.
pub async fn settle_session_and_finalize_error_with_dispatcher(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    terminal_message: Option<&Message>,
    error: AgentRuntimeError,
) -> Result<(), String> {
    if let Err(persist_error) =
        persist_terminal_assistant_sync(active_sessions, session_id, terminal_message).await
    {
        return handle_terminal_persist_failure(
            session_repo,
            active_sessions,
            dispatcher,
            session_id,
            persist_error,
        )
        .await;
    }

    transition_to_terminal_status_and_emit(
        session_repo,
        active_sessions,
        dispatcher,
        session_id,
        TerminalTransition {
            status: SessionStatus::Error,
            event: AgentEvent::WorkflowError {
                session_id: session_id.to_string(),
                error,
            },
            event_label: "WorkflowError",
        },
    )
    .await
}

/// Finalize a failed workflow only after terminal messages are durable.
///
/// Unlike the success settlement path, this does not re-check `pending_events`.
/// Doing so would create an async recursion cycle through `handle_llm_error_with_outcome`.
pub async fn settle_session_and_finalize_error(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    terminal_message: Option<&Message>,
    error: AgentRuntimeError,
) -> Result<(), String> {
    let dispatcher = crate::agent::tauri_events::TauriEventDispatcher::new(app_handle);
    settle_session_and_finalize_error_with_dispatcher(
        session_repo,
        active_sessions,
        &dispatcher,
        session_id,
        terminal_message,
        error,
    )
    .await
}
