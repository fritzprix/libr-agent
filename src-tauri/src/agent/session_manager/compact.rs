use crate::agent::compact_recovery::handle_compact_error_state;
use crate::agent::compaction_text::sanitize_compaction_semantic_text;
use crate::agent::events::AgentEventDispatcher;
use crate::agent::llm::completion::{
    build_compact_summary_message_for_messages, build_compaction_preservation_hints,
    load_context_management_settings, summarize_recent_tool_calls,
};
use crate::agent::llm::types::{
    AgentRuntimeError, AgentRuntimeErrorType, CompactStateEvent, CompactStatePhase,
};
use crate::agent::state::{
    AgentSession, CompactionRecoveryPhase, CompactionResumeAction, CompactionSnapshot,
};
use crate::agent::tauri_events::{emit_compact_finished, emit_compact_request};
use crate::mcp::types::MCPContent;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::compact_context_repository::CompactContextRepository;
use crate::repositories::message_repository::MessageRepository;
use crate::repositories::{CompactContextRecord, SessionRepository};
use crate::services::WorkspaceService;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

const MAX_COMPACTION_RETRY_ATTEMPTS: u32 = 3;
const MAX_COMPACTION_SUMMARY_RETRY_ATTEMPTS: u32 = 3;
const COMPACT_PREVIEW_MAX_CHARS: usize = 96;
const COMPACTION_SUMMARY_HARD_LIMIT_RATIO: usize = 10;
const COMPACTION_SUMMARY_TRUNCATION_SUFFIX: &str = "…";
const COMPACTION_FALLBACK_ARTIFACT_DIR: &str = ".libragent/tool-results/compaction";
const COMPACTION_FALLBACK_ARTIFACT_MESSAGE_LIMIT: usize = 12;
const COMPACTION_FALLBACK_ACTIVE_REQUEST_LIMIT: usize = 4;
const COMPACTION_FALLBACK_REFERENCE_LIMIT: usize = 5;
const COMPACTION_FALLBACK_MESSAGE_TEXT_LIMIT: usize = 1_200;
const COMPACTION_FALLBACK_NOTE: &str =
    "Auto-saved via fallback summary after compaction retries ran out.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactSummaryClampResult {
    pub summary: String,
    pub hard_limit_tokens: usize,
    pub estimated_tokens: usize,
    pub original_estimated_tokens: usize,
    pub was_clamped: bool,
}

fn compact_summary_hard_limit_tokens(max_input_context: usize) -> usize {
    std::cmp::max(1, max_input_context / COMPACTION_SUMMARY_HARD_LIMIT_RATIO)
}

fn estimate_wrapped_compact_summary_tokens(
    session_id: &str,
    summary: &str,
    compacted_messages: &[Message],
) -> usize {
    let summary_message =
        build_compact_summary_message_for_messages(session_id, summary, compacted_messages, 0);
    crate::agent::llm::estimate_tokens_bpe(&summary_message)
}

fn truncate_summary_prefix(summary: &str, max_chars: usize) -> String {
    let total_chars = summary.chars().count();
    let prefix = summary.chars().take(max_chars).collect::<String>();
    let trimmed = prefix.trim_end();
    if max_chars < total_chars {
        format!("{}{}", trimmed, COMPACTION_SUMMARY_TRUNCATION_SUFFIX)
    } else {
        trimmed.to_string()
    }
}

pub fn clamp_compact_summary_to_context_limit(
    session_id: &str,
    summary: &str,
    compacted_messages: &[Message],
    max_input_context: usize,
) -> CompactSummaryClampResult {
    let normalized_summary = summary.trim();
    let hard_limit_tokens = compact_summary_hard_limit_tokens(max_input_context);
    let original_estimated_tokens =
        estimate_wrapped_compact_summary_tokens(session_id, normalized_summary, compacted_messages);

    if original_estimated_tokens <= hard_limit_tokens {
        return CompactSummaryClampResult {
            summary: normalized_summary.to_string(),
            hard_limit_tokens,
            estimated_tokens: original_estimated_tokens,
            original_estimated_tokens,
            was_clamped: false,
        };
    }

    let total_chars = normalized_summary.chars().count();
    let mut low = 0usize;
    let mut high = total_chars;
    let mut best_summary = String::new();
    let mut best_estimated_tokens =
        estimate_wrapped_compact_summary_tokens(session_id, &best_summary, compacted_messages);

    while low <= high {
        let mid = low + ((high - low) / 2);
        let candidate = truncate_summary_prefix(normalized_summary, mid);
        let candidate_estimated_tokens =
            estimate_wrapped_compact_summary_tokens(session_id, &candidate, compacted_messages);

        if candidate_estimated_tokens <= hard_limit_tokens {
            best_summary = candidate;
            best_estimated_tokens = candidate_estimated_tokens;
            low = mid + 1;
        } else if mid == 0 {
            break;
        } else {
            high = mid - 1;
        }
    }

    CompactSummaryClampResult {
        summary: best_summary,
        hard_limit_tokens,
        estimated_tokens: best_estimated_tokens,
        original_estimated_tokens,
        was_clamped: true,
    }
}

