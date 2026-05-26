use crate::agent::compact_recovery::handle_compact_error_state;
use crate::agent::compaction_text::sanitize_compaction_semantic_text;
use crate::agent::events::AgentEventDispatcher;
use crate::agent::llm::types::{
    AgentRuntimeError, AgentRuntimeErrorType, CompactStateEvent, CompactStatePhase,
};
use crate::agent::state::{AgentSession, CompactionRecoveryPhase, CompactionResumeAction};
use crate::agent::tauri_events::emit_compact_finished;
use crate::mcp::types::MCPContent;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::compact_context_repository::CompactContextRepository;
use crate::repositories::message_repository::MessageRepository;
use crate::repositories::{CompactContextRecord, SessionRepository};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

const MAX_COMPACTION_RETRY_ATTEMPTS: u32 = 3;
const COMPACT_PREVIEW_MAX_CHARS: usize = 96;

pub fn should_retry_budget_related_blocking_compaction(
    snapshot: &crate::agent::state::CompactionSnapshot,
    error: &AgentRuntimeError,
) -> bool {
    if !snapshot.blocks_workflow()
        || matches!(
            snapshot.recovery_phase,
            CompactionRecoveryPhase::DegradedTools
        )
    {
        return false;
    }

    if matches!(error.error_type, AgentRuntimeErrorType::ContextLimitError) {
        return true;
    }

    let error_code = error
        .details
        .as_ref()
        .and_then(|details| details.error_code.as_deref());
    if matches!(
        error_code,
        Some("CONTEXT_LIMIT_EXCEEDED") | Some("RUST_PREFLIGHT_CONTEXT_LIMIT")
    ) {
        return true;
    }

    let normalized_message = error.display_message.to_lowercase();
    normalized_message.contains("prompt too long")
        || normalized_message.contains("exceeds max context window")
        || normalized_message.contains("maximum context length")
}

fn spawn_resume_completion(
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

pub async fn handle_compact_error_with_dispatcher(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: String,
    error: crate::agent::llm::types::AgentRuntimeError,
) -> Result<(), String> {
    let (compaction, snapshot, session_name) = {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
            (
                Some(session.compaction.clone()),
                Some(session.compaction.snapshot().await),
                session
                    .metadata
                    .name
                    .clone()
                    .unwrap_or_else(|| session_id.chars().take(8).collect::<String>()),
            )
        } else {
            (None, None, session_id.chars().take(8).collect::<String>())
        }
    };

    if let (Some(compaction), Some(snapshot)) = (compaction, snapshot) {
        if should_retry_budget_related_blocking_compaction(&snapshot, &error) {
            let transition_label = match snapshot.recovery_phase {
                CompactionRecoveryPhase::CacheAligned
                    if snapshot.retry_attempt < MAX_COMPACTION_RETRY_ATTEMPTS =>
                {
                    let retry_attempt = compaction.increment_retry_attempt().await;
                    format!("budget-retry-{}", retry_attempt)
                }
                CompactionRecoveryPhase::CacheAligned => {
                    compaction.transition_to_overflow_recovery().await;
                    "overflow-recovery".to_string()
                }
                CompactionRecoveryPhase::OverflowRecovery => {
                    compaction.transition_to_degraded_tools().await;
                    "degraded-tools".to_string()
                }
                CompactionRecoveryPhase::DegradedTools => unreachable!(),
            };
            compaction.clear_runtime_state(false).await;
            dispatcher.emit_compact_state(CompactStateEvent {
                session_id: session_id.clone(),
                session_name: Some(session_name),
                compacting: false,
                awaiting_compact: false,
                phase: CompactStatePhase::Failed,
                error: Some(error.display_message.clone()),
            })?;
            log::warn!(
                "🔁 Advancing compaction overflow recovery after budget-related failure: session={}, next_step={}",
                session_id,
                transition_label
            );
            spawn_resume_completion(
                session_repo,
                active_sessions,
                proxy_manager,
                app_handle,
                &session_id,
                "LLM completion after compaction overflow",
            );
            return Ok(());
        }

        compaction.reset_recovery_progress().await;
    }

    handle_compact_error_state(session_repo, active_sessions, dispatcher, session_id, error).await
}

