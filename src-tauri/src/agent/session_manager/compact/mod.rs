mod context;
mod fallback;
pub(crate) mod md_export;
mod persistence;
mod recovery;
mod summary;

use self::context::compacted_messages_prefix_for_to_id;
use self::fallback::{
    build_compaction_hard_fallback_artifact, build_compaction_hard_fallback_summary,
    build_compaction_raw_output_artifact, compaction_fallback_artifact_relative_path,
    compaction_raw_output_relative_path, RawCompactionOutputDump,
};
use self::persistence::{
    abort_empty_delta_compaction_and_resume, persist_compact_summary_and_resume,
    retry_invalid_compact_summary_if_possible, CompactSummaryPersistenceContext,
};
use self::summary::validate_compact_summary;
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

/// What the frontend stream exposed about the compaction call; every field is
/// absent for providers that do not report it.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactStreamMeta {
    pub finish_reason: Option<String>,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

impl CompactStreamMeta {
    fn hit_output_cap(&self) -> bool {
        matches!(self.finish_reason.as_deref(), Some("length" | "max_tokens"))
    }

    pub(super) fn describe_usage(&self) -> String {
        let part = |label: &str, value: Option<u64>| value.map(|v| format!("{label}={v}"));
        let parts = [
            part("prompt", self.prompt_tokens),
            part("completion", self.completion_tokens),
            part("total", self.total_tokens),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        if parts.is_empty() {
            "unknown".to_string()
        } else {
            parts.join(", ")
        }
    }
}

pub struct CompactResponseParams<'a> {
    pub active_sessions: &'a Arc<RwLock<HashMap<String, AgentSession>>>,
    pub app_handle: &'a AppHandle,
    pub session_repo: &'a Arc<dyn SessionRepository>,
    pub proxy_manager: &'a Arc<MCPServiceProxyManager>,
    pub session_id: &'a str,
    pub to_id: String,
    pub compacted_delta_count: usize,
    pub summary: String,
    pub stream_meta: Option<CompactStreamMeta>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactResponseOutcome {
    pub retried: bool,
}

/// Saves the pre-clamp LLM output so the compaction prompt can be tuned.
/// Logs only the path and sizes, never the content.
async fn dump_raw_compaction_output(dump: RawCompactionOutputDump<'_>) -> Option<String> {
    let relative_path = compaction_raw_output_relative_path(
        dump.session_id,
        dump.to_id,
        chrono::Utc::now().timestamp_millis(),
    );
    let text = build_compaction_raw_output_artifact(&dump);
    match WorkspaceService::workspace_write_file(
        &relative_path,
        text.as_bytes(),
        Some(dump.session_id.to_string()),
    )
    .await
    {
        Ok(()) => {
            log::warn!(
                "📝 Saved raw compaction output for session {} at '{}': raw_estimated_tokens={}, clamped_estimated_tokens={:?}, hard_limit_tokens={:?}",
                dump.session_id,
                relative_path,
                dump.raw_estimated_tokens,
                dump.clamped_estimated_tokens,
                dump.hard_limit_tokens
            );
            Some(relative_path)
        }
        Err(error) => {
            log::warn!(
                "Failed to persist raw compaction output '{}' for session {}: {}",
                relative_path,
                dump.session_id,
                error
            );
            None
        }
    }
}

/// Dumps the raw output when it was clamped or failed validation; a clean,
/// unclamped summary writes nothing.
#[allow(clippy::too_many_arguments)]
pub async fn dump_raw_compaction_output_if_needed(
    session_id: &str,
    to_id: &str,
    raw_output: &str,
    clamp: &CompactSummaryClampResult,
    validation: &Result<(), String>,
    retry_count: u32,
    model: &str,
    stream_meta: Option<&CompactStreamMeta>,
) -> Option<String> {
    let mut reasons = Vec::new();
    if stream_meta.is_some_and(CompactStreamMeta::hit_output_cap) {
        reasons.push("provider output cap reached (finish_reason=length)".to_string());
    }
    if clamp.was_clamped {
        reasons.push("clamped to hard limit".to_string());
    }
    if let Err(error) = validation {
        reasons.push(format!("validation failed: {error}"));
    }
    if reasons.is_empty() {
        return None;
    }
    let reason = reasons.join("; ");
    dump_raw_compaction_output(RawCompactionOutputDump {
        session_id,
        to_id,
        reason: &reason,
        raw_output,
        raw_estimated_tokens: clamp.original_estimated_tokens,
        clamped_estimated_tokens: Some(clamp.estimated_tokens),
        hard_limit_tokens: Some(clamp.hard_limit_tokens),
        retry_count,
        model,
        stream_meta,
    })
    .await
}

/// Dumps text received before a compaction stream failed or aborted; empty
/// partial output writes nothing.
pub async fn dump_partial_compaction_output_if_any(
    session_id: &str,
    to_id: &str,
    partial_output: &str,
    error_message: &str,
    retry_count: u32,
    model: &str,
) -> Option<String> {
    if partial_output.trim().is_empty() {
        return None;
    }
    dump_raw_compaction_output(RawCompactionOutputDump {
        session_id,
        to_id,
        reason: &format!("stream error or abort: {error_message}"),
        raw_output: partial_output,
        raw_estimated_tokens: crate::agent::llm::estimate_text_tokens(partial_output),
        clamped_estimated_tokens: None,
        hard_limit_tokens: None,
        retry_count,
        model,
        stream_meta: None,
    })
    .await
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

    // ✅ NEW: Write pre-compaction markdown for AI context recovery in fallback path
    let markdown_result = md_export::write_compaction_markdown(
        context.session_id,
        &compacted_messages,
        3, // max_epochs
    )
    .await;

    let mut summary = build_compaction_hard_fallback_summary(
        &compacted_messages,
        saved_artifact_relative_path,
        &context.to_id,
        context.compacted_delta_count,
        snapshot,
        failure_reason,
    );
    let planning_seeds =
        crate::agent::llm::completion::load_compaction_planning_seeds(context.session_id).await;
    summary = crate::agent::llm::completion::ensure_must_and_intent_sections(
        &summary,
        &compacted_messages,
        &planning_seeds,
    );

    if let Ok((relative_path, _)) = &markdown_result {
        summary.push_str(&md_export::format_transcript_recovery_suffix(relative_path));
    } else if let Err(e) = &markdown_result {
        log::warn!(
            "Failed to write pre_compaction markdown in fallback path: {}",
            e
        );
    }
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
    let epoch_path = markdown_result
        .as_ref()
        .ok()
        .map(|(relative_path, _)| relative_path.clone());
    let mut context = context;
    context.telemetry_phase =
        crate::agent::compaction_telemetry::CompactionTelemetryPhase::HardFallback;
    context.epoch_path = epoch_path;
    context.fallback_path = saved_artifact_relative_path.map(|path| path.to_string());
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
        stream_meta,
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

    // Defense in depth: prepare should already no-op empty deltas, but if a
    // response still arrives with compacted_delta_count==0, never rewrite handoff.
    if crate::agent::llm::completion::compaction::should_noop_empty_compaction_delta(
        compacted_delta_count,
    ) {
        return abort_empty_delta_compaction_and_resume(CompactSummaryPersistenceContext {
            active_sessions,
            app_handle,
            session_repo,
            proxy_manager,
            session_id,
            session_name,
            to_id,
            compacted_delta_count,
            telemetry_phase: crate::agent::compaction_telemetry::CompactionTelemetryPhase::Failed,
            epoch_path: None,
            fallback_path: None,
            raw_output_path: None,
        })
        .await;
    }

    let context_settings =
        crate::agent::llm::completion::load_context_management_settings_for_session(
            active_sessions,
            session_id,
        )
        .await;
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
    let validation_result =
        validate_compact_summary(&clamped_summary.summary, compacted_delta_count);
    let retry_count = match &compaction {
        Some(compaction) => compaction.summary_retry_count().await,
        None => 0,
    };
    let model = {
        let active = active_sessions.read().await;
        active
            .get(session_id)
            .map(|session| session.metadata.model.clone())
            .unwrap_or_else(|| "unknown".to_string())
    };
    let raw_output_path = dump_raw_compaction_output_if_needed(
        session_id,
        &to_id,
        &summary,
        &clamped_summary,
        &validation_result,
        retry_count,
        &model,
        stream_meta.as_ref(),
    )
    .await;
    if let Err(validation_error) = validation_result {
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
                telemetry_phase:
                    crate::agent::compaction_telemetry::CompactionTelemetryPhase::HardFallback,
                epoch_path: None,
                fallback_path: None,
                raw_output_path,
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

    // ✅ NEW: Write pre-compaction markdown for AI context recovery
    let markdown_result = md_export::write_compaction_markdown(
        session_id,
        &compacted_messages,
        3, // max_epochs
    )
    .await;

    let planning_seeds =
        crate::agent::llm::completion::load_compaction_planning_seeds(session_id).await;
    let mut final_summary = crate::agent::llm::completion::ensure_must_and_intent_sections(
        &clamped_summary.summary,
        &compacted_messages,
        &planning_seeds,
    );
    if let Ok((relative_path, _)) = &markdown_result {
        final_summary.push_str(&md_export::format_transcript_recovery_suffix(relative_path));
    } else if let Err(e) = &markdown_result {
        log::warn!("Failed to write pre_compaction markdown: {}", e);
    }

    let epoch_path = markdown_result
        .as_ref()
        .ok()
        .map(|(relative_path, _)| relative_path.clone());
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
            telemetry_phase:
                crate::agent::compaction_telemetry::CompactionTelemetryPhase::Succeeded,
            epoch_path,
            fallback_path: None,
            raw_output_path,
        },
        final_summary,
    )
    .await
}
