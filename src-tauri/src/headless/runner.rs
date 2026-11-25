//! Ephemeral session execution and monitoring for `liblet` headless mode (#2703).

use crate::agent::session_manager::approvals::{
    list_pending_approvals_in, resolve_pending_tool_approval_in,
};
use crate::agent::state::{
    pending_user_prompt_summary, take_any_pending_user_prompt, AgentSession,
    PendingUserPromptSummary,
};
use crate::execution_mode::ExecutionMode;
use crate::headless::cli::{exit_codes, OutputFormat};
use crate::headless::output::{
    stream_events_for_message, summarize_user_prompt_payload, write_stream_event,
    HeadlessPendingApproval, HeadlessResultPayload, HeadlessStreamEvent,
};
use crate::mcp::builtin::agent::utils::latest_session_output;
use crate::models::chat::Message;
use crate::repositories::{SessionRepository, SessionStatus};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{watch, RwLock};
use tokio::time::Instant;

/// OS interrupt signal kind mapped to POSIX exit codes (`130` for `SIGINT`, `143` for `SIGTERM`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptSignal {
    SigInt,
    SigTerm,
}

impl InterruptSignal {
    pub fn exit_code(self) -> i32 {
        match self {
            Self::SigInt => exit_codes::INTERRUPTED_SIGINT,
            Self::SigTerm => exit_codes::TERMINATED_SIGTERM,
        }
    }
}

/// Parameters for monitoring an active ephemeral headless session.
pub struct HeadlessMonitorParams<'a> {
    pub session_repo: &'a Arc<dyn SessionRepository>,
    pub active_sessions: &'a Arc<RwLock<HashMap<String, AgentSession>>>,
    pub session_id: &'a str,
    pub workspace_path: String,
    pub execution_mode: ExecutionMode,
    pub model: String,
    pub provider: String,
    pub assistant_id: Option<String>,
    pub output_format: OutputFormat,
    pub timeout: Option<Duration>,
    pub poll_interval: Duration,
}

async fn pending_user_prompt_in_active(
    active_sessions: &RwLock<HashMap<String, AgentSession>>,
    session_id: &str,
) -> Option<PendingUserPromptSummary> {
    let slot = {
        let active = active_sessions.read().await;
        active.get(session_id)?.pending_user_prompt.clone()
    };
    pending_user_prompt_summary(&slot).await
}

async fn cancel_pending_user_prompt_in_active(
    active_sessions: &RwLock<HashMap<String, AgentSession>>,
    session_id: &str,
) {
    let slot = {
        let active = active_sessions.read().await;
        active
            .get(session_id)
            .map(|s| s.pending_user_prompt.clone())
    };
    if let Some(slot) = slot {
        // Dropping the PendingUserPrompt closes its oneshot channel (`UserPromptOutcome::Stopped`).
        let _ = take_any_pending_user_prompt(&slot).await;
    }
}

async fn reject_all_pending_approvals_in_active(
    active_sessions: &RwLock<HashMap<String, AgentSession>>,
    session_id: &str,
    tool_call_ids: &[String],
) {
    for tool_call_id in tool_call_ids {
        let _ = resolve_pending_tool_approval_in(
            active_sessions,
            None,
            session_id,
            tool_call_id,
            false,
        )
        .await;
    }
}

async fn cancel_session_token_in_active(
    active_sessions: &RwLock<HashMap<String, AgentSession>>,
    session_id: &str,
) {
    let active = active_sessions.read().await;
    if let Some(session) = active.get(session_id) {
        session.cancellation_token.cancel();
    }
}

fn extract_error_from_messages(messages: &[Message]) -> Option<String> {
    for msg in messages.iter().rev() {
        if let Some(err_val) = msg.error.as_ref() {
            if let Some(s) = err_val.as_str() {
                if !s.trim().is_empty() {
                    return Some(s.to_string());
                }
            }
            for key in ["displayMessage", "message", "details"] {
                if let Some(s) = err_val.get(key).and_then(|v| v.as_str()) {
                    if !s.trim().is_empty() {
                        return Some(s.to_string());
                    }
                }
            }
        }
    }
    None
}