pub struct CompactResponseParams<'a> {
    pub active_sessions: &'a Arc<RwLock<HashMap<String, AgentSession>>>,
    pub app_handle: &'a AppHandle,
    pub session_repo: &'a Arc<dyn SessionRepository>,
    pub proxy_manager: &'a Arc<MCPServiceProxyManager>,
    pub session_id: &'a str,
    pub from_id: String,
    pub to_id: String,
    pub summary: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactContextView {
    pub id: String,
    pub session_id: String,
    pub from_id: String,
    pub to_id: String,
    pub summary: String,
    pub created_at: i64,
    pub earlier_preview: Option<String>,
    pub latest_included_preview: Option<String>,
    pub condensed_count: Option<usize>,
}

pub async fn handle_compact_response(params: CompactResponseParams<'_>) -> Result<(), String> {
    let CompactResponseParams {
        active_sessions,
        app_handle,
        session_repo,
        proxy_manager,
        session_id,
        from_id,
        to_id,
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
    let started_at_ms = if let Some(compaction) = compaction {
        compaction.snapshot().await.started_at_ms()
    } else {
        None
    };

    log::info!(
        "✅ Compact response stored for session {}: from_id={}, to_id={}, summary_chars={}, summary_est_tokens=~{}, elapsed_ms={}",
        session_id,
        from_id,
        to_id,
        summary.len(),
        summary.len() / 4,
        started_at_ms
            .map(|value| (chrono::Utc::now().timestamp_millis() - value).to_string())
            .unwrap_or_else(|| "unknown".to_string())
    );

    let record = CompactContextRecord {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        from_id,
        to_id,
        summary,
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    save_compact_context(active_sessions, session_id, record).await?;

    let resume_action = {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(session_id) {
            Some(session.compaction.complete_success().await)
        } else {
            None
        }
    };
    let resume_action = resume_action.unwrap_or(CompactionResumeAction::Nothing);

    log::info!(
        "📌 Compact completion decision for session {}: action={}",
        session_id,
        match &resume_action {
            CompactionResumeAction::Nothing => "nothing",
            CompactionResumeAction::ResumeCompletion => "resume_completion",
        }
    );
    if let Err(error) = emit_compact_finished(
        app_handle,
        session_id.to_string(),
        session_name,
        CompactStatePhase::Succeeded,
        None,
    ) {
        log::warn!(
            "Failed to emit compact finished state for session {} after successful compaction: {}",
            session_id,
            error
        );
    }

    match resume_action {
        CompactionResumeAction::ResumeCompletion => {
            log::info!(
                "▶️ Resuming blocked LLM completion after compaction for session {}",
                session_id
            );
            spawn_resume_completion(
                session_repo,
                active_sessions,
                proxy_manager,
                app_handle,
                session_id,
                "LLM completion",
            );
        }
        CompactionResumeAction::Nothing => {}
    }

    Ok(())
}

fn truncate_preview(value: &str, max_chars: usize) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }

    format!(
        "{}…",
        normalized
            .chars()
            .take(max_chars.saturating_sub(1))
            .collect::<String>()
            .trim_end()
    )
}

fn extract_text_preview(text: &str) -> Option<String> {
    let cleaned = sanitize_compaction_semantic_text(text);
    let preview = truncate_preview(&cleaned, COMPACT_PREVIEW_MAX_CHARS);
    if preview.is_empty() {
        None
    } else {
        Some(preview)
    }
}

fn extract_message_preview(message: &Message) -> Option<String> {
    for content in &message.content {
        match content {
            MCPContent::Text { text, .. } => {
                if let Some(preview) = extract_text_preview(text) {
                    return Some(preview);
                }
            }
            MCPContent::Thinking { thinking, .. } => {
                if let Some(preview) = extract_text_preview(thinking) {
                    return Some(preview);
                }
            }
            MCPContent::ToolCall { name, .. } => {
                return Some(truncate_preview(
                    &format!("Tool call: {}", name),
                    COMPACT_PREVIEW_MAX_CHARS,
                ));
            }
            _ => {}
        }
    }

    if let Some(tool_calls) = &message.tool_calls {
        if let Some(tool_call) = tool_calls.first() {
            return Some(truncate_preview(
                &format!("Tool call: {}", tool_call.function.name),
                COMPACT_PREVIEW_MAX_CHARS,
            ));
        }
    }

    if message.role == "tool" {
        return Some("Tool result".to_string());
    }

    None
}

