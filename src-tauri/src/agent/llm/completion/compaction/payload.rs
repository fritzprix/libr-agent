use crate::agent::llm::completion::request::build_compact_summary_message_for_messages;
use crate::agent::llm::types::CompactionParentRequest;
use crate::models::chat::Message;
use crate::repositories::CompactContextRecord;

use super::instruction::{build_compaction_instruction, build_compaction_instruction_message};

pub(super) struct CompactionRequestPayload {
    pub(super) compact_messages: Vec<Message>,
    pub(super) from_id: String,
    pub(super) to_id: String,
    pub(super) compacted_delta_count: usize,
    pub(super) reused_prior_summary: bool,
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
                from_id: record.from_id.clone(),
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
        from_id: messages
            .first()
            .map(|message| message.id.clone())
            .unwrap_or_default(),
        to_id: messages[split_idx - 1].id.clone(),
        compacted_delta_count: split_idx,
        reused_prior_summary: false,
    })
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

pub(super) fn estimate_compaction_non_message_tokens(
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
