//! Centralized workflow settlement gate.
//!
//! All natural-completion and workflow-error finalization paths converge here so
//! terminal assistant messages are durable before parents or the UI observe
//! `Idle` / `Error`.
//!
//! See `docs/architecture/workflow-settlement.md` for the finish-window TOCTOU
//! scenario and why the success path performs two pending-event scans.

use crate::agent::events::{AgentEvent, AgentEventDispatcher, WorkflowCompletionReason};
use crate::agent::llm::types::{AgentRuntimeError, AgentRuntimeErrorType};
use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
use crate::repositories::{SessionRepository, SessionStatus};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

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

fn latest_assistant_message_from_cache(messages: &[Message]) -> Option<Message> {
    messages
        .iter()
        .rev()
        .find(|message| message.role == "assistant")
        .cloned()
}

fn user_facing_persist_session_missing_error() -> String {
    "The session could not be found while saving the final response.".to_string()
}

fn user_facing_persist_storage_error() -> String {
    "The final response could not be saved. The session has been marked as failed.".to_string()
}

fn user_facing_workflow_event_emit_error(event_label: &str) -> String {
    format!(
        "The session state was updated, but the UI could not be notified ({event_label}). \
         Refresh or check the session status manually."
    )
}

fn emit_settlement_agent_event(
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    event: AgentEvent,
    event_label: &str,
) -> Result<(), String> {
    dispatcher.emit_agent_event(event).map_err(|error| {
        log::error!(
            "Failed to emit {event_label} for session {session_id}: {error}. \
                 Session status was already updated; StatusChanged should have reached the UI."
        );
        user_facing_workflow_event_emit_error(event_label)
    })
}

