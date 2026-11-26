//! Stop / SubagentStop hook continuation before settling to Idle.

use super::WorkflowRestart;
use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::SessionRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

/// Choose `Stop` vs `SubagentStop` from whether the session has a parent
/// (#2812 settle stub). Pure so unit tests cover the branch without a full
/// AgentSession.
pub(super) fn settle_stub_event(
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
pub(super) async fn subagent_matcher_value(
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
pub(super) fn subagent_matcher_value_with_names(
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
pub(super) enum StopHookTransition {
    Continue,
    Settle,
    StoppedByHook(String),
}

/// Dispatch `Stop` (top-level session) or `SubagentStop` (child session) before
/// settling to Idle (#2925, #3004 G1.3).
pub(super) async fn maybe_continue_from_stop_hook(
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
pub(super) fn evaluate_stop_hook_outcome(
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
