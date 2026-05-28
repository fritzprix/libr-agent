use crate::agent::llm::completion::request::build_compact_summary_message_for_messages;
use crate::agent::llm::types::CompactionParentRequest;
use crate::mcp::types::MCPContent;
use crate::models::chat::Message;
use crate::repositories::CompactContextRecord;

use super::instruction::{build_compaction_instruction, build_compaction_instruction_message};

pub(super) struct CompactionRequestPayload {
    pub(super) compact_messages: Vec<Message>,
    pub(super) to_id: String,
    pub(super) compacted_delta_count: usize,
    pub(super) reused_prior_summary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionRequestPayloadPreview {
    pub message_count: usize,
    pub to_id: String,
    pub compacted_delta_count: usize,
    pub reused_prior_summary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionPayloadMessageDiagnostic {
    pub id: String,
    pub role: String,
    pub source: String,
    pub flags: Vec<String>,
    pub preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionPayloadDiagnostics {
    pub total_messages: usize,
    pub body_message_count: usize,
    pub raw_delta_message_count: usize,
    pub compact_summary_count: usize,
    pub compaction_instruction_count: usize,
    pub scaffolding_count: usize,
    pub external_request_count: usize,
    pub assistant_message_count: usize,
    pub tool_message_count: usize,
    pub latest_external_request_message_ids: Vec<String>,
    pub messages: Vec<CompactionPayloadMessageDiagnostic>,
}

pub fn apply_compaction_retry_budget(safe_input_token_limit: usize, retry_attempt: u32) -> usize {
    let reduction_percent = match retry_attempt {
        0 => 100,
        1 => 85,
        2 => 70,
        _ => 55,
    };
    let minimum_floor = safe_input_token_limit.min(1024);
    std::cmp::max(
        safe_input_token_limit.saturating_mul(reduction_percent) / 100,
        minimum_floor,
    )
}

fn build_incremental_compact_summary_message(
    session_id: &str,
    summary: &str,
    compacted_messages: &[Message],
    created_at: i64,
) -> Message {
    build_compact_summary_message_for_messages(session_id, summary, compacted_messages, created_at)
}

pub(super) fn build_compaction_request_payload(
    session_id: &str,
    messages: &[Message],
    split_idx: usize,
    compact_record: Option<&CompactContextRecord>,
    created_at: i64,
) -> Option<CompactionRequestPayload> {
    if split_idx == 0 {
        return None;
    }

    if let Some(record) = compact_record {
        if let Some(compacted_to_idx) = messages
            .iter()
            .position(|message| message.id == record.to_id)
        {
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

            return Some(CompactionRequestPayload {
                compact_messages,
                to_id: messages[split_idx - 1].id.clone(),
                compacted_delta_count: split_idx - first_delta_message_idx,
                reused_prior_summary: true,
            });
        }
    }

    let mut compact_messages = messages[..split_idx].to_vec();
    let instruction = build_compaction_instruction(&compact_messages);
    compact_messages.push(build_compaction_instruction_message(
        session_id,
        instruction,
        created_at,
    ));
    Some(CompactionRequestPayload {
        compact_messages,
        to_id: messages[split_idx - 1].id.clone(),
        compacted_delta_count: split_idx,
        reused_prior_summary: false,
    })
}

pub fn build_compaction_request_payload_for_testing(
    session_id: &str,
    messages: &[Message],
    split_idx: usize,
    compact_record: Option<&CompactContextRecord>,
    created_at: i64,
) -> Option<CompactionRequestPayloadPreview> {
    build_compaction_request_payload(session_id, messages, split_idx, compact_record, created_at)
        .map(|payload| CompactionRequestPayloadPreview {
            message_count: payload.compact_messages.len(),
            to_id: payload.to_id,
            compacted_delta_count: payload.compacted_delta_count,
            reused_prior_summary: payload.reused_prior_summary,
        })
}

fn compact_message_preview(message: &Message) -> String {
    let preview = message
        .content
        .iter()
        .filter_map(|content| match content {
            MCPContent::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    const PREVIEW_LIMIT: usize = 120;
    if preview.chars().count() <= PREVIEW_LIMIT {
        return preview;
    }

    let truncated = preview.chars().take(PREVIEW_LIMIT).collect::<String>();
    format!("{}...", truncated)
}

fn compaction_message_flags(message: &Message) -> Vec<String> {
    let mut flags = Vec::new();

    if message.is_compact_summary() {
        flags.push("compact_summary".to_string());
    }
    if message.is_compaction_instruction() {
        flags.push("compaction_instruction".to_string());
    }
    if message.is_request_layout_scaffolding_message() {
        flags.push("scaffolding".to_string());
    }
    if message.is_external_request_message() {
        flags.push("external_request".to_string());
    }
    if message.role == "tool" {
        flags.push("tool".to_string());
    }
    if message
        .tool_calls
        .as_ref()
        .is_some_and(|calls| !calls.is_empty())
    {
        flags.push("tool_call".to_string());
    }

    flags
}

pub fn inspect_compaction_payload(messages: &[Message]) -> CompactionPayloadDiagnostics {
    let latest_external_request_message_ids = latest_real_user_request_block_range(messages)
        .map(|(start, end)| {
            messages[start..end]
                .iter()
                .map(|message| message.id.clone())
                .collect()
        })
        .unwrap_or_default();

    CompactionPayloadDiagnostics {
        total_messages: messages.len(),
        body_message_count: messages
            .iter()
            .filter(|message| {
                !message.is_request_layout_scaffolding_message()
                    && !message.is_compaction_overlay_message()
            })
            .count(),
        raw_delta_message_count: messages
            .iter()
            .filter(|message| {
                !message.is_request_layout_scaffolding_message()
                    && !message.is_compaction_overlay_message()
                    && !message.is_compact_summary()
            })
            .count(),
        compact_summary_count: messages
            .iter()
            .filter(|message| message.is_compact_summary())
            .count(),
        compaction_instruction_count: messages
            .iter()
            .filter(|message| message.is_compaction_instruction())
            .count(),
        scaffolding_count: messages
            .iter()
            .filter(|message| message.is_request_layout_scaffolding_message())
            .count(),
        external_request_count: messages
            .iter()
            .filter(|message| message.is_external_request_message())
            .count(),
        assistant_message_count: messages
            .iter()
            .filter(|message| message.role == "assistant")
            .count(),
        tool_message_count: messages
            .iter()
            .filter(|message| message.role == "tool")
            .count(),
        latest_external_request_message_ids,
        messages: messages
            .iter()
            .map(|message| CompactionPayloadMessageDiagnostic {
                id: message.id.clone(),
                role: message.role.clone(),
                source: message
                    .source
                    .as_ref()
                    .map(|source| source.as_str().to_string())
                    .unwrap_or_else(|| "none".to_string()),
                flags: compaction_message_flags(message),
                preview: compact_message_preview(message),
            })
            .collect(),
    }
}

fn provider_requires_compaction_tool_chain_cleanup(provider_id: &str) -> bool {
    ["anthropic", "gemini", "openai", "openrouter", "groq"].contains(&provider_id)
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

    let cleaned = provider_cleanup_compaction_messages(messages.to_vec(), provider_id);
    let conservative_total =
        crate::agent::llm::token_utils::calculate_conservative_preflight_prompt_tokens(
            &cleaned,
            system_prompt_tokens,
            tools_tokens,
            preserved_calibration_ratio,
        );
    if conservative_total < safe_input_token_limit {
        return Ok(cleaned);
    }

    let single_message = if cleaned.len() == 1 {
        crate::agent::llm::context_selector::truncate_single_oversized_message_to_fit_conservative_limit(
            &cleaned,
            safe_input_token_limit,
            system_prompt_tokens,
            tools_tokens,
            preserved_calibration_ratio,
        )
    } else {
        cleaned
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
        "Compaction payload exceeds the effective context limit without lossy cache-aligned trimming ({} >= {}); advance to overflow recovery instead of dropping older compaction history.",
        conservative_total, safe_input_token_limit
    ))
}

fn latest_real_user_request_block_range(messages: &[Message]) -> Option<(usize, usize)> {
    let latest_request_idx = messages
        .iter()
        .rposition(Message::is_external_request_message)?;

    let mut block_start = latest_request_idx;
    while block_start > 0 && messages[block_start - 1].is_external_request_message() {
        block_start -= 1;
    }

    let mut block_end = latest_request_idx + 1;
    while block_end < messages.len() && messages[block_end].is_external_request_message() {
        block_end += 1;
    }

    Some((block_start, block_end))
}

fn compact_summary_has_active_request_anchor(message: &Message) -> bool {
    if !message.is_compact_summary() {
        return false;
    }

    let text = message
        .content
        .iter()
        .filter_map(|content| match content {
            MCPContent::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    let Some((_, active_request_block)) = text.split_once("### Active Request") else {
        return false;
    };

    active_request_block
        .lines()
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with("### "))
        .any(|line| line.trim_start().starts_with("- "))
}

fn latest_request_anchor_range(messages: &[Message]) -> Option<(usize, usize)> {
    if let Some(range) = latest_real_user_request_block_range(messages) {
        return Some(range);
    }

    if let Some(summary_idx) = messages
        .iter()
        .rposition(compact_summary_has_active_request_anchor)
    {
        return Some((summary_idx, summary_idx + 1));
    }

    // If there is no active request text parsed in the summary, but a prior compact summary message
    // itself exists, it guarantees the user's workflow-wide instructions are safely preserved within it.
    // We treat the prior summary itself as the anchor to avoid workflow shutdowns.
    if let Some(summary_idx) = messages.iter().position(|m| m.is_compact_summary()) {
        return Some((summary_idx, summary_idx + 1));
    }

    None
}

fn provider_cleanup_compaction_messages(messages: Vec<Message>, provider_id: &str) -> Vec<Message> {
    if provider_requires_compaction_tool_chain_cleanup(provider_id) {
        crate::agent::llm::context_selector::remove_incomplete_tool_chains(messages)
    } else {
        messages
    }
}

fn candidate_compaction_total(
    messages: &[Message],
    provider_id: &str,
    safe_input_token_limit: usize,
    system_prompt_tokens: usize,
    tools_tokens: usize,
    preserved_calibration_ratio: Option<f64>,
) -> Option<(Vec<Message>, usize)> {
    let cleaned = provider_cleanup_compaction_messages(messages.to_vec(), provider_id);
    let conservative_total =
        crate::agent::llm::token_utils::calculate_conservative_preflight_prompt_tokens(
            &cleaned,
            system_prompt_tokens,
            tools_tokens,
            preserved_calibration_ratio,
        );

    if conservative_total < safe_input_token_limit {
        Some((cleaned, conservative_total))
    } else {
        None
    }
}

fn split_compaction_round_messages(messages: &[Message]) -> (Vec<Message>, Option<Message>) {
    let instruction = messages
        .last()
        .filter(|message| message.is_compaction_overlay_message())
        .cloned();
    let body_end = messages
        .len()
        .saturating_sub(usize::from(instruction.is_some()));
    let body_messages = messages[..body_end]
        .iter()
        .filter(|message| {
            !message.is_request_layout_scaffolding_message()
                && !message.is_compaction_overlay_message()
        })
        .cloned()
        .collect::<Vec<_>>();

    (body_messages, instruction)
}

pub fn build_overflow_recovery_compaction_messages(
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

    let (body_messages, instruction) = split_compaction_round_messages(messages);

    let latest_real_user_request_range = latest_real_user_request_block_range(&body_messages);
    let latest_request_range = latest_request_anchor_range(&body_messages);

    let previous_summary_idx = body_messages
        .iter()
        .position(|message| message.is_compact_summary());
    let remaining_indices = (0..body_messages.len())
        .filter(|index| {
            !matches!(previous_summary_idx, Some(summary_idx) if summary_idx == *index)
                && latest_request_range
                    .map(|(latest_request_start, latest_request_end)| {
                        *index < latest_request_start || *index >= latest_request_end
                    })
                    .unwrap_or(true)
        })
        .collect::<Vec<_>>();

    let attempt_with_summary = previous_summary_idx
        .into_iter()
        .map(Some)
        .chain(std::iter::once(None));

    for summary_idx in attempt_with_summary {
        // Overflow recovery is allowed to reduce the active live body by FIFO semantics.
        // We therefore keep the newest active context possible by progressively dropping
        // older active messages while preserving the priority anchors.
        for fifo_drop_count in 0..=remaining_indices.len() {
            let mut selected_indices = Vec::new();
            if let Some(summary_idx) = summary_idx {
                selected_indices.push(summary_idx);
            }
            if let Some((latest_request_start, latest_request_end)) = latest_request_range {
                selected_indices.extend(latest_request_start..latest_request_end);
            }
            selected_indices.extend(remaining_indices.iter().skip(fifo_drop_count).copied());
            selected_indices.sort_unstable();
            selected_indices.dedup();

            let mut candidate = selected_indices
                .into_iter()
                .map(|index| body_messages[index].clone())
                .collect::<Vec<_>>();
            if let Some(instruction) = &instruction {
                candidate.push(instruction.clone());
            }

            if let Some((cleaned, _)) = candidate_compaction_total(
                &candidate,
                provider_id,
                safe_input_token_limit,
                system_prompt_tokens,
                tools_tokens,
                preserved_calibration_ratio,
            ) {
                return Ok(cleaned);
            }
        }
    }

    Err(if latest_real_user_request_range.is_some() {
        "Compaction overflow recovery could not fit the latest real user request anchor and any valid priority-preserving active-message FIFO subset within the effective context limit."
            .to_string()
    } else {
        "Compaction overflow recovery could not fit any valid priority-preserving active-message FIFO subset within the effective context limit."
            .to_string()
    })
}

pub(super) fn estimate_compaction_non_message_tokens(
    parent_request: Option<&CompactionParentRequest>,
) -> (usize, usize) {
    let system_prompt_tokens = parent_request
        .and_then(|request| request.system_prompt.as_ref())
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
