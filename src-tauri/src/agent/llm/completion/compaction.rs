use crate::agent::state::AgentSession;
use crate::agent::state::DeferredWorkflowStep;
use crate::mcp::types::MCPContent;
use crate::models::chat::{Message, MessageSource};
use crate::repositories::CompactContextRecord;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

use super::context::{load_context_management_settings, uses_compaction_strategy};
use crate::agent::llm::types::{CompactRequest, CompactionParentRequest};
use crate::agent::tauri_events::{emit_compact_request, emit_compact_started};

pub fn should_trigger_background_compaction(
    current_tokens: usize,
    safe_input_token_limit: usize,
    context_strategy: &str,
) -> bool {
    uses_compaction_strategy(context_strategy)
        && current_tokens
            > crate::agent::llm::token_utils::calculate_compact_threshold(safe_input_token_limit)
}

pub fn should_trigger_post_response_compaction(
    usage_total_tokens: usize,
    safe_input_token_limit: usize,
    context_strategy: &str,
) -> bool {
    should_trigger_background_compaction(
        usage_total_tokens,
        safe_input_token_limit,
        context_strategy,
    )
}

async fn resolve_parent_request(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    explicit_parent_request: Option<CompactionParentRequest>,
) -> Option<CompactionParentRequest> {
    if explicit_parent_request.is_some() {
        return explicit_parent_request;
    }

    let request_handle = {
        let active = active_sessions.read().await;
        active
            .get(session_id)
            .map(|session| session.last_completion_request.clone())
    }?;

    let request = request_handle.read().await.clone();
    request
}

pub(crate) struct BackgroundCompactionHandles {
    pub(crate) compact_in_flight_arc: Arc<AtomicBool>,
    pub(crate) last_compacted_tail_id_arc: Arc<RwLock<Option<String>>>,
}

struct InFlightResetGuard {
    flag: Arc<AtomicBool>,
    armed: bool,
}

impl InFlightResetGuard {
    fn new(flag: Arc<AtomicBool>) -> Self {
        Self { flag, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for InFlightResetGuard {
    fn drop(&mut self) {
        if self.armed {
            self.flag.store(false, Ordering::SeqCst);
        }
    }
}

fn build_incremental_compact_summary_message(
    session_id: &str,
    summary: &str,
    compacted_messages: &[Message],
    created_at: i64,
) -> Message {
    super::request::build_compact_summary_message_for_messages(
        session_id,
        summary,
        compacted_messages,
        created_at,
    )
}

const COMPACTION_SECTION_SCHEMA: &str = "Use EXACTLY these sections in this order:\n\
1. Stable Context\n\
2. Key Decisions & Constraints\n\
3. Active Request\n\
4. Required References\n\
5. Current State\n\
6. Recent Tool Results\n\
7. Next Actions";

const COMPACTION_RULES: &[&str] = &[
    "Use terse bullet points, not prose paragraphs.",
    "Prefer noun phrases and short action statements.",
    "Minimize adjectives, adverbs, filler, and repetition.",
    "Do not restate obvious chronology or narration.",
    "Preserve durable facts, decisions, constraints, user preferences, and unresolved work.",
    "If the messages include an unresolved external request, you MUST record it in Active Request.",
    "If the unresolved request depends on earlier discovered context, you MUST record the minimum file paths, symbol names, entities, or identifiers needed to execute it in Required References.",
    "Keep volatile/recent details in Current State, Recent Tool Results, or Next Actions.",
    "Do not paraphrase away concrete requirements such as technology choices, limits, file targets, or requested deliverables when they are still relevant to pending work.",
    "Do not replace exact file paths, symbol names, or user-named targets with vague descriptions when they are still operationally relevant.",
    "If a detail is recoverable from recent tool results, do not duplicate it in stable sections.",
];

const COMPACTION_SECTION_LIMITS: &[&str] = &[
    "Stable Context: at most 6 bullets",
    "Key Decisions & Constraints: at most 6 bullets",
    "Active Request: at most 4 bullets",
    "Required References: at most 5 bullets",
    "Current State: at most 6 bullets",
    "Recent Tool Results: at most 5 bullets",
    "Next Actions: at most 5 bullets",
    "Each bullet should be one short sentence or fragment.",
];

const COMPACTION_OUTPUT_CONSTRAINT: &str =
    "IMPORTANT: Do NOT attempt to use tools in this response. Just output plain text.";

const INCREMENTAL_COMPACTION_RESIDUAL_PREFIX: &str = "The first message is a previously accumulated compact summary that represents ALL earlier conversation history.\n\n\
CRITICAL RESIDUAL RULE: Every fact, decision, action, and context item recorded in that prior summary MUST be preserved verbatim or re-stated with equivalent fidelity in your new summary. \
Do NOT drop durable information from the prior summary. \
You may tighten wording, remove duplication, and relocate items into the required sections, but you must preserve the same meaning and operational usefulness. \
Your new summary = (prior summary, preserved faithfully and reorganized if needed) + (new messages, summarised under the same schema).";

const ACTIVE_REQUEST_BULLET_LIMIT: usize = 4;
const REQUIRED_REFERENCE_BULLET_LIMIT: usize = 5;
const REFERENCE_CONTEXT_WINDOW_MESSAGES: usize = 8;
const INSTRUCTION_HINT_TEXT_LIMIT: usize = 320;

static BACKTICK_REFERENCE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"`([^`\r\n]+)`").expect("valid backtick regex"));
static PATH_REFERENCE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)\b(?:[a-z]:[\\/]|\.{1,2}[\\/]|[^\\/\s]+[\\/])[\w./\\-]*\.[a-z0-9]{1,12}\b"#)
        .expect("valid path regex")
});
static SYMBOL_REFERENCE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"\b(?:interface|type|class|enum|trait|struct|function|fn|const|let)\s+([A-Za-z_][A-Za-z0-9_]*)",
    )
    .expect("valid symbol regex")
});
static IDENTIFIER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_]{1,79}$").expect("valid identifier regex"));

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionPreservationHints {
    pub active_request: Vec<String>,
    pub required_references: Vec<String>,
}