async fn snapshot_session_state(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &RwLock<HashMap<String, AgentSession>>,
    session_id: &str,
) -> Option<(SessionStatus, Option<String>, Vec<Message>)> {
    {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(session_id) {
            let messages = session.messages.read().await.clone();
            return Some((
                session.metadata.status.clone(),
                session.metadata.status_reason.clone(),
                messages,
            ));
        }
    }

    if let Ok(Some(meta)) = session_repo.get_session(session_id).await {
        return Some((meta.status, meta.status_reason, Vec::new()));
    }

    None
}

/// Monitor an active ephemeral session until it completes, blocks on approval or
/// user question/secret, times out, is interrupted, or errors.
pub async fn monitor_headless_session<W: Write>(
    params: HeadlessMonitorParams<'_>,
    mut interrupt_rx: watch::Receiver<Option<InterruptSignal>>,
    stream_writer: &mut W,
) -> HeadlessResultPayload {
    let deadline = params.timeout.map(|d| Instant::now() + d);

    if params.output_format == OutputFormat::StreamJson {
        let init_event = HeadlessStreamEvent::Init {
            workspace_path: params.workspace_path.clone(),
            execution_mode: params.execution_mode,
            model: params.model.clone(),
            provider: params.provider.clone(),
            assistant_id: params.assistant_id.clone(),
        };
        let _ = write_stream_event(stream_writer, &init_event);
    }

    let mut seen_message_ids: HashSet<String> = HashSet::new();
    let mut observed_turn_activity = false;

    loop {
        let maybe_sig = *interrupt_rx.borrow();
        if let Some(sig) = maybe_sig {
            cancel_session_token_in_active(params.active_sessions, params.session_id).await;
            return HeadlessResultPayload::interrupted(
                sig.exit_code(),
                params.workspace_path,
                params.execution_mode,
            );
        }

        if let Some(dl) = deadline {
            if Instant::now() >= dl {
                cancel_session_token_in_active(params.active_sessions, params.session_id).await;
                return HeadlessResultPayload::timeout(
                    params.workspace_path,
                    params.execution_mode,
                );
            }
        }

        // 1. Emit any newly arrived messages/tool_use/tool_result in stream-json mode
        if let Some((status, status_reason, messages)) = snapshot_session_state(
            params.session_repo,
            params.active_sessions,
            params.session_id,
        )
        .await
        {
            if matches!(
                status,
                SessionStatus::Busy
                    | SessionStatus::Error
                    | SessionStatus::Paused
                    | SessionStatus::Queued
                    | SessionStatus::Provisioning
            ) || messages
                .iter()
                .any(|m| m.role == "assistant" || m.role == "tool")
                || messages.iter().any(|m| m.error.is_some())
            {
                observed_turn_activity = true;
            }

            if params.output_format == OutputFormat::StreamJson {
                for msg in &messages {
                    if msg.is_streaming.unwrap_or(false) {
                        continue;
                    }
                    if seen_message_ids.insert(msg.id.clone()) {
                        for ev in stream_events_for_message(msg) {
                            let _ = write_stream_event(stream_writer, &ev);
                        }
                    }
                }
            }

            // 2. Check if the session blocked on a tool approval not covered by the initial execution mode
            let initial_approvals =
                list_pending_approvals_in(params.active_sessions, params.session_id).await;
            if !initial_approvals.is_empty() {
                // Brief settle window so parallel tool calls in the same turn batch
                // all register their approval requests before we snapshot & stop.
                tokio::time::sleep(Duration::from_millis(15)).await;
                let approvals =
                    list_pending_approvals_in(params.active_sessions, params.session_id).await;
                let approvals = if approvals.is_empty() {
                    initial_approvals
                } else {
                    approvals
                };

                let tool_call_ids: Vec<String> =
                    approvals.iter().map(|a| a.tool_call_id.clone()).collect();
                let headless_approvals: Vec<HeadlessPendingApproval> = approvals
                    .into_iter()
                    .map(HeadlessPendingApproval::from)
                    .collect();

                if params.output_format == OutputFormat::StreamJson {
                    for item in &headless_approvals {
                        let ev = HeadlessStreamEvent::from_pending_approval(item);
                        let _ = write_stream_event(stream_writer, &ev);
                    }
                }

                reject_all_pending_approvals_in_active(
                    params.active_sessions,
                    params.session_id,
                    &tool_call_ids,
                )
                .await;
                cancel_session_token_in_active(params.active_sessions, params.session_id).await;

                return HeadlessResultPayload::blocked_on_approval(
                    headless_approvals,
                    params.workspace_path,
                    params.execution_mode,
                );
            }

            // 3. Check if the session blocked on a user question (`ui__askUser`) or secret prompt
            if let Some(prompt_summary) =
                pending_user_prompt_in_active(params.active_sessions, params.session_id).await
            {
                if params.output_format == OutputFormat::StreamJson {
                    let ev = HeadlessStreamEvent::from_pending_user_prompt(&prompt_summary);
                    let _ = write_stream_event(stream_writer, &ev);
                }

                let prompt_hint = summarize_user_prompt_payload(&prompt_summary.payload);

                cancel_pending_user_prompt_in_active(params.active_sessions, params.session_id)
                    .await;
                cancel_session_token_in_active(params.active_sessions, params.session_id).await;

                return HeadlessResultPayload::blocked_on_question(
                    prompt_hint,
                    params.workspace_path,
                    params.execution_mode,
                );
            }

            // 4. Check terminal session status once turn activity has started
            if observed_turn_activity {
                match status {
                    SessionStatus::Idle => {
                        let message_values: Vec<serde_json::Value> = messages
                            .iter()
                            .rev()
                            .filter_map(|m| serde_json::to_value(m).ok())
                            .collect();
                        let output = latest_session_output(&message_values);
                        return HeadlessResultPayload::completed(
                            output,
                            params.workspace_path,
                            params.execution_mode,
                        );
                    }
                    SessionStatus::Error | SessionStatus::Paused => {
                        let err_msg = status_reason
                            .filter(|s| !s.trim().is_empty())
                            .or_else(|| extract_error_from_messages(&messages))
                            .unwrap_or_else(|| format!("Session ended with status: {:?}", status));

                        if params.output_format == OutputFormat::StreamJson {
                            let ev = HeadlessStreamEvent::Error {
                                message: err_msg.clone(),
                            };
                            let _ = write_stream_event(stream_writer, &ev);
                        }

                        return HeadlessResultPayload::error(
                            err_msg,
                            params.workspace_path,
                            params.execution_mode,
                        );
                    }
                    SessionStatus::Busy | SessionStatus::Queued | SessionStatus::Provisioning => {}
                }
            }
        } else if observed_turn_activity {
            return HeadlessResultPayload::error(
                "Ephemeral session was removed before completion".to_string(),
                params.workspace_path,
                params.execution_mode,
            );
        }

        let sleep_dur = if let Some(dl) = deadline {
            let now = Instant::now();
            if now >= dl {
                continue;
            }
            std::cmp::min(params.poll_interval, dl - now)
        } else {
            params.poll_interval
        };

        tokio::select! {
            _ = tokio::time::sleep(sleep_dur) => {}
            changed = interrupt_rx.changed() => {
                if changed.is_ok() {
                    continue;
                }
            }
        }
    }
}

/// Spawn OS signal listeners (`SIGINT` and on Unix `SIGTERM`) and report them via a watch channel.
pub fn spawn_interrupt_watcher() -> watch::Receiver<Option<InterruptSignal>> {
    let (tx, rx) = watch::channel(None);

    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm = signal(SignalKind::terminate()).ok();
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    let _ = tx.send(Some(InterruptSignal::SigInt));
                }
                _ = async {
                    if let Some(ref mut s) = sigterm {
                        let _ = s.recv().await;
                    } else {
                        std::future::pending::<()>().await;
                    }
                } => {
                    let _ = tx.send(Some(InterruptSignal::SigTerm));
                }
            }
        }
        #[cfg(not(unix))]
        {
            if tokio::signal::ctrl_c().await.is_ok() {
                let _ = tx.send(Some(InterruptSignal::SigInt));
            }
        }
    });

    rx
}
