use super::context::save_compact_context;
use super::CompactResponseOutcome;
use crate::agent::llm::types::CompactStatePhase;
use crate::agent::state::{AgentSession, CompactionResumeAction};
use crate::agent::tauri_events::{emit_compact_finished, emit_compact_request};
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::{CompactContextRecord, MessageRepository, SessionRepository};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

const MAX_COMPACTION_SUMMARY_RETRY_ATTEMPTS: u32 = 3;

pub(super) struct CompactSummaryPersistenceContext<'a> {
    pub active_sessions: &'a Arc<RwLock<HashMap<String, AgentSession>>>,
    pub app_handle: &'a AppHandle,
    pub session_repo: &'a Arc<dyn SessionRepository>,
    pub proxy_manager: &'a Arc<MCPServiceProxyManager>,
    pub session_id: &'a str,
    pub session_name: Option<String>,
    pub to_id: String,
    pub compacted_delta_count: usize,
}

pub(super) fn spawn_resume_completion(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: &str,
    log_label: &'static str,
) {
    let session_repo = session_repo.clone();
    let active_sessions = active_sessions.clone();
    let proxy_manager = proxy_manager.clone();
    let app_handle = app_handle.clone();
    let resume_session_id = session_id.to_string();

    tokio::spawn(async move {
        if let Err(error) = crate::agent::llm::request_llm_completion_with_recovery(
            &session_repo,
            &active_sessions,
            &proxy_manager,
            &app_handle,
            resume_session_id.clone(),
        )
        .await
        {
            log::error!(
                "Failed to resume {} after compaction for session {}: {}",
                log_label,
                resume_session_id,
                error
            );
        }
    });
}

fn clear_message_prompt_token_checkpoint(message: &mut Message) -> bool {
    let mut changed = false;

    if message.prompt_tokens.take().is_some() {
        changed = true;
    }

    if let Some(usage) = message
        .usage
        .as_mut()
        .and_then(|usage| usage.as_object_mut())
    {
        if usage.remove("promptTokens").is_some() {
            changed = true;
        }
    }

    changed
}

pub fn clear_message_prompt_token_checkpoint_for_testing(message: &mut Message) -> bool {
    clear_message_prompt_token_checkpoint(message)
}

async fn invalidate_retained_tail_prompt_token_checkpoints(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    to_id: &str,
) -> Result<(), String> {
    let changed_messages = {
        let active = active_sessions.read().await;
        let Some(session) = active.get(session_id) else {
            return Ok(());
        };

        let messages = session.messages.read().await;
        let Some(to_idx) = messages.iter().position(|message| message.id == to_id) else {
            return Ok(());
        };

        messages
            .iter()
            .skip(to_idx.saturating_add(1))
            .filter_map(|message| {
                let mut updated_message = message.clone();
                if clear_message_prompt_token_checkpoint(&mut updated_message) {
                    Some(updated_message)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };

    if changed_messages.is_empty() {
        return Ok(());
    }

    crate::state::get_message_repository()
        .insert_many(changed_messages.clone())
        .await
        .map_err(|error| error.to_string())?;

    let changed_by_id = changed_messages
        .into_iter()
        .map(|message| (message.id.clone(), message))
        .collect::<HashMap<_, _>>();

    let active = active_sessions.read().await;
    let Some(session) = active.get(session_id) else {
        return Ok(());
    };
    let mut messages = session.messages.write().await;
    for message in messages.iter_mut() {
        if let Some(updated_message) = changed_by_id.get(&message.id) {
            *message = updated_message.clone();
        }
    }

    Ok(())
}

pub(super) async fn retry_invalid_compact_summary_if_possible(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    validation_error: &str,
) -> Result<Option<CompactResponseOutcome>, String> {
    let compaction = {
        let active = active_sessions.read().await;
        active
            .get(session_id)
            .map(|session| session.compaction.clone())
    };
    let Some(compaction) = compaction else {
        return Ok(None);
    };

    let snapshot = compaction.snapshot().await;
    if !snapshot.blocks_workflow() {
        return Ok(None);
    }

    let Some(compact_event) = compaction.current_request().await else {
        return Ok(None);
    };

    let current_retry_count = compaction.summary_retry_count().await;
    if current_retry_count >= MAX_COMPACTION_SUMMARY_RETRY_ATTEMPTS {
        return Ok(None);
    }

    let retry_count = compaction.increment_summary_retry_count().await;
    log::warn!(
        "🔁 Retrying compaction summary request after validation failure: session={}, retry_count={}, error={}",
        session_id,
        retry_count,
        validation_error
    );
    emit_compact_request(app_handle, compact_event)?;
    Ok(Some(CompactResponseOutcome { retried: true }))
}

pub(super) async fn persist_compact_summary_and_resume(
    context: CompactSummaryPersistenceContext<'_>,
    summary: String,
) -> Result<CompactResponseOutcome, String> {
    let record = CompactContextRecord {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: context.session_id.to_string(),
        to_id: context.to_id.clone(),
        condensed_count: Some(context.compacted_delta_count),
        summary,
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    invalidate_retained_tail_prompt_token_checkpoints(
        context.active_sessions,
        context.session_id,
        &context.to_id,
    )
    .await?;
    save_compact_context(context.active_sessions, context.session_id, record).await?;

    let resume_action = {
        let active = context.active_sessions.read().await;
        if let Some(session) = active.get(context.session_id) {
            Some(session.compaction.complete_success().await)
        } else {
            None
        }
    };
    let resume_action = resume_action.unwrap_or(CompactionResumeAction::Nothing);

    log::info!(
        "📌 Compact completion decision for session {}: action={}",
        context.session_id,
        match &resume_action {
            CompactionResumeAction::Nothing => "nothing",
            CompactionResumeAction::ResumeCompletion => "resume_completion",
        }
    );
    if let Err(error) = emit_compact_finished(
        context.app_handle,
        context.session_id.to_string(),
        context.session_name,
        CompactStatePhase::Succeeded,
        None,
    ) {
        log::warn!(
            "Failed to emit compact finished state for session {} after successful compaction: {}",
            context.session_id,
            error
        );
    }

    match resume_action {
        CompactionResumeAction::ResumeCompletion => {
            log::info!(
                "▶️ Resuming blocked LLM completion after compaction for session {}",
                context.session_id
            );
            spawn_resume_completion(
                context.session_repo,
                context.active_sessions,
                context.proxy_manager,
                context.app_handle,
                context.session_id,
                "LLM completion",
            );
        }
        CompactionResumeAction::Nothing => {}
    }

    Ok(CompactResponseOutcome { retried: false })
}