fn render_bulleted_lines(lines: &[&str]) -> String {
    lines
        .iter()
        .map(|line| format!("- {}", line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_base_compaction_instruction() -> String {
    format!(
        "Summarise the previous conversation history using strict compact Markdown.\n\n{}\n\nCompression rules:\n{}\n\nSection limits:\n{}\n\n{}",
        COMPACTION_SECTION_SCHEMA,
        render_bulleted_lines(COMPACTION_RULES),
        render_bulleted_lines(COMPACTION_SECTION_LIMITS),
        COMPACTION_OUTPUT_CONSTRAINT
    )
}

fn normalize_instruction_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_instruction_text(text: &str, limit: usize) -> String {
    let normalized = normalize_instruction_text(text);
    if normalized.chars().count() <= limit {
        return normalized;
    }

    let truncated = normalized
        .chars()
        .take(limit.saturating_sub(1))
        .collect::<String>();
    format!("{}…", truncated.trim_end())
}

fn extract_message_text_fragments(message: &Message) -> Vec<String> {
    message
        .content
        .iter()
        .filter_map(|content| match content {
            MCPContent::Text { text, .. } => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            _ => None,
        })
        .collect()
}

fn push_unique_limited(values: &mut Vec<String>, candidate: String, limit: usize) {
    if candidate.is_empty() || values.iter().any(|existing| existing == &candidate) {
        return;
    }

    if values.len() < limit {
        values.push(candidate);
    }
}

fn format_reference_candidate(candidate: &str) -> String {
    if PATH_REFERENCE_RE.is_match(candidate) {
        format!("Preserve file path `{}`", candidate)
    } else {
        format!("Preserve identifier `{}`", candidate)
    }
}

fn is_reference_candidate(text: &str) -> bool {
    PATH_REFERENCE_RE.is_match(text) || IDENTIFIER_RE.is_match(text)
}

fn extract_reference_candidates_from_text(text: &str) -> Vec<String> {
    let mut references = Vec::new();

    for captures in BACKTICK_REFERENCE_RE.captures_iter(text) {
        if let Some(candidate) = captures.get(1).map(|capture| capture.as_str().trim()) {
            if is_reference_candidate(candidate) {
                push_unique_limited(&mut references, candidate.to_string(), usize::MAX);
            }
        }
    }

    for candidate in PATH_REFERENCE_RE
        .find_iter(text)
        .map(|capture| capture.as_str())
    {
        push_unique_limited(&mut references, candidate.to_string(), usize::MAX);
    }

    for captures in SYMBOL_REFERENCE_RE.captures_iter(text) {
        if let Some(candidate) = captures.get(1).map(|capture| capture.as_str()) {
            push_unique_limited(&mut references, candidate.to_string(), usize::MAX);
        }
    }

    references
}

fn extract_compact_summary_body(message: &Message) -> Option<String> {
    if !message.is_compact_summary() {
        return None;
    }

    let text = extract_message_text_fragments(message).join("\n");
    let body = text.strip_prefix("### Previous Conversation Summary\n\n")?;
    let summary_only = body
        .split("\n\n### Recent Tool Call Snapshot (latest 5)\n")
        .next()
        .unwrap_or(body)
        .trim();

    if summary_only.is_empty() {
        None
    } else {
        Some(summary_only.to_string())
    }
}

fn extract_summary_section_bullets(summary: &str, section_heading: &str) -> Vec<String> {
    let mut bullets = Vec::new();
    let mut in_section = false;
    let markdown_heading = format!("### {}", section_heading);

    for line in summary.lines() {
        let trimmed = line.trim();

        if trimmed == markdown_heading || trimmed == section_heading {
            in_section = true;
            continue;
        }

        if trimmed.starts_with("### ") || trimmed.ends_with(':') {
            in_section = false;
        }

        if in_section {
            if let Some(bullet) = trimmed.strip_prefix("- ") {
                let normalized = bullet.trim();
                if !normalized.is_empty() {
                    bullets.push(normalized.to_string());
                }
            }
        }
    }

    bullets
}

fn collect_prior_summary_hints(
    message: &Message,
    active_request: &mut Vec<String>,
    required_references: &mut Vec<String>,
) {
    let Some(summary_body) = extract_compact_summary_body(message) else {
        return;
    };

    for bullet in extract_summary_section_bullets(&summary_body, "Active Request") {
        push_unique_limited(
            active_request,
            truncate_instruction_text(&bullet, INSTRUCTION_HINT_TEXT_LIMIT),
            ACTIVE_REQUEST_BULLET_LIMIT,
        );
    }

    for bullet in extract_summary_section_bullets(&summary_body, "Required References") {
        push_unique_limited(required_references, bullet, REQUIRED_REFERENCE_BULLET_LIMIT);
    }
}

fn collect_reference_candidates_from_json_value(
    value: &serde_json::Value,
    references: &mut Vec<String>,
) {
    match value {
        serde_json::Value::String(text) => {
            let extracted = extract_reference_candidates_from_text(text);
            if extracted.is_empty() && IDENTIFIER_RE.is_match(text) {
                push_unique_limited(references, text.clone(), usize::MAX);
            } else {
                for candidate in extracted {
                    push_unique_limited(references, candidate, usize::MAX);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_reference_candidates_from_json_value(item, references);
            }
        }
        serde_json::Value::Object(map) => {
            for value in map.values() {
                collect_reference_candidates_from_json_value(value, references);
            }
        }
        _ => {}
    }
}

fn collect_reference_candidates_from_message(message: &Message, references: &mut Vec<String>) {
    for text in extract_message_text_fragments(message) {
        for candidate in extract_reference_candidates_from_text(&text) {
            push_unique_limited(references, candidate, usize::MAX);
        }
    }

    if let Some(tool_calls) = &message.tool_calls {
        for tool_call in tool_calls {
            if let Ok(arguments) =
                serde_json::from_str::<serde_json::Value>(&tool_call.function.arguments)
            {
                collect_reference_candidates_from_json_value(&arguments, references);
            }
        }
    }
}

pub fn build_compaction_preservation_hints(messages: &[Message]) -> CompactionPreservationHints {
    let mut active_request = Vec::new();
    let mut required_references = Vec::new();

    if let Some(previous_summary) = messages.iter().find(|message| message.is_compact_summary()) {
        collect_prior_summary_hints(
            previous_summary,
            &mut active_request,
            &mut required_references,
        );
    }

    let Some(active_request_start) = find_latest_external_request_block_start(messages) else {
        return CompactionPreservationHints {
            active_request,
            required_references,
        };
    };

    let request_block_end = messages[active_request_start..]
        .iter()
        .take_while(|message| message.is_external_request_message())
        .count()
        + active_request_start;

    for message in &messages[active_request_start..request_block_end] {
        for fragment in extract_message_text_fragments(message) {
            push_unique_limited(
                &mut active_request,
                truncate_instruction_text(&fragment, INSTRUCTION_HINT_TEXT_LIMIT),
                ACTIVE_REQUEST_BULLET_LIMIT,
            );
            for candidate in extract_reference_candidates_from_text(&fragment) {
                push_unique_limited(
                    &mut required_references,
                    format_reference_candidate(&candidate),
                    REQUIRED_REFERENCE_BULLET_LIMIT,
                );
            }
        }
    }

    let reference_window_start =
        active_request_start.saturating_sub(REFERENCE_CONTEXT_WINDOW_MESSAGES);
    let mut raw_reference_candidates = Vec::new();
    for message in &messages[reference_window_start..request_block_end] {
        collect_reference_candidates_from_message(message, &mut raw_reference_candidates);
    }

    for candidate in raw_reference_candidates {
        push_unique_limited(
            &mut required_references,
            format_reference_candidate(&candidate),
            REQUIRED_REFERENCE_BULLET_LIMIT,
        );
    }

    CompactionPreservationHints {
        active_request,
        required_references,
    }
}

fn build_compaction_hint_block(messages: &[Message]) -> Option<String> {
    let hints = build_compaction_preservation_hints(messages);
    if hints.active_request.is_empty() && hints.required_references.is_empty() {
        return None;
    }

    let mut parts = Vec::new();
    if !hints.active_request.is_empty() {
        parts.push(format!(
            "Active Request candidates:\n{}",
            hints
                .active_request
                .iter()
                .map(|line| format!("- {}", line))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    if !hints.required_references.is_empty() {
        parts.push(format!(
            "Required References candidates:\n{}",
            hints
                .required_references
                .iter()
                .map(|line| format!("- {}", line))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    Some(format!(
        "Preservation hints for this compaction input:\n{}",
        parts.join("\n\n")
    ))
}

fn build_compaction_instruction(messages: &[Message]) -> String {
    let mut instruction = build_base_compaction_instruction();
    if let Some(hint_block) = build_compaction_hint_block(messages) {
        instruction = format!("{}\n\n{}", instruction, hint_block);
    }

    if messages.first().map(Message::is_compact_summary) == Some(true) {
        return format!(
            "{}\n\n{}",
            INCREMENTAL_COMPACTION_RESIDUAL_PREFIX, instruction
        );
    }

    instruction
}

fn build_compaction_instruction_message(
    session_id: &str,
    instruction: String,
    created_at: i64,
) -> Message {
    Message {
        id: format!("compaction-instruction-{}", created_at),
        session_id: session_id.to_string(),
        role: "user".to_string(),
        content: vec![MCPContent::Text {
            text: instruction,
            is_error: None,
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
        created_at,
        updated_at: created_at,
        source: Some(MessageSource::CompactionInstruction),
        error: None,
        metadata: None,
    }
}

fn build_compaction_request_payload(
    session_id: &str,
    messages: &[Message],
    split_idx: usize,
    compact_record: Option<&CompactContextRecord>,
    created_at: i64,
) -> Option<(Vec<Message>, String, String, usize, bool)> {
    if split_idx == 0 {
        return None;
    }

    if let Some(record) = compact_record {
        if let Some(compacted_to_idx) = messages
            .iter()
            .position(|message| message.id == record.to_id)
        {
            // Incremental compaction input is:
            //   [previous summary as one synthetic message] + [raw delta since record.to_id]
            // It is deliberately not the full compactable prefix.
            let first_delta_message_idx = compacted_to_idx.saturating_add(1);
            if first_delta_message_idx >= split_idx {
                return None;
            }

            let mut compact_messages = Vec::with_capacity(1 + split_idx - first_delta_message_idx);
            compact_messages.push(build_incremental_compact_summary_message(
                session_id,
                &record.summary,
                &messages[..=compacted_to_idx],
                created_at,
            ));
            compact_messages.extend(messages[first_delta_message_idx..split_idx].iter().cloned());

            let instruction = build_compaction_instruction(&compact_messages);
            compact_messages.push(build_compaction_instruction_message(
                session_id,
                instruction,
                created_at,
            ));

            return Some((
                compact_messages,
                record.from_id.clone(),
                messages[split_idx - 1].id.clone(),
                split_idx - first_delta_message_idx,
                true,
            ));
        }
    }

    // First compaction in a session has no previous compact summary yet, so the
    // compactable raw prefix becomes the initial delta baseline.
    let mut compact_messages = messages[..split_idx].to_vec();
    let instruction = build_compaction_instruction(&compact_messages);
    compact_messages.push(build_compaction_instruction_message(
        session_id,
        instruction,
        created_at,
    ));
    Some((
        compact_messages,
        messages
            .first()
            .map(|message| message.id.clone())
            .unwrap_or_default(),
        messages[split_idx - 1].id.clone(),
        split_idx,
        false,
    ))
}

fn provider_requires_compaction_tool_chain_cleanup(provider_id: &str) -> bool {
    ["anthropic", "gemini", "openai", "openrouter", "groq"].contains(&provider_id)
}

fn drop_oldest_compaction_message(messages: &mut Vec<Message>) -> bool {
    if messages.len() <= 1 {
        return false;
    }

    let drop_index = if messages[0].is_compact_summary() && messages.len() > 1 {
        1
    } else {
        0
    };
    messages.remove(drop_index);
    true
}

pub fn fit_compaction_request_messages_to_limit(
    messages: &[Message],
    provider_id: &str,
    safe_input_token_limit: usize,
    system_prompt_tokens: usize,
    tools_tokens: usize,
) -> Result<Vec<Message>, String> {
    let preserved_calibration_ratio =
        crate::agent::llm::token_utils::try_derive_bpe_calibration_ratio(
            messages,
            system_prompt_tokens,
            tools_tokens,
        );

    let mut fitted = messages.to_vec();
    while fitted.len() > 1 {
        let conservative_total =
            crate::agent::llm::token_utils::calculate_conservative_preflight_prompt_tokens(
                &fitted,
                system_prompt_tokens,
                tools_tokens,
                preserved_calibration_ratio,
            );
        if conservative_total < safe_input_token_limit {
            return Ok(fitted);
        }

        if !drop_oldest_compaction_message(&mut fitted) {
            break;
        }

        if provider_requires_compaction_tool_chain_cleanup(provider_id) {
            fitted = crate::agent::llm::context_selector::remove_incomplete_tool_chains(fitted);
        }
    }

    let single_message = if fitted.len() == 1 {
        crate::agent::llm::context_selector::truncate_single_oversized_message_to_fit_conservative_limit(
            &fitted,
            safe_input_token_limit,
            system_prompt_tokens,
            tools_tokens,
            preserved_calibration_ratio,
        )
    } else {
        fitted
    };

    if single_message
        .iter()
        .all(|message| message.is_compact_summary() || message.is_compaction_instruction())
    {
        return Err(
            "Compaction payload reduction exhausted the raw delta and would leave only the prior compact summary anchor and compaction instruction scaffolding.".to_string(),
        );
    }

    let conservative_total =
        crate::agent::llm::token_utils::calculate_conservative_preflight_prompt_tokens(
            &single_message,
            system_prompt_tokens,
            tools_tokens,
            preserved_calibration_ratio,
        );

    if conservative_total < safe_input_token_limit {
        return Ok(single_message);
    }

    Err(format!(
        "Compaction payload still exceeds the effective context limit after compaction-fit reduction ({} >= {}).",
        conservative_total, safe_input_token_limit
    ))
}

fn estimate_compaction_non_message_tokens(
    parent_request: Option<&CompactionParentRequest>,
) -> (usize, usize) {
    let combined_system_prompt = parent_request.and_then(|request| {
        match (&request.system_prompt, &request.session_context) {
            (Some(system_prompt), Some(session_context)) => {
                Some(format!("{}\n\n{}", system_prompt, session_context))
            }
            (Some(system_prompt), None) => Some(system_prompt.clone()),
            (None, Some(session_context)) => Some(session_context.clone()),
            (None, None) => None,
        }
    });
    let system_prompt_tokens = combined_system_prompt
        .as_ref()
        .map(|prompt| crate::agent::llm::token_utils::estimate_text_tokens(prompt))
        .unwrap_or(0);
    let tools_tokens = parent_request
        .and_then(|request| request.available_tools.as_ref())
        .and_then(|tools| serde_json::to_string(tools).ok())
        .map(|tools_json| crate::agent::llm::token_utils::estimate_text_tokens(&tools_json))
        .unwrap_or(0);

    (system_prompt_tokens, tools_tokens)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionSelectionPreview {
    pub compacted_ids: Vec<String>,
    pub preserved_ids: Vec<String>,
}

fn build_compaction_selection_preview(
    messages: &[Message],
    split_idx: usize,
) -> CompactionSelectionPreview {
    let split_idx = split_idx.min(messages.len());

    CompactionSelectionPreview {
        compacted_ids: messages[..split_idx]
            .iter()
            .map(|message| message.id.clone())
            .collect(),
        preserved_ids: messages[split_idx..]
            .iter()
            .map(|message| message.id.clone())
            .collect(),
    }
}

pub fn preview_preflight_compaction_selection(messages: &[Message]) -> CompactionSelectionPreview {
    build_compaction_selection_preview(messages, find_preflight_compaction_split_index(messages))
}

pub fn preview_background_compaction_selection(messages: &[Message]) -> CompactionSelectionPreview {
    build_compaction_selection_preview(messages, find_background_compaction_split_index(messages))
}

/// Determines the compactable prefix for preflight compaction.
///
/// Unlike background compaction, preflight compaction must preserve the newest
/// direct user turn so we don't summarize away the input that still needs an
/// answer. However, workflow-generated tool output at the tail is compactable,
/// and unresolved tool chains must remain intact. This helper aligns those
/// constraints into a single split index:
///
/// - latest `user`/non-tool tail: preserve the last message
/// - latest `tool` tail: compact as much as `find_compaction_split_index()` allows
/// - unresolved tool chain: preserve the full unresolved tail
fn find_preflight_compaction_split_index(messages: &[Message]) -> usize {
    if messages.is_empty() {
        return 0;
    }

    let unresolved_boundary =
        crate::agent::llm::context_selector::find_compaction_split_index(messages);

    match messages.last().map(|message| message.role.as_str()) {
        Some("tool") => unresolved_boundary,
        Some(_) => std::cmp::min(messages.len().saturating_sub(1), unresolved_boundary),
        None => 0,
    }
}

fn find_latest_external_request_block_start(messages: &[Message]) -> Option<usize> {
    let latest_request_idx = messages
        .iter()
        .rposition(Message::is_external_request_message)?;

    let mut block_start = latest_request_idx;
    while block_start > 0 && messages[block_start - 1].is_external_request_message() {
        block_start -= 1;
    }

    Some(block_start)
}

fn find_background_compaction_split_index(messages: &[Message]) -> usize {
    let unresolved_boundary =
        crate::agent::llm::context_selector::find_compaction_split_index(messages);

    // Post-response compaction must keep the currently active external request in
    // raw tail, but unresolved tool chains still take precedence so we never
    // compact an assistant tool-call owner away from its pending tool results.
    let Some(active_request_start) = find_latest_external_request_block_start(messages) else {
        return unresolved_boundary;
    };

    std::cmp::min(unresolved_boundary, active_request_start)
}

pub fn should_skip_same_tail_compaction(messages: &[Message], split_idx: usize) -> bool {
    let has_compact_summary = messages
        .first()
        .map(|message| message.is_compact_summary())
        .unwrap_or(false);

    if !has_compact_summary {
        // Without a persisted compact summary at the head, a same-tail retry is
        // still meaningful: the previous compaction may have failed or been
        // abandoned before Rust stored the summary record.
        return false;
    }

    split_idx <= 1
}

async fn load_merged_compaction_messages(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> Result<(String, Vec<Message>), String> {
    let (session_name, messages) = {
        let active = active_sessions.read().await;
        let session = active
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        let session_name = session
            .metadata
            .name
            .clone()
            .unwrap_or_else(|| session_id.chars().take(8).collect::<String>());

        let messages = session
            .messages
            .read()
            .await
            .iter()
            .filter(|m| !m.is_recovery_message())
            .cloned()
            .collect::<Vec<_>>();

        (session_name, messages)
    };

    Ok((
        session_name,
        super::request::normalize_request_messages(messages),
    ))
}

struct BackgroundCompactionTrigger<'a> {
    session_id: &'a str,
    session_name: &'a str,
    messages: &'a [Message],
    split_idx: usize,
    parent_request: Option<CompactionParentRequest>,
}

impl<'a> BackgroundCompactionTrigger<'a> {
    async fn execute(
        self,
        active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
        app_handle: &AppHandle,
        handles: &BackgroundCompactionHandles,
    ) -> Result<bool, String> {
        let Self {
            session_id,
            session_name,
            messages,
            split_idx,
            parent_request,
        } = self;

        if split_idx == 0 {
            return Ok(false);
        }

        let claimed_in_flight = handles
            .compact_in_flight_arc
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok();

        if !claimed_in_flight {
            log::debug!("⏭️ Compaction skipped (in_flight): session={}", session_id);
            return Ok(false);
        }

        let mut in_flight_guard = InFlightResetGuard::new(handles.compact_in_flight_arc.clone());
        let current_tail_id = messages.last().map(|m| m.id.clone());
        let last_compacted_tail = handles.last_compacted_tail_id_arc.read().await.clone();
        let same_tail = current_tail_id.as_deref() == last_compacted_tail.as_deref();

        if same_tail && should_skip_same_tail_compaction(messages, split_idx) {
            log::debug!(
                "⏭️ Compaction skipped (same tail): session={}, tail={}",
                session_id,
                current_tail_id.as_deref().unwrap_or("?")
            );
            return Ok(false);
        }
        let started_at_ms = chrono::Utc::now().timestamp_millis();
        let compact_context_record = {
            let active = active_sessions.read().await;
            active
                .get(session_id)
                .map(|session| session.compact_context.clone())
        };
        let compact_context_record = if let Some(compact_context_handle) = compact_context_record {
            compact_context_handle.read().await.clone()
        } else {
            None
        };
        let Some((compact_msgs, from_id, to_id, compacted_delta_count, reused_prior_summary)) =
            build_compaction_request_payload(
                session_id,
                messages,
                split_idx,
                compact_context_record.as_ref(),
                started_at_ms,
            )
        else {
            log::debug!(
                "⏭️ Compaction skipped (no new delta beyond previous summary): session={}, tail={}",
                session_id,
                current_tail_id.as_deref().unwrap_or("?")
            );
            return Ok(false);
        };

        let compact_started_at_ms_handle = {
            let active = active_sessions.read().await;
            active
                .get(session_id)
                .map(|session| session.compaction.started_at_ms.clone())
        };

        let settings = load_context_management_settings().await;
        let safe_input_token_limit =
            std::cmp::min(settings.max_input_context, settings.model_max_limit);
        let resolved_parent_request =
            resolve_parent_request(active_sessions, session_id, parent_request).await;
        let (system_prompt_tokens, tools_tokens) =
            estimate_compaction_non_message_tokens(resolved_parent_request.as_ref());
        let compact_msgs = fit_compaction_request_messages_to_limit(
            &compact_msgs,
            resolved_parent_request
                .as_ref()
                .map(|request| request.provider.as_str())
                .unwrap_or("openai"),
            safe_input_token_limit,
            system_prompt_tokens,
            tools_tokens,
        )?;

        let compact_event = CompactRequest {
            session_id: session_id.to_string(),
            session_name: session_name.to_string(),
            messages: compact_msgs,
            from_id,
            to_id,
            parent_request: resolved_parent_request,
            resume_completion_after_compact: false,
        };
        let log_from_id = compact_event.from_id.clone();
        let log_to_id = compact_event.to_id.clone();
        let app = app_handle.clone();
        let state_session_id = session_id.to_string();
        let state_session_name = session_name.to_string();
        let in_flight_flag = handles.compact_in_flight_arc.clone();
        let last_compacted_tail_id_arc = handles.last_compacted_tail_id_arc.clone();
        let tail_id_for_task = current_tail_id.clone();
        tokio::spawn(async move {
            if let Err(e) = emit_compact_started(
                &app,
                state_session_id.clone(),
                Some(state_session_name),
                false,
            ) {
                log::error!("Failed to emit llm:compact-state: {}", e);
                in_flight_flag.store(false, Ordering::SeqCst);
                return;
            }
            if let Err(e) = emit_compact_request(&app, compact_event) {
                log::error!("Failed to emit llm:compact-request: {}", e);
                in_flight_flag.store(false, Ordering::SeqCst);
                return;
            }
            *last_compacted_tail_id_arc.write().await = tail_id_for_task;
            if let Some(compact_started_at_ms_handle) = compact_started_at_ms_handle {
                *compact_started_at_ms_handle.write().await = Some(started_at_ms);
            }
        });
        in_flight_guard.disarm();
        log::info!(
            "🔧 Background compaction triggered: session={}, from_id={}, to_id={}, split_idx={}, compacted_delta_count={}, reused_prior_summary={}, tail={}, started_at_ms={}",
            session_id,
            log_from_id,
            log_to_id,
            split_idx,
            compacted_delta_count,
            reused_prior_summary,
            current_tail_id.as_deref().unwrap_or("?"),
            started_at_ms
        );

        Ok(true)
    }
}

pub(crate) async fn trigger_post_response_background_compaction(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    session_name: &str,
    messages: &[Message],
    parent_request: Option<CompactionParentRequest>,
    handles: &BackgroundCompactionHandles,
) -> Result<bool, String> {
    BackgroundCompactionTrigger {
        session_id,
        session_name,
        messages,
        parent_request,
        split_idx: find_background_compaction_split_index(messages),
    }
    .execute(active_sessions, app_handle, handles)
    .await
}

pub async fn trigger_post_response_compaction_if_needed(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    session_name: &str,
    messages: &[Message],
    usage_total_tokens: usize,
    deferred_step: DeferredWorkflowStep,
) -> Result<bool, String> {
    let settings = load_context_management_settings().await;
    let safe_input_token_limit =
        std::cmp::min(settings.max_input_context, settings.model_max_limit);
    let trigger_threshold =
        crate::agent::llm::token_utils::calculate_compact_threshold(safe_input_token_limit);
    let should_trigger = should_trigger_post_response_compaction(
        usage_total_tokens,
        safe_input_token_limit,
        &settings.context_strategy,
    );

    log::info!(
        "🧮 Post-response compaction evaluation: session={}, total_tokens={}, strategy={}, configured_max_input_context={}, model_max_limit={}, safe_input_token_limit={}, trigger_threshold={}, should_trigger={}",
        session_id,
        usage_total_tokens,
        settings.context_strategy,
        settings.max_input_context,
        settings.model_max_limit,
        safe_input_token_limit,
        trigger_threshold,
        should_trigger
    );

    if !should_trigger {
        return Ok(false);
    }

    let (handles, finalize_workflow_after_compact, deferred_workflow_step_handle) = {
        let active = active_sessions.read().await;
        let session = active
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        (
            BackgroundCompactionHandles {
                compact_in_flight_arc: session.compaction.in_flight.clone(),
                last_compacted_tail_id_arc: session.compaction.last_compacted_tail_id.clone(),
            },
            session.compaction.finalize_workflow_after_compact.clone(),
            session.compaction.deferred_workflow_step.clone(),
        )
    };

    finalize_workflow_after_compact.store(true, Ordering::SeqCst);
    *deferred_workflow_step_handle.write().await = Some(deferred_step);
    let triggered = trigger_post_response_background_compaction(
        active_sessions,
        app_handle,
        session_id,
        session_name,
        messages,
        None,
        &handles,
    )
    .await?;

    if !triggered {
        finalize_workflow_after_compact.store(false, Ordering::SeqCst);
        *deferred_workflow_step_handle.write().await = None;
    } else {
        log::info!(
            "🧹 Post-response compaction triggered synchronously from completed response usage: session={}, total_tokens={}, limit={}",
            session_id,
            usage_total_tokens,
            safe_input_token_limit
        );
    }

    Ok(triggered)
}

pub(crate) async fn try_trigger_preflight_compaction(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    session_name: &str,
    messages: &[Message],
    parent_request: Option<CompactionParentRequest>,
    resume_completion_after_compact: bool,
) -> Result<bool, String> {
    if messages.len() <= 1 {
        log::debug!(
            "⏭️ Preflight compaction skipped (insufficient messages): session={}, count={}",
            session_id,
            messages.len()
        );
        return Ok(false);
    }

    let split_idx = find_preflight_compaction_split_index(messages);
    if split_idx == 0 {
        log::debug!(
            "⏭️ Preflight compaction skipped (split_idx=0): session={}",
            session_id
        );
        return Ok(false);
    }
    let (
        compact_in_flight_arc,
        last_compacted_tail_id_arc,
        awaiting_compact_arc,
        compact_context_handle,
    ) = {
        let active = active_sessions.read().await;
        let session = active
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        (
            session.compaction.in_flight.clone(),
            session.compaction.last_compacted_tail_id.clone(),
            session.compaction.awaiting_completion.clone(),
            session.compact_context.clone(),
        )
    };

    let claimed_in_flight = compact_in_flight_arc
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok();
    if !claimed_in_flight {
        if resume_completion_after_compact {
            awaiting_compact_arc.store(true, Ordering::SeqCst);
        }
        emit_compact_started(
            app_handle,
            session_id.to_string(),
            Some(session_name.to_string()),
            resume_completion_after_compact,
        )?;
        if resume_completion_after_compact {
            log::info!(
                "⏳ Reusing in-flight compaction and arming resume-after-compact: session={}, mode=preflight",
                session_id
            );
        } else {
            log::info!(
                "⏳ Reusing in-flight compaction without resume-after-compact: session={}, mode=manual",
                session_id
            );
        }
        return Ok(true);
    }

    let mut in_flight_guard = InFlightResetGuard::new(compact_in_flight_arc.clone());
    let current_tail_id = messages.last().map(|m| m.id.clone());
    let last_compacted_tail = last_compacted_tail_id_arc.read().await.clone();
    if current_tail_id.as_deref() == last_compacted_tail.as_deref()
        && should_skip_same_tail_compaction(messages, split_idx)
    {
        log::debug!(
            "⏭️ Preflight compaction skipped (same tail): session={}, tail={}, split_idx={}",
            session_id,
            current_tail_id.as_deref().unwrap_or("?"),
            split_idx
        );
        return Ok(false);
    }

    let started_at_ms = chrono::Utc::now().timestamp_millis();
    let compact_context_record = compact_context_handle.read().await.clone();
    let Some((compact_msgs, from_id, to_id, compacted_delta_count, reused_prior_summary)) =
        build_compaction_request_payload(
            session_id,
            messages,
            split_idx,
            compact_context_record.as_ref(),
            started_at_ms,
        )
    else {
        log::debug!(
            "⏭️ Preflight compaction skipped (no new delta beyond previous summary): session={}, tail={}, split_idx={}",
            session_id,
            current_tail_id.as_deref().unwrap_or("?"),
            split_idx
        );
        return Ok(false);
    };

    let compact_started_at_ms_handle = {
        let active = active_sessions.read().await;
        active
            .get(session_id)
            .map(|session| session.compaction.started_at_ms.clone())
    };

    let settings = load_context_management_settings().await;
    let safe_input_token_limit =
        std::cmp::min(settings.max_input_context, settings.model_max_limit);
    let resolved_parent_request =
        resolve_parent_request(active_sessions, session_id, parent_request).await;
    let (system_prompt_tokens, tools_tokens) =
        estimate_compaction_non_message_tokens(resolved_parent_request.as_ref());
    let compact_msgs = fit_compaction_request_messages_to_limit(
        &compact_msgs,
        resolved_parent_request
            .as_ref()
            .map(|request| request.provider.as_str())
            .unwrap_or("openai"),
        safe_input_token_limit,
        system_prompt_tokens,
        tools_tokens,
    )?;

    let compact_event = CompactRequest {
        session_id: session_id.to_string(),
        session_name: session_name.to_string(),
        messages: compact_msgs,
        from_id,
        to_id,
        parent_request: resolved_parent_request,
        resume_completion_after_compact,
    };
    let log_from_id = compact_event.from_id.clone();
    let log_to_id = compact_event.to_id.clone();
    emit_compact_started(
        app_handle,
        session_id.to_string(),
        Some(session_name.to_string()),
        resume_completion_after_compact,
    )?;
    emit_compact_request(app_handle, compact_event)?;
    if resume_completion_after_compact {
        awaiting_compact_arc.store(true, Ordering::SeqCst);
    }
    *last_compacted_tail_id_arc.write().await = current_tail_id.clone();
    if let Some(compact_started_at_ms_handle) = compact_started_at_ms_handle {
        *compact_started_at_ms_handle.write().await = Some(started_at_ms);
    }
    in_flight_guard.disarm();

    if resume_completion_after_compact {
        log::info!(
            "⏸️ Preflight compaction triggered: session={}, from_id={}, to_id={}, split_idx={}, compacted_delta_count={}, reused_prior_summary={}, tail={}, started_at_ms={}",
            session_id,
            log_from_id,
            log_to_id,
            split_idx,
            compacted_delta_count,
            reused_prior_summary,
            current_tail_id.as_deref().unwrap_or("?"),
            started_at_ms
        );
    } else {
        log::info!(
            "🧰 Manual compaction triggered: session={}, from_id={}, to_id={}, split_idx={}, compacted_delta_count={}, reused_prior_summary={}, tail={}, started_at_ms={}",
            session_id,
            log_from_id,
            log_to_id,
            split_idx,
            compacted_delta_count,
            reused_prior_summary,
            current_tail_id.as_deref().unwrap_or("?"),
            started_at_ms
        );
    }

    Ok(true)
}

pub async fn trigger_preflight_compaction_for_session(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
) -> Result<bool, String> {
    let (session_name, merged_messages) =
        load_merged_compaction_messages(active_sessions, session_id).await?;
    try_trigger_preflight_compaction(
        active_sessions,
        app_handle,
        session_id,
        &session_name,
        &merged_messages,
        None,
        true,
    )
    .await
}

pub async fn trigger_manual_compaction_for_session(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
) -> Result<bool, String> {
    let (session_name, merged_messages) =
        load_merged_compaction_messages(active_sessions, session_id).await?;
    try_trigger_preflight_compaction(
        active_sessions,
        app_handle,
        session_id,
        &session_name,
        &merged_messages,
        None,
        false,
    )
    .await
}