pub async fn session_has_pending_events(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> bool {
    let active = active_sessions.read().await;
    if let Some(session) = active.get(session_id) {
        return session.pending_events.read().await.count() > 0;
    }

    false
}

/// Re-check `pending_events` immediately before transitioning to Idle.
///
/// Returns `true` when pending messages were found and a new LLM turn was
/// requested. Callers must skip the Idle transition in that case.
///
/// This is invoked twice on the success settlement path: once inside
/// [`settle_before_terminal_transition`] and again immediately before writing
/// `Idle`. Between those two scans another task can enqueue into
/// `pending_events` (the finish-window race). The second scan closes that
/// window without introducing a new terminal state machine.
pub async fn continue_workflow_if_pending_events(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    restart: &dyn WorkflowRestart,
    session_id: &str,
) -> Result<bool, String> {
    if !session_has_pending_events(active_sessions, session_id).await {
        return Ok(false);
    }

    log::info!(
        "Pending messages detected for session {} during workflow finish. Continuing workflow.",
        session_id
    );

    restart
        .restart(
            session_repo,
            active_sessions,
            proxy_manager,
            app_handle,
            session_id.to_string(),
        )
        .await?;

    Ok(true)
}

/// Persist the terminal assistant message synchronously before exposing `Idle`.
///
/// When `terminal_message` is provided it is written directly; otherwise the latest
/// assistant row from the in-memory cache is used.
pub async fn persist_terminal_assistant_sync(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    terminal_message: Option<&Message>,
) -> Result<(), String> {
    let message_to_persist = if let Some(message) = terminal_message {
        Some(message.clone())
    } else {
        let sessions = active_sessions.read().await;
        let session = sessions.get(session_id).ok_or_else(|| {
            log::error!(
                "Terminal persist cache lookup failed: session {} is not in active_sessions",
                session_id
            );
            user_facing_persist_session_missing_error()
        })?;
        let messages = session.messages.read().await;
        latest_assistant_message_from_cache(&messages)
    };

    let Some(message) = message_to_persist else {
        return Ok(());
    };

    let repo = crate::state::get_message_repository();
    repo.insert(&message).await.map_err(|error| {
        log::error!(
            "Failed to persist terminal assistant message: session={}, msg_id={}, error={}",
            session_id,
            message.id,
            error
        );
        user_facing_persist_storage_error()
    })
}

/// Persist terminal messages and run the first pending-work scan.
///
/// Returns `Ok(true)` when pending events restarted the workflow. The success
/// path still performs a second pending scan in
/// [`settle_session_and_go_idle_with_dispatcher`] to close the finish-window
/// race that can occur after this function returns.
pub async fn settle_before_terminal_transition(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    restart: &dyn WorkflowRestart,
    session_id: &str,
    terminal_message: Option<&Message>,
) -> Result<bool, String> {
    persist_terminal_assistant_sync(active_sessions, session_id, terminal_message).await?;

    continue_workflow_if_pending_events(
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        restart,
        session_id,
    )
    .await
}

async fn mark_error_after_persist_failure(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    user_facing_error: &str,
) -> Result<(), String> {
    log::error!(
        "Terminal persist failed for session {}. Transitioning to Error. user_message={}",
        session_id,
        user_facing_error
    );
    crate::agent::lifecycle::update_session_status_with_dispatcher(
        session_repo,
        active_sessions,
        dispatcher,
        session_id,
        SessionStatus::Error,
    )
    .await?;

    let workflow_error =
        AgentRuntimeError::new(AgentRuntimeErrorType::AiServiceError, user_facing_error)
            .with_code("TERMINAL_PERSIST_FAILED");

    if let Err(emit_error) = dispatcher.emit_agent_event(AgentEvent::WorkflowError {
        session_id: session_id.to_string(),
        error: workflow_error,
    }) {
        log::error!(
            "Session {} is Error after terminal persist failure, but WorkflowError failed to emit: {}. \
             StatusChanged should still have reached the UI.",
            session_id,
            emit_error
        );
    }

    Ok(())
}

async fn handle_terminal_persist_failure(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    persist_error: String,
) -> Result<(), String> {
    if let Err(mark_error) = mark_error_after_persist_failure(
        session_repo,
        active_sessions,
        dispatcher,
        session_id,
        &persist_error,
    )
    .await
    {
        log::error!(
            "Persist failed for session {} and marking Error also failed: {}",
            session_id,
            mark_error
        );
    }
    Err(persist_error)
}

async fn transition_to_terminal_status_and_emit(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    status: SessionStatus,
    event: AgentEvent,
    event_label: &str,
) -> Result<(), String> {
    crate::agent::lifecycle::update_session_status_with_dispatcher(
        session_repo,
        active_sessions,
        dispatcher,
        session_id,
        status,
    )
    .await?;

    emit_settlement_agent_event(dispatcher, session_id, event, event_label)
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
        SessionStatus::Idle,
        AgentEvent::WorkflowCompleted {
            session_id: session_id.to_string(),
            reason: completion_reason,
            stop_reason: None,
        },
        "WorkflowCompleted",
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

/// Choose `Stop` vs `SubagentStop` from whether the session has a parent
/// (#2812 settle stub). Pure so unit tests cover the branch without a full
/// AgentSession.
pub(crate) fn settle_stub_event(
    parent_session_id: Option<&str>,
) -> crate::services::hooks::HookEvent {
    if parent_session_id.is_some() {
        crate::services::hooks::HookEvent::SubagentStop
    } else {
        crate::services::hooks::HookEvent::Stop
    }
}

/// Resolve the matcher value (agent type) for `SubagentStop` (#3003 G1.5,
/// G2.3). Plugin agents carry deterministic assistant ids
/// `plugin:<source_id>:<agent>` where `<source_id>` is an internal
/// `plugin-<16 hex>` id no user would write in a matcher, so the plugin's
/// **namespace** (manifest name, else repository name — resolved from the
/// session plugin store first, then the global store, exactly like the
/// plugin agent definitions themselves) is used for the scope
/// (`my-plugin:reviewer`). User agents carry an internal uuid, so the
/// assistant's **name** from the repository is used. Falls back to the
/// source id (plugin) or the session name (user) with a warning when the
/// lookup fails. Returns `""` for top-level sessions (`Stop` never matches
/// an agent type).
pub(crate) async fn subagent_matcher_value(
    parent_session_id: Option<&str>,
    assistant_id: Option<&str>,
    session_name: Option<&str>,
) -> String {
    let Some(parent_id) = parent_session_id else {
        return String::new();
    };
    let trimmed_id = assistant_id.map(str::trim).filter(|s| !s.is_empty());
    if let Some(id) = trimmed_id {
        if let Some((source_id, _agent)) = crate::services::skill_service::parse_plugin_agent_id(id)
        {
            // Plugin scope = the plugin namespace from the parent session's
            // plugin store first, then the global store (#3003 G2.3):
            // session-installed plugins live in the parent session's store,
            // matching spawn (`load_assistant_or_plugin_agent`) and resume
            // (`load_assistant_or_plugin`).
            let scope = crate::services::skill_service::resolve_plugin_source_namespace(
                source_id, parent_id,
            );
            if scope.is_none() {
                log::warn!(
                    "SubagentStop: plugin source {source_id} not found in session or global stores; matching on the source id"
                );
            }
            return subagent_matcher_value_with_names(
                parent_session_id,
                assistant_id,
                session_name,
                scope.as_deref(),
                None,
            );
        }
        // User agent: the assistant configuration's name (#3003 G2.3).
        use crate::repositories::assistant_repository::AssistantRepository;
        let assistant_name: Option<String> = crate::state::get_assistant_repository()
            .get_assistant(id)
            .await
            .ok()
            .flatten()
            .map(|assistant| assistant.name)
            .filter(|name: &String| !name.trim().is_empty());
        if assistant_name.is_none() {
            log::warn!("SubagentStop: assistant {id} not found; matching on the session name");
        }
        return subagent_matcher_value_with_names(
            parent_session_id,
            assistant_id,
            session_name,
            None,
            assistant_name.as_deref(),
        );
    }
    subagent_matcher_value_with_names(parent_session_id, assistant_id, session_name, None, None)
}

/// Pure core of [`subagent_matcher_value`] with the plugin manifest name and
/// assistant name already resolved, so unit tests cover the mapping without
/// the plugin registry or the database.
fn subagent_matcher_value_with_names(
    parent_session_id: Option<&str>,
    assistant_id: Option<&str>,
    session_name: Option<&str>,
    plugin_name: Option<&str>,
    assistant_name: Option<&str>,
) -> String {
    if parent_session_id.is_none() {
        return String::new();
    }
    if let Some(id) = assistant_id.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some((source_id, agent)) = crate::services::skill_service::parse_plugin_agent_id(id)
        {
            let scope = plugin_name
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(source_id);
            return format!("{scope}:{agent}");
        }
        if let Some(name) = assistant_name.map(str::trim).filter(|s| !s.is_empty()) {
            return name.to_string();
        }
    }
    session_name
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("")
        .to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StopHookTransition {
    Continue,
    Settle,
    StoppedByHook(String),
}

/// Dispatch `Stop` (top-level session) or `SubagentStop` (child session) before
/// settling to Idle (#2925, #3004 G1.3).
async fn maybe_continue_from_stop_hook(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    restart: &dyn WorkflowRestart,
    session_id: &str,
) -> Result<StopHookTransition, String> {
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
    let event = settle_stub_event(parent_session_id.as_deref());
    let matcher_value = subagent_matcher_value(
        parent_session_id.as_deref(),
        assistant_id.as_deref(),
        session_name.as_deref(),
    )
    .await;
    let mut params = crate::services::hooks::HookEnvelopeParams::new(
        session_id,
        std::path::PathBuf::from("."),
        event,
    );
    params.stop_hook_active = crate::services::hooks::stop_hook_continuation_count(session_id) > 0;
    if parent_session_id.is_some() {
        params.child_session_id = Some(session_id.to_string());
        // Claude Code: `agent_id` is the unique subagent instance, `agent_type`
        // is the name matchers filter on (#3003 G1.5).
        params.agent_id = Some(session_id.to_string());
        if !matcher_value.is_empty() {
            params.agent_type = Some(matcher_value.clone());
        }
    }
    let outcome =
        crate::services::hooks::dispatch_hook_event(event, &matcher_value, &params, session_id)
            .await;

    let transition = evaluate_stop_hook_outcome(session_id, event, outcome);
    if transition == StopHookTransition::Continue {
        restart
            .restart(
                session_repo,
                active_sessions,
                proxy_manager,
                app_handle,
                session_id.to_string(),
            )
            .await?;
    }
    Ok(transition)
}

/// Evaluate a `Stop` / `SubagentStop` hook result against `MAX_STOP_HOOK_CONTINUATIONS`
/// (#2925 G1.2, #3004 G2.1/G2.2).
pub(crate) fn evaluate_stop_hook_outcome(
    session_id: &str,
    event: crate::services::hooks::HookEvent,
    outcome: crate::services::hooks::HookDispatchResult,
) -> StopHookTransition {
    // `continue: false` (#3004): the turn settles as Idle with `stopReason`
    // surfaced to the user, even when the same result also asks for a block.
    if outcome.stops_processing {
        let reason = outcome.stop_message("Stopped by hook.");
        log::info!(
            "{} hook stopped processing for session {}: {}",
            event.as_str(),
            session_id,
            reason
        );
        return StopHookTransition::StoppedByHook(reason);
    }
    if outcome.decision != Some(crate::services::hooks::HookDecision::Block) {
        return StopHookTransition::Settle;
    }
    if !crate::services::hooks::try_increment_stop_hook_continuations(session_id) {
        log::info!(
            "{} hook returned block for session {} but reached MAX_STOP_HOOK_CONTINUATIONS ({}); settling to Idle",
            event.as_str(),
            session_id,
            crate::services::hooks::MAX_STOP_HOOK_CONTINUATIONS
        );
        return StopHookTransition::Settle;
    }
    if let Some(ctx) = outcome.joined_additional_context() {
        crate::services::hooks::push_turn_hook_context(session_id, &ctx);
    }
    let reason = outcome
        .reason
        .filter(|r| !r.trim().is_empty())
        .unwrap_or_else(|| {
            format!(
                "{} hook requested continuation before stopping.",
                event.as_str()
            )
        });
    crate::services::hooks::push_turn_hook_context(
        session_id,
        &format!("[{} hook]: {}", event.as_str(), reason),
    );
    StopHookTransition::Continue
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
        SessionStatus::Error,
        AgentEvent::WorkflowError {
            session_id: session_id.to_string(),
            error,
        },
        "WorkflowError",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::context::registry::ContextRegistry;
    use crate::agent::events::AgentEventDispatcher;
    use crate::agent::llm::types::{AgentRuntimeError, AgentRuntimeErrorType};
    use crate::agent::state::{CompactionRuntimeState, PendingEvent, PendingEventManager};
    use crate::repositories::{
        InMemorySessionRepository, SessionMetadata, SessionRepository, SessionStatus,
    };
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};
    use std::time::SystemTime;
    use tokio_util::sync::CancellationToken;

    #[derive(Default)]
    struct WorkflowEventFailingDispatcher {
        fail_workflow_events: bool,
        events: Mutex<Vec<AgentEvent>>,
    }

    impl AgentEventDispatcher for WorkflowEventFailingDispatcher {
        fn emit_agent_event(&self, event: AgentEvent) -> Result<(), String> {
            if self.fail_workflow_events
                && matches!(
                    event,
                    AgentEvent::WorkflowCompleted { .. } | AgentEvent::WorkflowError { .. }
                )
            {
                return Err("event emission failed".to_string());
            }
            self.events.lock().expect("event lock").push(event);
            Ok(())
        }

        fn emit_compact_state(
            &self,
            _event: crate::agent::llm::types::CompactStateEvent,
        ) -> Result<(), String> {
            Ok(())
        }

        fn emit_completion_cancel(
            &self,
            _request: crate::agent::llm::types::CompletionCancelRequest,
        ) -> Result<(), String> {
            Ok(())
        }
    }

    async fn seed_in_memory_session(
        session_repo: &Arc<dyn SessionRepository>,
        active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
        session_id: &str,
        status: SessionStatus,
    ) {
        let metadata = SessionMetadata {
            id: session_id.to_string(),
            name: None,
            status: status.clone(),
            model: "gpt-5.4".to_string(),
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
            created_at: 1,
            updated_at: 1,
            last_viewed_at: None,
            last_message_at: None,
            last_attention_at: None,
            last_attention_reason: None,
            is_bookmarked: false,
            execution_mode: crate::execution_mode::ExecutionMode::Normal,
            workspace_override: None,
            workspace_isolation: crate::models::workspace_isolation::WorkspaceIsolationMode::Host,
            docker_config: None,
            docker_container_name: None,
            docker_host_workspace_path: None,
            status_reason: None,
            forked_from: None,
        };
        session_repo
            .upsert_session(&metadata)
            .await
            .expect("session upsert");
        active_sessions
            .write()
            .await
            .insert(session_id.to_string(), build_session(session_id));
    }

    #[derive(Default)]
    struct RecordingDispatcher {
        events: Mutex<Vec<AgentEvent>>,
    }

    impl RecordingDispatcher {
        fn agent_events(&self) -> Vec<AgentEvent> {
            self.events.lock().expect("event lock").clone()
        }
    }

    impl AgentEventDispatcher for RecordingDispatcher {
        fn emit_agent_event(&self, event: AgentEvent) -> Result<(), String> {
            self.events.lock().expect("event lock").push(event);
            Ok(())
        }

        fn emit_compact_state(
            &self,
            _event: crate::agent::llm::types::CompactStateEvent,
        ) -> Result<(), String> {
            Ok(())
        }

        fn emit_completion_cancel(
            &self,
            _request: crate::agent::llm::types::CompletionCancelRequest,
        ) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn user_facing_persist_errors_hide_internal_details() {
        assert_eq!(
            user_facing_persist_session_missing_error(),
            "The session could not be found while saving the final response."
        );
        assert!(!user_facing_persist_storage_error().contains("session="));
        assert!(!user_facing_workflow_event_emit_error("WorkflowCompleted").contains("emit"));
    }

    #[tokio::test]
    async fn mark_error_after_persist_failure_emits_workflow_error() {
        crate::init_session_bus(crate::agent::session_bus::SessionBus::new());
        crate::state::init_concurrency_gate(crate::agent::concurrency::ConcurrencyGate::new(
            crate::agent::concurrency::DEFAULT_MAX_ACTIVE_AGENTS,
            crate::agent::concurrency::DEFAULT_MAX_SUSPENDED_AGENTS,
            crate::agent::concurrency::DEFAULT_MAX_ACTIVE_PROCESSES,
            crate::agent::concurrency::DEFAULT_MAX_SUSPENDED_PROCESSES,
        ));

        let session_repo = Arc::new(InMemorySessionRepository::new()) as Arc<dyn SessionRepository>;
        let active_sessions = Arc::new(RwLock::new(HashMap::new()));
        seed_in_memory_session(
            &session_repo,
            &active_sessions,
            "sess-persist-mark-error",
            SessionStatus::Busy,
        )
        .await;

        let dispatcher = RecordingDispatcher::default();
        let user_message = user_facing_persist_storage_error();
        mark_error_after_persist_failure(
            &session_repo,
            &active_sessions,
            &dispatcher,
            "sess-persist-mark-error",
            &user_message,
        )
        .await
        .expect("mark error succeeds");

        assert_eq!(
            session_repo
                .get_session("sess-persist-mark-error")
                .await
                .expect("session load")
                .expect("session exists")
                .status,
            SessionStatus::Error
        );
        assert!(dispatcher.agent_events().iter().any(|event| {
            matches!(
                event,
                AgentEvent::WorkflowError { session_id, error }
                    if session_id == "sess-persist-mark-error"
                        && error.display_message == user_message
            )
        }));
    }

    #[tokio::test]
    async fn settle_session_and_finalize_error_with_dispatcher_propagates_emit_failure() {
        crate::init_session_bus(crate::agent::session_bus::SessionBus::new());
        crate::state::init_concurrency_gate(crate::agent::concurrency::ConcurrencyGate::new(
            crate::agent::concurrency::DEFAULT_MAX_ACTIVE_AGENTS,
            crate::agent::concurrency::DEFAULT_MAX_SUSPENDED_AGENTS,
            crate::agent::concurrency::DEFAULT_MAX_ACTIVE_PROCESSES,
            crate::agent::concurrency::DEFAULT_MAX_SUSPENDED_PROCESSES,
        ));

        let session_repo = Arc::new(InMemorySessionRepository::new()) as Arc<dyn SessionRepository>;
        let active_sessions = Arc::new(RwLock::new(HashMap::new()));
        seed_in_memory_session(
            &session_repo,
            &active_sessions,
            "sess-emit-failure",
            SessionStatus::Busy,
        )
        .await;

        let dispatcher = WorkflowEventFailingDispatcher {
            fail_workflow_events: true,
            ..Default::default()
        };
        let runtime_error = AgentRuntimeError::new(
            AgentRuntimeErrorType::AiServiceError,
            "Provider request failed",
        );

        let result = settle_session_and_finalize_error_with_dispatcher(
            &session_repo,
            &active_sessions,
            &dispatcher,
            "sess-emit-failure",
            None,
            runtime_error,
        )
        .await;

        assert_eq!(
            result,
            Err(user_facing_workflow_event_emit_error("WorkflowError"))
        );
        assert_eq!(
            session_repo
                .get_session("sess-emit-failure")
                .await
                .expect("session load")
                .expect("session exists")
                .status,
            SessionStatus::Error
        );
    }

    fn build_session(session_id: &str) -> AgentSession {
        let now = chrono::Utc::now().timestamp_millis();
        AgentSession {
            metadata: SessionMetadata {
                id: session_id.to_string(),
                name: None,
                status: SessionStatus::Busy,
                model: "gpt-5.4".to_string(),
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
                created_at: now,
                updated_at: now,
                last_viewed_at: None,
                last_message_at: None,
                last_attention_at: None,
                last_attention_reason: None,
                is_bookmarked: false,
                execution_mode: crate::execution_mode::ExecutionMode::Normal,
                workspace_override: None,
                workspace_isolation:
                    crate::models::workspace_isolation::WorkspaceIsolationMode::Host,
                docker_config: None,
                docker_container_name: None,
                docker_host_workspace_path: None,
                status_reason: None,
                forked_from: None,
            },
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
            pending_user_prompt: crate::agent::state::new_user_prompt_slot(),
            context_registry: Arc::new(ContextRegistry::new()),
            compact_context: Arc::new(RwLock::new(None)),
            compaction: CompactionRuntimeState::new(),
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

    #[tokio::test]
    async fn session_has_pending_events_is_false_when_queue_empty() {
        let sessions = Arc::new(RwLock::new(HashMap::from([(
            "sess-1".to_string(),
            build_session("sess-1"),
        )])));

        assert!(!session_has_pending_events(&sessions, "sess-1").await);
    }

    #[tokio::test]
    async fn session_has_pending_events_is_true_when_queue_has_messages() {
        let session = build_session("sess-2");
        session
            .pending_events
            .write()
            .await
            .add(PendingEvent::Message("msg-1".to_string()));

        let sessions = Arc::new(RwLock::new(HashMap::from([(
            "sess-2".to_string(),
            session,
        )])));

        assert!(session_has_pending_events(&sessions, "sess-2").await);
    }

    #[test]
    fn latest_assistant_message_from_cache_returns_newest_assistant() {
        use crate::mcp::types::MCPContent;
        use crate::models::chat::Message;

        fn assistant_message(id: &str, text: &str) -> Message {
            Message {
                id: id.to_string(),
                session_id: "sess".to_string(),
                role: "assistant".to_string(),
                content: vec![MCPContent::Text {
                    text: text.to_string(),
                }],
                tool_calls: None,
                tool_call_id: None,
                is_streaming: None,
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

        let messages = vec![
            assistant_message("asst-1", "first"),
            Message {
                id: "tool-1".to_string(),
                session_id: "sess".to_string(),
                role: "tool".to_string(),
                content: vec![MCPContent::Text {
                    text: "tool output".to_string(),
                }],
                tool_calls: None,
                tool_call_id: None,
                is_streaming: None,
                thinking: None,
                thinking_signature: None,
                assistant_id: None,
                attachments: None,
                tool_use: None,
                usage: None,
                prompt_tokens: None,
                created_at: 2,
                updated_at: 2,
                source: None,
                error: None,
                metadata: None,
            },
            assistant_message("asst-2", "final answer"),
        ];

        let latest = latest_assistant_message_from_cache(&messages).expect("assistant");
        assert_eq!(latest.id, "asst-2");
        let MCPContent::Text { text, .. } = &latest.content[0] else {
            panic!("expected text content");
        };
        assert_eq!(text, "final answer");
    }

    #[test]
    fn settle_stub_event_picks_stop_for_top_level_session() {
        assert_eq!(
            settle_stub_event(None),
            crate::services::hooks::HookEvent::Stop
        );
    }

    #[test]
    fn settle_stub_event_picks_subagent_stop_when_parent_set() {
        assert_eq!(
            settle_stub_event(Some("parent-session")),
            crate::services::hooks::HookEvent::SubagentStop
        );
    }

    #[test]
    fn evaluate_stop_hook_outcome_continues_up_to_cap_then_settles() {
        use crate::services::hooks::{
            clear_hook_context, take_turn_hook_context, HookDecision, HookDispatchResult,
            HookEvent, MAX_STOP_HOOK_CONTINUATIONS,
        };

        let sid = "finish-stop-hook-cap-test";
        clear_hook_context(sid);

        for i in 1..=MAX_STOP_HOOK_CONTINUATIONS {
            let cont = super::evaluate_stop_hook_outcome(
                sid,
                HookEvent::Stop,
                HookDispatchResult {
                    additional_context: vec![],
                    decision: Some(HookDecision::Block),
                    reason: Some(format!("keep going #{i}")),
                    updated_input: None,
                    stop_reason: None,
                    stops_processing: false,
                },
            );
            assert_eq!(
                cont,
                super::StopHookTransition::Continue,
                "continuation #{i} must be allowed"
            );
            let ctx = take_turn_hook_context(sid).expect("feedback queued");
            assert!(ctx.contains(&format!("keep going #{i}")));
        }

        // Next block exceeds MAX_STOP_HOOK_CONTINUATIONS -> must settle to Idle.
        let over_cap = super::evaluate_stop_hook_outcome(
            sid,
            HookEvent::Stop,
            HookDispatchResult {
                additional_context: vec![],
                decision: Some(HookDecision::Block),
                reason: Some("one too many".to_string()),
                updated_input: None,
                stop_reason: None,
                stops_processing: false,
            },
        );
        assert_eq!(
            over_cap,
            super::StopHookTransition::Settle,
            "must not continue past MAX_STOP_HOOK_CONTINUATIONS"
        );
        assert!(take_turn_hook_context(sid).is_none());
        clear_hook_context(sid);
    }

    #[test]
    fn subagent_matcher_value_uses_agent_type_for_children_only() {
        // Ids exactly as the app builds them: plugin source ids are
        // `plugin-<16 hex>` (skill_service::importing::plugin_source_id), so
        // a plugin agent row is `plugin:plugin-<16 hex>:<agent>` (#3003 G2.3).
        assert_eq!(
            super::subagent_matcher_value_with_names(
                None,
                Some("Explore"),
                Some("child"),
                None,
                None
            ),
            ""
        );
        // Plugin agents resolve to the plugin manifest name for the scope.
        assert_eq!(
            super::subagent_matcher_value_with_names(
                Some("parent-1"),
                Some("plugin:plugin-1a2b3c4d5e6f7a8b:reviewer"),
                Some("child-session-name"),
                Some("my-plugin"),
                None,
            ),
            "my-plugin:reviewer"
        );
        // A plugin whose manifest name cannot be read falls back to the
        // source id (still better than the session name).
        assert_eq!(
            super::subagent_matcher_value_with_names(
                Some("parent-1"),
                Some("plugin:plugin-1a2b3c4d5e6f7a8b:reviewer"),
                Some("child-session-name"),
                None,
                None,
            ),
            "plugin-1a2b3c4d5e6f7a8b:reviewer"
        );
        // User agents resolve to the assistant configuration's name.
        assert_eq!(
            super::subagent_matcher_value_with_names(
                Some("parent-1"),
                Some("b15fdf3e-6192-453d-9dc2-7b8ae6dab321"),
                Some("child-session-name"),
                None,
                Some("reviewer"),
            ),
            "reviewer"
        );
        // Without an assistant row, the session name remains the fallback.
        assert_eq!(
            super::subagent_matcher_value_with_names(
                Some("parent-1"),
                Some("b15fdf3e-6192-453d-9dc2-7b8ae6dab321"),
                Some("reviewer"),
                None,
                None,
            ),
            "reviewer"
        );
        assert_eq!(
            super::subagent_matcher_value_with_names(
                Some("parent-1"),
                None,
                Some("reviewer"),
                None,
                None
            ),
            "reviewer"
        );
        assert_eq!(
            super::subagent_matcher_value_with_names(
                Some("parent-1"),
                Some("  "),
                None,
                None,
                None
            ),
            ""
        );
    }

    /// Drives the production async wrapper (not just the pure core) with an
    /// app-built plugin id. The plugin is not installed in the unit-test
    /// environment, so the namespace lookup misses and the scope falls back
    /// to the source id — proving the wrapper resolves through the
    /// session+global store path without panicking (#3003 G2.3).
    #[tokio::test]
    async fn subagent_matcher_value_resolves_plugin_scope_async() {
        assert_eq!(
            super::subagent_matcher_value(
                Some("parent-1"),
                Some("plugin:plugin-1a2b3c4d5e6f7a8b:reviewer"),
                Some("child-session-name"),
            )
            .await,
            "plugin-1a2b3c4d5e6f7a8b:reviewer"
        );
        assert_eq!(
            super::subagent_matcher_value(
                None,
                Some("plugin:plugin-1a2b3c4d5e6f7a8b:reviewer"),
                Some("child")
            )
            .await,
            ""
        );
        assert_eq!(
            super::subagent_matcher_value(Some("parent-1"), None, Some("reviewer")).await,
            "reviewer"
        );
    }

    /// #3003 G2.3: session-scoped plugin agents live in the **parent**
    /// session's plugin store, so `subagent_matcher_value` must resolve the
    /// plugin namespace from the parent session's store and return
    /// `<manifest-name>:<agent>`.
    #[tokio::test]
    async fn subagent_matcher_value_resolves_parent_session_plugin_namespace_async() {
        use crate::services::hooks::registry::{
            get_session_plugins_directory, save_source_in, upsert_session_store_entry,
            PluginMetadata, PluginSourceManifest, SessionPluginStoreEntry,
        };
        let parent_id = format!("test-parent-{}", uuid::Uuid::new_v4());
        let source_id = "plugin-1a2b3c4d5e6f7a8b";
        let plugins_dir = get_session_plugins_directory(&parent_id).expect("session plugins dir");
        std::fs::create_dir_all(plugins_dir.join(source_id)).expect("create plugin dir");

        let manifest = PluginSourceManifest {
            source_url: "https://github.com/example/my-plugin".to_string(),
            source_id: source_id.to_string(),
            skill_names: Vec::new(),
            hooks: None,
            hooks_approved: false,
            approval_hashes: std::collections::BTreeMap::new(),
            created_at: 0,
            plugin: Some(PluginMetadata {
                name: "my-plugin".to_string(),
                version: None,
                description: None,
                author: None,
                homepage: None,
                repository: None,
                license: None,
                keywords: Vec::new(),
            }),
            hook_files: Vec::new(),
            inline_hooks: Vec::new(),
            mcp_servers: Vec::new(),
            mcp_approved: false,
            enabled: true,
            installed_commit: None,
        };
        save_source_in(&plugins_dir, manifest).expect("save plugin manifest");
        upsert_session_store_entry(
            &plugins_dir,
            SessionPluginStoreEntry {
                source_id: source_id.to_string(),
                source_url: "https://github.com/example/my-plugin".to_string(),
                installed_at: 0,
            },
        )
        .expect("upsert session store entry");

        let resolved = super::subagent_matcher_value(
            Some(&parent_id),
            Some("plugin:plugin-1a2b3c4d5e6f7a8b:reviewer"),
            Some("child-session-name"),
        )
        .await;

        // Clean up the test parent's default workspace directory.
        if let Ok(manager) = crate::session::get_session_manager() {
            let _ = std::fs::remove_dir_all(
                manager.get_session_default_workspace_dir_unverified(&parent_id),
            );
        }

        assert_eq!(resolved, "my-plugin:reviewer");
    }

    /// #3003 G2.3: a user agent's matcher value is the **assistant
    /// configuration's name** read from the repository — a real lookup, not
    /// an injected string.
    #[tokio::test]
    async fn subagent_matcher_value_resolves_user_assistant_name_async() {
        use sea_orm_migration::MigratorTrait;
        let _guard = crate::state::lock_test_global_state().await;
        crate::lifecycle::database::register_sqlite_vec();
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("memory db");
        crate::migration::Migrator::up(&db, None)
            .await
            .expect("migrations");
        crate::state::set_assistant_repository(
            crate::repositories::assistant_repository::SqliteAssistantRepository::new(db),
        );
        use crate::repositories::AssistantRepository;
        crate::state::get_assistant_repository()
            .create_assistant(
                "b15fdf3e-6192-453d-9dc2-7b8ae6dab321".to_string(),
                "Code Reviewer".to_string(),
                "{}".to_string(),
            )
            .await
            .unwrap();

        assert_eq!(
            super::subagent_matcher_value(
                Some("parent-1"),
                Some("b15fdf3e-6192-453d-9dc2-7b8ae6dab321"),
                Some("child-session-name"),
            )
            .await,
            "Code Reviewer"
        );
        // Unknown assistant id → session name fallback (with a warning).
        assert_eq!(
            super::subagent_matcher_value(
                Some("parent-1"),
                Some("00000000-0000-0000-0000-000000000000"),
                Some("child-session-name"),
            )
            .await,
            "child-session-name"
        );
    }

    #[test]
    fn stop_hook_continue_false_settles_with_user_visible_reason_even_with_block() {
        use crate::services::hooks::{
            clear_hook_context, take_turn_hook_context, HookDecision, HookDispatchResult, HookEvent,
        };

        let sid = "finish-stop-hook-stop-test";
        clear_hook_context(sid);
        let transition = super::evaluate_stop_hook_outcome(
            sid,
            HookEvent::Stop,
            HookDispatchResult {
                additional_context: vec![],
                decision: Some(HookDecision::Block),
                reason: Some("keep going".to_string()),
                updated_input: None,
                stop_reason: Some("budget exhausted".to_string()),
                stops_processing: true,
            },
        );
        assert_eq!(
            transition,
            super::StopHookTransition::StoppedByHook("budget exhausted".to_string()),
            "continue:false must settle the turn and surface stopReason"
        );
        assert!(
            take_turn_hook_context(sid).is_none(),
            "a stopped turn must not queue continuation feedback"
        );
        assert_eq!(
            crate::services::hooks::stop_hook_continuation_count(sid),
            0,
            "stopping must not consume the continuation budget"
        );
        clear_hook_context(sid);
    }
}
