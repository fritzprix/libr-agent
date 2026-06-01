mod context;
mod fallback;
mod persistence;
mod recovery;
mod summary;

use self::context::compacted_messages_prefix_for_to_id;
use self::fallback::{
    build_compaction_hard_fallback_artifact, build_compaction_hard_fallback_summary,
    compaction_fallback_artifact_relative_path,
};
use self::persistence::{
    persist_compact_summary_and_resume, retry_invalid_compact_summary_if_possible,
    CompactSummaryPersistenceContext,
};
use self::summary::validate_compact_summary;
use crate::agent::llm::completion::load_context_management_settings;
use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::SessionRepository;
use crate::services::WorkspaceService;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

pub use self::context::{
    get_compact_context, get_compact_context_view, save_compact_context,
    wait_for_compaction_to_settle, CompactContextView,
};
pub use self::fallback::{
    build_compaction_hard_fallback_summary_for_testing,
    compaction_fallback_artifact_relative_path_for_testing,
};
pub use self::persistence::clear_message_prompt_token_checkpoint_for_testing;
pub use self::recovery::{
    handle_compact_error_with_dispatcher, should_retry_budget_related_blocking_compaction,
};
pub use self::summary::{
    clamp_compact_summary_to_context_limit, validate_compact_summary_for_testing,
    CompactSummaryClampResult,
};

pub struct CompactResponseParams<'a> {
    pub active_sessions: &'a Arc<RwLock<HashMap<String, AgentSession>>>,
    pub app_handle: &'a AppHandle,
    pub session_repo: &'a Arc<dyn SessionRepository>,
    pub proxy_manager: &'a Arc<MCPServiceProxyManager>,
    pub session_id: &'a str,
    pub to_id: String,
    pub compacted_delta_count: usize,
    pub summary: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactResponseOutcome {
    pub retried: bool,
}

async fn complete_compaction_with_hard_fallback(
    context: CompactSummaryPersistenceContext<'_>,
    failure_reason: &str,
    snapshot: Option<&crate::agent::state::CompactionSnapshot>,
) -> Result<CompactResponseOutcome, String> {
    let compacted_messages = compacted_messages_prefix_for_to_id(
        context.active_sessions,
        context.session_id,
        &context.to_id,
    )
    .await;
    let created_at = chrono::Utc::now().timestamp_millis();
    let artifact_relative_path =
        compaction_fallback_artifact_relative_path(context.session_id, &context.to_id, created_at);
    let artifact_text = build_compaction_hard_fallback_artifact(
        context.session_id,
        &context.to_id,
        context.compacted_delta_count,
        &compacted_messages,
        &artifact_relative_path,
        snapshot,
        failure_reason,
    );
    let artifact_write_result = WorkspaceService::workspace_write_file(
        &artifact_relative_path,
        artifact_text.as_bytes(),
        Some(context.session_id.to_string()),
    )
    .await;
    let saved_artifact_relative_path = match artifact_write_result {
        Ok(()) => Some(artifact_relative_path.as_str()),
        Err(error) => {
            log::warn!(
                "Failed to persist compaction fallback artifact '{}' for session {}: {}",
                artifact_relative_path,
                context.session_id,
                error
            );
            None
        }
    };

    let summary = build_compaction_hard_fallback_summary(
        &compacted_messages,
        saved_artifact_relative_path,
        &context.to_id,
        context.compacted_delta_count,
        snapshot,
        failure_reason,
    );
    if saved_artifact_relative_path.is_some() {
        log::warn!(
            "🧯 Stored deterministic compaction fallback for session {} at '{}' after failure: {}",
            context.session_id,
            artifact_relative_path,
            failure_reason
        );
    } else {
        log::warn!(
            "🧯 Stored deterministic compaction fallback summary without artifact for session {} after failure: {}",
            context.session_id,
            failure_reason
        );
    }
    persist_compact_summary_and_resume(context, summary).await
}

pub async fn handle_compact_response(
    params: CompactResponseParams<'_>,
) -> Result<CompactResponseOutcome, String> {
    let CompactResponseParams {
        active_sessions,
        app_handle,
        session_repo,
        proxy_manager,
        session_id,
        to_id,
        compacted_delta_count,
        summary,
    } = params;
    let (compaction, session_name) = {
        let active = active_sessions.read().await;
        active.get(session_id).map_or((None, None), |session| {
            (
                Some(session.compaction.clone()),
                session.metadata.name.clone(),
            )
        })
    };
    let started_at_ms = if let Some(compaction) = &compaction {
        compaction.snapshot().await.started_at_ms()
    } else {
        None
    };
    if let Some(compaction) = &compaction {
        let Some(current_request) = compaction.current_request().await else {
            log::warn!(
                "Ignoring stale compaction response for session {}: no active request for to_id={}",
                session_id,
                to_id
            );
            return Ok(CompactResponseOutcome { retried: false });
        };
        if current_request.to_id != to_id {
            log::warn!(
                "Ignoring stale compaction response for session {}: response_to_id={}, active_to_id={}",
                session_id,
                to_id,
                current_request.to_id
            );
            return Ok(CompactResponseOutcome { retried: false });
        }
    } else {
        log::warn!(
            "Ignoring stale compaction response for session {}: no active compaction controller for to_id={}",
            session_id,
            to_id
        );
        return Ok(CompactResponseOutcome { retried: false });
    }

    let context_settings = load_context_management_settings().await;
    let compacted_messages =
        compacted_messages_prefix_for_to_id(active_sessions, session_id, &to_id).await;
    let clamped_summary = summary::clamp_compact_summary_to_context_limit(
        session_id,
        &summary,
        &compacted_messages,
        context_settings.max_input_context(),
    );
    if clamped_summary.was_clamped {
        log::warn!(
            "✂️ Clamped oversized compact summary in backend: session={}, hard_limit_tokens={}, original_estimated_tokens={}, clamped_estimated_tokens={}",
            session_id,
            clamped_summary.hard_limit_tokens,
            clamped_summary.original_estimated_tokens,
            clamped_summary.estimated_tokens
        );
    }
    if let Err(validation_error) =
        validate_compact_summary(&clamped_summary.summary, compacted_delta_count)
    {
        if let Some(outcome) = retry_invalid_compact_summary_if_possible(
            active_sessions,
            app_handle,
            session_id,
            &validation_error,
        )
        .await?
        {
            return Ok(outcome);
        }
        let fallback_snapshot = if let Some(compaction) = &compaction {
            Some(compaction.snapshot().await)
        } else {
            None
        };
        return complete_compaction_with_hard_fallback(
            CompactSummaryPersistenceContext {
                active_sessions,
                app_handle,
                session_repo,
                proxy_manager,
                session_id,
                session_name,
                to_id,
                compacted_delta_count,
            },
            &validation_error,
            fallback_snapshot.as_ref(),
        )
        .await;
    }

    log::info!(
        "✅ Compact response stored for session {}: to_id={}, compacted_delta_count={}, summary_chars={}, summary_est_tokens={}, elapsed_ms={}",
        session_id,
        to_id,
        compacted_delta_count,
        clamped_summary.summary.len(),
        clamped_summary.estimated_tokens,
        started_at_ms
            .map(|value| (chrono::Utc::now().timestamp_millis() - value).to_string())
            .unwrap_or_else(|| "unknown".to_string())
    );

    persist_compact_summary_and_resume(
        CompactSummaryPersistenceContext {
            active_sessions,
            app_handle,
            session_repo,
            proxy_manager,
            session_id,
            session_name,
            to_id,
            compacted_delta_count,
        },
        clamped_summary.summary,
    )
    .await
}
