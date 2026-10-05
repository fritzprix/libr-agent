use crate::agent::compaction_telemetry::{
    summary_sha256_hex, utc_now_rfc3339, CompactionTelemetryEvent, CompactionTelemetryPhase,
};
use crate::agent::session_manager::CompactContextView;
use crate::agent::AgentSessionManager;
use crate::services::WorkspaceService;
use crate::utils::session_id::display_session_id;
use serde::Serialize;
use std::sync::Arc;
use warp::{http::StatusCode, Rejection, Reply};

use super::helpers::{resolve_error_reply, resolve_http_session_ref};
use super::types::ErrorResponse;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactContextApiRecord {
    pub id: String,
    pub session_id: String,
    pub to_id: String,
    pub condensed_count: Option<usize>,
    pub summary: String,
    pub summary_sha256: String,
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_included_preview: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactWorkspaceArtifacts {
    pub pre_compaction_epochs: Vec<String>,
    pub fallback_artifacts: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactTelemetryDerived {
    pub compaction_count: usize,
    pub used_hard_fallback: bool,
    pub has_compact_context: bool,
    pub evidence_basis: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactContextApiResponse {
    pub schema_version: String,
    pub session_id: String,
    pub captured_at: String,
    pub compact_context: Option<CompactContextApiRecord>,
    pub events: Vec<CompactionTelemetryEvent>,
    pub workspace_artifacts: CompactWorkspaceArtifacts,
    pub derived: CompactTelemetryDerived,
}

fn infer_compact_source(
    events: &[CompactionTelemetryEvent],
    summary: &str,
) -> &'static str {
    if events
        .iter()
        .any(|event| event.phase == CompactionTelemetryPhase::HardFallback)
        || summary.contains("Compaction fallback")
        || summary.contains("deterministic compaction fallback")
    {
        "hard_fallback"
    } else {
        "llm_summary"
    }
}

fn evidence_basis(
    has_context: bool,
    events: &[CompactionTelemetryEvent],
    artifacts: &CompactWorkspaceArtifacts,
) -> String {
    if events.iter().any(|event| {
        matches!(
            event.phase,
            CompactionTelemetryPhase::Succeeded | CompactionTelemetryPhase::HardFallback
        )
    }) {
        "events".to_string()
    } else if has_context {
        "compact_context".to_string()
    } else if !artifacts.pre_compaction_epochs.is_empty()
        || !artifacts.fallback_artifacts.is_empty()
    {
        "workspace_epochs".to_string()
    } else {
        "none".to_string()
    }
}

async fn scan_workspace_artifacts(session_id: &str) -> CompactWorkspaceArtifacts {
    let mut pre_compaction_epochs = Vec::new();
    let mut fallback_artifacts = Vec::new();

    let libragent_root = match WorkspaceService::resolve_path_for_session(session_id, ".libragent").await
    {
        Ok(path) => path,
        Err(_) => {
            return CompactWorkspaceArtifacts {
                pre_compaction_epochs,
                fallback_artifacts,
            };
        }
    };

    if let Ok(mut entries) = tokio::fs::read_dir(&libragent_root).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("pre_compaction_epoch_") && name.ends_with(".md") {
                pre_compaction_epochs.push(format!(".libragent/{name}"));
            }
        }
    }
    pre_compaction_epochs.sort();

    let fallback_dir = libragent_root.join("tool-results").join("compaction");
    if let Ok(mut entries) = tokio::fs::read_dir(&fallback_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("fallback-") {
                fallback_artifacts.push(format!(".libragent/tool-results/compaction/{name}"));
            }
        }
    }
    fallback_artifacts.sort();

    CompactWorkspaceArtifacts {
        pre_compaction_epochs,
        fallback_artifacts,
    }
}

fn map_compact_context_view(
    view: CompactContextView,
    events: &[CompactionTelemetryEvent],
) -> CompactContextApiRecord {
    let source = infer_compact_source(events, &view.summary).to_string();
    CompactContextApiRecord {
        id: view.id,
        session_id: display_session_id(&view.session_id),
        to_id: view.to_id,
        condensed_count: view.condensed_count,
        summary_sha256: summary_sha256_hex(&view.summary),
        summary: view.summary,
        created_at: view.created_at,
        latest_included_preview: view.latest_included_preview,
        source,
    }
}

pub async fn build_compact_context_api_response(
    manager: &AgentSessionManager,
    session_id: &str,
) -> Result<CompactContextApiResponse, String> {
    let view = manager.get_compact_context_view(session_id).await?;
    let events = {
        let active = manager.active_sessions_arc();
        let active = active.read().await;
        match active.get(session_id) {
            Some(session) => session.compaction.telemetry_events().await,
            None => Vec::new(),
        }
    };
    let workspace_artifacts = scan_workspace_artifacts(session_id).await;
    let compact_context = view.map(|view| map_compact_context_view(view, &events));
    let has_compact_context = compact_context.is_some();
    let used_hard_fallback = events
        .iter()
        .any(|event| event.phase == CompactionTelemetryPhase::HardFallback)
        || compact_context
            .as_ref()
            .is_some_and(|context| context.source == "hard_fallback");
    let compaction_count = if has_compact_context {
        1.max(
            events
                .iter()
                .filter(|event| {
                    matches!(
                        event.phase,
                        CompactionTelemetryPhase::Succeeded
                            | CompactionTelemetryPhase::HardFallback
                    )
                })
                .count(),
        )
    } else {
        events
            .iter()
            .filter(|event| {
                matches!(
                    event.phase,
                    CompactionTelemetryPhase::Succeeded | CompactionTelemetryPhase::HardFallback
                )
            })
            .count()
    };
    let basis = evidence_basis(has_compact_context, &events, &workspace_artifacts);

    Ok(CompactContextApiResponse {
        schema_version: "compaction-telemetry/v1".to_string(),
        session_id: display_session_id(session_id),
        captured_at: utc_now_rfc3339(),
        compact_context,
        events,
        workspace_artifacts,
        derived: CompactTelemetryDerived {
            compaction_count,
            used_hard_fallback,
            has_compact_context,
            evidence_basis: basis,
        },
    })
}

/// GET /api/sessions/:id/compact-context
pub async fn get_compact_context(
    id: String,
    manager: Arc<AgentSessionManager>,
) -> Result<impl Reply, Rejection> {
    let id = match resolve_http_session_ref(&id).await {
        Ok(id) => id,
        Err((status, error)) => return Ok(resolve_error_reply(status, error)),
    };

    match manager.get_session(&id).await {
        Ok(None) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: "Session not found".to_string(),
                }),
                StatusCode::NOT_FOUND,
            ));
        }
        Err(e) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse { error: e }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
        Ok(Some(_)) => {}
    }

    match build_compact_context_api_response(&manager, &id).await {
        Ok(payload) => Ok(warp::reply::with_status(
            warp::reply::json(&payload),
            StatusCode::OK,
        )),
        Err(e) => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse { error: e }),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}