pub async fn get_compact_context(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> Result<Option<CompactContextRecord>, String> {
    let active = active_sessions.read().await;
    if let Some(session) = active.get(session_id) {
        let compact = session.compact_context.read().await;
        if compact.is_some() {
            return Ok((*compact).clone());
        }
    }

    let repo = crate::state::get_compact_context_repository();
    repo.get_by_session_id(session_id)
        .await
        .map_err(|e| e.to_string())
}

async fn load_boundary_messages(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    record: &CompactContextRecord,
) -> Result<HashMap<String, Message>, String> {
    let mut boundary_messages = HashMap::new();

    {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(session_id) {
            let messages = session.messages.read().await;
            for message in messages.iter() {
                if message.id == record.from_id || message.id == record.to_id {
                    boundary_messages.insert(message.id.clone(), message.clone());
                }
            }
        }
    }

    let mut missing_ids = Vec::new();
    if !boundary_messages.contains_key(&record.from_id) {
        missing_ids.push(record.from_id.clone());
    }
    if !boundary_messages.contains_key(&record.to_id) {
        missing_ids.push(record.to_id.clone());
    }

    if !missing_ids.is_empty() {
        let repo = crate::state::get_message_repository();
        let loaded = repo
            .get_by_ids(missing_ids)
            .await
            .map_err(|error| error.to_string())?;
        for message in loaded {
            boundary_messages.insert(message.id.clone(), message);
        }
    }

    Ok(boundary_messages)
}

async fn derive_condensed_count(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    record: &CompactContextRecord,
) -> Option<usize> {
    let active = active_sessions.read().await;
    let session = active.get(session_id)?;
    let messages = session.messages.read().await;
    let from_index = messages
        .iter()
        .position(|message| message.id == record.from_id)?;
    let to_index = messages
        .iter()
        .position(|message| message.id == record.to_id)?;
    (from_index <= to_index).then_some(to_index - from_index + 1)
}

fn build_compact_context_view(
    record: CompactContextRecord,
    boundary_messages: &HashMap<String, Message>,
    condensed_count: Option<usize>,
) -> CompactContextView {
    let earlier_preview = boundary_messages
        .get(&record.from_id)
        .and_then(extract_message_preview);
    let latest_included_preview = boundary_messages
        .get(&record.to_id)
        .and_then(extract_message_preview);

    CompactContextView {
        id: record.id,
        session_id: record.session_id,
        from_id: record.from_id,
        to_id: record.to_id,
        summary: record.summary,
        created_at: record.created_at,
        earlier_preview,
        latest_included_preview,
        condensed_count,
    }
}

pub async fn get_compact_context_view(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> Result<Option<CompactContextView>, String> {
    let Some(record) = get_compact_context(active_sessions, session_id).await? else {
        return Ok(None);
    };

    let boundary_messages = load_boundary_messages(active_sessions, session_id, &record).await?;
    let condensed_count = derive_condensed_count(active_sessions, session_id, &record).await;

    Ok(Some(build_compact_context_view(
        record,
        &boundary_messages,
        condensed_count,
    )))
}

pub async fn save_compact_context(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    record: CompactContextRecord,
) -> Result<(), String> {
    {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(session_id) {
            let mut compact = session.compact_context.write().await;
            *compact = Some(record.clone());
        }
    }

    let repo = crate::state::get_compact_context_repository();
    repo.upsert(&record).await.map_err(|e| e.to_string())?;

    Ok(())
}

pub async fn wait_for_compaction_to_settle(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    timeout: std::time::Duration,
) -> Result<(), String> {
    let started_at = std::time::Instant::now();

    loop {
        let is_settled = {
            let active = active_sessions.read().await;
            let session = active
                .get(session_id)
                .ok_or_else(|| format!("Session not found: {}", session_id))?;
            session.compaction.is_settled()
        };

        if is_settled {
            return Ok(());
        }

        if started_at.elapsed() >= timeout {
            return Err(format!(
                "Timed out waiting for compaction to settle for session {} after {} seconds",
                session_id,
                timeout.as_secs()
            ));
        }

        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
}