async fn compacted_messages_prefix_for_to_id(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    to_id: &str,
) -> Vec<Message> {
    let active = active_sessions.read().await;
    let Some(session) = active.get(session_id) else {
        return Vec::new();
    };
    let session_messages = session.messages.read().await;
    let Some(to_idx) = session_messages
        .iter()
        .position(|message| message.id == to_id)
    else {
        return Vec::new();
    };
    session_messages.iter().take(to_idx + 1).cloned().collect()
}

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
        || normalized_message.contains("empty response from streamchat")
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
            compaction.clear_in_flight_state(false).await;
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

        if snapshot.blocks_workflow() {
            if let Some(compact_event) = compaction.current_request().await {
                return complete_compaction_with_hard_fallback(
                    CompactSummaryPersistenceContext {
                        active_sessions,
                        app_handle,
                        session_repo,
                        proxy_manager,
                        session_id: &session_id,
                        session_name: Some(session_name),
                        to_id: compact_event.to_id.clone(),
                        compacted_delta_count: compact_event.compacted_delta_count,
                    },
                    &error.display_message,
                    Some(&snapshot),
                )
                .await
                .map(|_| ());
            }
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
    pub to_id: String,
    pub compacted_delta_count: usize,
    pub summary: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactContextView {
    pub id: String,
    pub session_id: String,
    pub to_id: String,
    pub summary: String,
    pub created_at: i64,
    pub latest_included_preview: Option<String>,
    pub condensed_count: Option<usize>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactResponseOutcome {
    pub retried: bool,
}

fn minimum_compact_summary_chars(compacted_delta_count: usize) -> usize {
    match compacted_delta_count {
        0..=2 => 32,
        3..=5 => 64,
        _ => 96,
    }
}

fn validate_compact_summary(summary: &str, compacted_delta_count: usize) -> Result<(), String> {
    let normalized = summary.trim();
    if normalized.is_empty() {
        return Err("Compaction summary was empty.".to_string());
    }

    let min_chars = minimum_compact_summary_chars(compacted_delta_count);
    let summary_chars = normalized.chars().count();
    if summary_chars < min_chars {
        return Err(format!(
            "Compaction summary was too short: got {} chars, expected at least {}.",
            summary_chars, min_chars
        ));
    }

    Ok(())
}

pub fn validate_compact_summary_for_testing(
    summary: &str,
    compacted_delta_count: usize,
) -> Result<(), String> {
    validate_compact_summary(summary, compacted_delta_count)
}

fn push_bullet_section(lines: &mut Vec<String>, heading: &str, bullets: &[String]) {
    lines.push(format!("### {}", heading));
    if bullets.is_empty() {
        lines.push("- None".to_string());
    } else {
        lines.extend(bullets.iter().map(|bullet| format!("- {}", bullet)));
    }
    lines.push(String::new());
}

fn truncate_for_fallback(text: &str, max_chars: usize) -> String {
    let normalized = sanitize_compaction_semantic_text(text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }

    format!(
        "{}{}",
        normalized
            .chars()
            .take(max_chars.saturating_sub(COMPACTION_SUMMARY_TRUNCATION_SUFFIX.chars().count()))
            .collect::<String>()
            .trim_end(),
        COMPACTION_SUMMARY_TRUNCATION_SUFFIX
    )
}

fn sanitize_fallback_identifier(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|char| {
            if char.is_ascii_alphanumeric() || matches!(char, '-' | '_') {
                char
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if sanitized.is_empty() {
        "item".to_string()
    } else {
        sanitized.chars().take(48).collect()
    }
}

fn compaction_fallback_artifact_relative_path(
    session_id: &str,
    to_id: &str,
    created_at: i64,
) -> String {
    format!(
        "{}/fallback-{}-{}-{}.md",
        COMPACTION_FALLBACK_ARTIFACT_DIR,
        created_at,
        sanitize_fallback_identifier(&session_id.chars().take(8).collect::<String>()),
        sanitize_fallback_identifier(&to_id.chars().take(12).collect::<String>())
    )
}

pub fn compaction_fallback_artifact_relative_path_for_testing(
    session_id: &str,
    to_id: &str,
    created_at: i64,
) -> String {
    compaction_fallback_artifact_relative_path(session_id, to_id, created_at)
}

fn fallback_message_excerpt(message: &Message) -> Option<String> {
    let mut fragments = Vec::new();

    for content in &message.content {
        match content {
            MCPContent::Text { text, .. } => {
                let snippet = truncate_for_fallback(text, COMPACTION_FALLBACK_MESSAGE_TEXT_LIMIT);
                if !snippet.is_empty() {
                    fragments.push(snippet);
                }
            }
            MCPContent::Thinking { thinking, .. } => {
                let snippet =
                    truncate_for_fallback(thinking, COMPACTION_FALLBACK_MESSAGE_TEXT_LIMIT);
                if !snippet.is_empty() {
                    fragments.push(format!("Thinking: {}", snippet));
                }
            }
            MCPContent::ToolCall { name, .. } => fragments.push(format!("Tool call: {}", name)),
            _ => {}
        }
    }

    if fragments.is_empty() {
        if let Some(tool_calls) = &message.tool_calls {
            for tool_call in tool_calls {
                fragments.push(format!("Tool call: {}", tool_call.function.name));
            }
        }
    }

    if fragments.is_empty() && message.role == "tool" {
        fragments.push("Tool result".to_string());
    }

    if fragments.is_empty() {
        None
    } else {
        Some(fragments.join("\n"))
    }
}

fn format_recovery_phase_label(recovery_phase: CompactionRecoveryPhase) -> &'static str {
    match recovery_phase {
        CompactionRecoveryPhase::CacheAligned => "cache-aligned",
        CompactionRecoveryPhase::OverflowRecovery => "overflow-recovery",
        CompactionRecoveryPhase::DegradedTools => "degraded-tools",
    }
}

fn build_compaction_hard_fallback_summary(
    compacted_messages: &[Message],
    artifact_relative_path: Option<&str>,
    to_id: &str,
    compacted_delta_count: usize,
    snapshot: Option<&CompactionSnapshot>,
    failure_reason: &str,
) -> String {
    let hints = build_compaction_preservation_hints(compacted_messages);
    let active_request = hints
        .active_request
        .into_iter()
        .take(COMPACTION_FALLBACK_ACTIVE_REQUEST_LIMIT)
        .collect::<Vec<_>>();
    let required_references = hints
        .required_references
        .into_iter()
        .take(COMPACTION_FALLBACK_REFERENCE_LIMIT)
        .collect::<Vec<_>>();
    let latest_preview = compacted_messages
        .last()
        .and_then(extract_message_preview)
        .unwrap_or_else(|| "Newest compacted message".to_string());
    let mut current_state = vec![
        COMPACTION_FALLBACK_NOTE.to_string(),
        format!(
            "Compacted {} messages up to `{}`.",
            compacted_delta_count,
            to_id.chars().take(12).collect::<String>()
        ),
        format!("Latest included: {}", latest_preview),
    ];
    if let Some(snapshot) = snapshot {
        current_state.push(format!(
            "Recovery path stopped at `{}` after {} summary retries.",
            format_recovery_phase_label(snapshot.recovery_phase),
            snapshot.summary_retry_count
        ));
    }
    current_state.push(format!(
        "Fallback reason: {}",
        truncate_for_fallback(failure_reason, 180)
    ));

    let recent_tool_results = summarize_recent_tool_calls(compacted_messages);
    let mut next_actions = vec!["Resume from the latest active request above.".to_string()];
    let mut fallback_note = Vec::new();
    if let Some(artifact_relative_path) = artifact_relative_path {
        next_actions.push(format!(
            "Open `{}` if you need the saved excerpts and recent detail.",
            artifact_relative_path
        ));
        fallback_note.push(format!(
            "Detailed fallback context saved to `{}`.",
            artifact_relative_path
        ));
        fallback_note.push(
            "Use that file when the summary feels too compressed or you need exact recent excerpts."
                .to_string(),
        );
    } else {
        fallback_note.push(
            "Detailed fallback context could not be written, so this summary is the only saved handoff."
                .to_string(),
        );
    }

    let mut lines = Vec::new();
    push_bullet_section(&mut lines, "Active Request", &active_request);
    push_bullet_section(&mut lines, "Required References", &required_references);
    push_bullet_section(&mut lines, "Current State", &current_state);
    push_bullet_section(&mut lines, "Recent Tool Results", &recent_tool_results);
    push_bullet_section(&mut lines, "Next Actions", &next_actions);
    push_bullet_section(&mut lines, "Fallback Note", &fallback_note);
    lines.join("\n").trim().to_string()
}

pub fn build_compaction_hard_fallback_summary_for_testing(
    compacted_messages: &[Message],
    artifact_relative_path: &str,
    to_id: &str,
    compacted_delta_count: usize,
    recovery_phase: CompactionRecoveryPhase,
    summary_retry_count: u32,
    failure_reason: &str,
) -> String {
    build_compaction_hard_fallback_summary(
        compacted_messages,
        Some(artifact_relative_path),
        to_id,
        compacted_delta_count,
        Some(&CompactionSnapshot {
            phase: crate::agent::state::CompactionPhase::Idle,
            last_compacted_tail_id: None,
            retry_attempt: 0,
            recovery_phase,
            summary_retry_count,
        }),
        failure_reason,
    )
}

struct CompactSummaryPersistenceContext<'a> {
    active_sessions: &'a Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &'a AppHandle,
    session_repo: &'a Arc<dyn SessionRepository>,
    proxy_manager: &'a Arc<MCPServiceProxyManager>,
    session_id: &'a str,
    session_name: Option<String>,
    to_id: String,
    compacted_delta_count: usize,
}

fn build_compaction_hard_fallback_artifact(
    session_id: &str,
    to_id: &str,
    compacted_delta_count: usize,
    compacted_messages: &[Message],
    artifact_relative_path: &str,
    snapshot: Option<&CompactionSnapshot>,
    failure_reason: &str,
) -> String {
    let hints = build_compaction_preservation_hints(compacted_messages);
    let mut lines = vec![
        "# Compaction fallback artifact".to_string(),
        String::new(),
        format!("- Session: `{}`", session_id),
        format!("- Boundary message: `{}`", to_id),
        format!("- Condensed messages: {}", compacted_delta_count),
        format!("- Saved artifact: `{}`", artifact_relative_path),
        format!(
            "- Trigger: {}",
            truncate_for_fallback(failure_reason, COMPACTION_FALLBACK_MESSAGE_TEXT_LIMIT)
        ),
    ];

    if let Some(snapshot) = snapshot {
        lines.push(format!(
            "- Recovery phase: `{}`",
            format_recovery_phase_label(snapshot.recovery_phase)
        ));
        lines.push(format!(
            "- Summary retries used: {}",
            snapshot.summary_retry_count
        ));
        lines.push(format!(
            "- Budget retry attempt: {}",
            snapshot.retry_attempt
        ));
    }

    lines.push(String::new());
    push_bullet_section(
        &mut lines,
        "Active Request Seeds",
        &hints
            .active_request
            .into_iter()
            .take(COMPACTION_FALLBACK_ACTIVE_REQUEST_LIMIT)
            .collect::<Vec<_>>(),
    );
    push_bullet_section(
        &mut lines,
        "Required References",
        &hints
            .required_references
            .into_iter()
            .take(COMPACTION_FALLBACK_REFERENCE_LIMIT)
            .collect::<Vec<_>>(),
    );
    push_bullet_section(
        &mut lines,
        "Recent Tool Results",
        &summarize_recent_tool_calls(compacted_messages),
    );

    lines.push("## Recent Message Excerpts".to_string());
    let excerpt_messages = compacted_messages
        .iter()
        .rev()
        .take(COMPACTION_FALLBACK_ARTIFACT_MESSAGE_LIMIT)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();

    for message in excerpt_messages {
        let source = message
            .source
            .as_ref()
            .map(|source| source.as_str().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        lines.push(format!(
            "### {} `{}` ({})",
            message.role, message.id, source
        ));
        lines
            .push(fallback_message_excerpt(message).unwrap_or_else(|| {
                "No text excerpt was recoverable for this message.".to_string()
            }));
        lines.push(String::new());
    }

    lines.join("\n").trim().to_string()
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

async fn retry_invalid_compact_summary_if_possible(
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

async fn persist_compact_summary_and_resume(
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

async fn complete_compaction_with_hard_fallback(
    context: CompactSummaryPersistenceContext<'_>,
    failure_reason: &str,
    snapshot: Option<&CompactionSnapshot>,
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
    let clamped_summary = clamp_compact_summary_to_context_limit(
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
                if message.id == record.to_id {
                    boundary_messages.insert(message.id.clone(), message.clone());
                }
            }
        }
    }

    let mut missing_ids = Vec::new();
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

fn build_compact_context_view(
    record: CompactContextRecord,
    boundary_messages: &HashMap<String, Message>,
) -> CompactContextView {
    let latest_included_preview = boundary_messages
        .get(&record.to_id)
        .and_then(extract_message_preview);

    CompactContextView {
        id: record.id,
        session_id: record.session_id,
        to_id: record.to_id,
        summary: record.summary,
        created_at: record.created_at,
        latest_included_preview,
        condensed_count: record.condensed_count,
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
    Ok(Some(build_compact_context_view(record, &boundary_messages)))
}

pub async fn save_compact_context(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    record: CompactContextRecord,
) -> Result<(), String> {
    let repo = crate::state::get_compact_context_repository();
    repo.upsert(&record).await.map_err(|e| e.to_string())?;

    {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(session_id) {
            let mut compact = session.compact_context.write().await;
            *compact = Some(record.clone());
        }
    }

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
