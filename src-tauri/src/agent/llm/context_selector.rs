use crate::agent::llm::token_utils;
use crate::models::chat::Message;

/// Minimal representation of model config needed for context selection
pub struct ModelContextInfo {
    pub context_window: usize,
}

/// Options for message selection
#[derive(Debug, Default)]
pub struct SelectionOptions {
    pub system_prompt: Option<String>,
    pub tools_json: Option<String>,
    pub max_messages: Option<usize>,
    pub max_tool_calls_per_message: Option<usize>,
}

/// Selected Context Result
#[derive(Debug)]
pub struct SelectedContext {
    pub messages: Vec<Message>,
}

/// Calculates the split index for compaction.
/// Returns `messages.len()` to compact ALL current messages. The natural "tail" is
/// whatever arrives AFTER `to_id` while the async summarization is in-flight —
/// tracked by `CompactRecord.to_id` in Step A of completion.rs.
pub fn find_compaction_split_index(messages: &[Message]) -> usize {
    messages.len()
}

/// Removes incomplete tool chains.
/// An incomplete chain is a `tool_calls` message without its corresponding `tool` result message.
pub fn remove_incomplete_tool_chains(messages: Vec<Message>) -> Vec<Message> {
    use std::collections::HashSet;

    let mut tool_use_ids = HashSet::new();
    let mut completed_tool_use_ids = HashSet::new();

    // First pass: collect all tool call IDs
    for msg in &messages {
        if msg.role == "assistant" {
            if let Some(tool_calls) = &msg.tool_calls {
                for tc in tool_calls {
                    tool_use_ids.insert(tc.id.clone());
                }
            }
        }
    }

    // Second pass: collect the IDs of tool calls that have a corresponding result
    for msg in &messages {
        if msg.role == "tool" {
            if let Some(tool_call_id) = &msg.tool_call_id {
                if tool_use_ids.contains(tool_call_id) {
                    completed_tool_use_ids.insert(tool_call_id.clone());
                }
            }
        }
    }

    // Third pass: build the result array, filtering out incomplete chains
    let mut result = Vec::with_capacity(messages.len());
    for msg in messages {
        if msg.role == "assistant" {
            if let Some(tool_calls) = &msg.tool_calls {
                let completed_tool_calls: Vec<_> = tool_calls
                    .iter()
                    .filter(|tc| completed_tool_use_ids.contains(&tc.id))
                    .cloned()
                    .collect();

                if !completed_tool_calls.is_empty() {
                    let mut processed_msg = msg.clone();
                    processed_msg.tool_calls = Some(completed_tool_calls);
                    result.push(processed_msg);
                } else {
                    let mut processed_msg = msg.clone();
                    processed_msg.tool_calls = None;
                    processed_msg.tool_use = None;
                    result.push(processed_msg);
                }
            } else {
                result.push(msg); // No tool calls, keep as is
            }
        } else if msg.role == "tool" {
            if let Some(tool_call_id) = &msg.tool_call_id {
                if completed_tool_use_ids.contains(tool_call_id) {
                    result.push(msg);
                }
            } else {
                result.push(msg); // Tool message without ID? keep it.
            }
        } else {
            // Keep all other messages
            result.push(msg);
        }
    }

    result
}

pub fn batch_tool_calls_in_messages(
    messages: &[Message],
    max_tool_calls_per_message: usize,
) -> Vec<Message> {
    let max_tool_calls = if max_tool_calls_per_message < 1 {
        4
    } else {
        max_tool_calls_per_message
    };

    let mut result = Vec::new();
    let mut processed_message_ids = std::collections::HashSet::new();

    for msg in messages {
        if processed_message_ids.contains(&msg.id) {
            continue;
        }

        let has_many_tool_calls = msg.role == "assistant"
            && msg.tool_calls.as_ref().map(|tc| tc.len()).unwrap_or(0) > max_tool_calls
            && msg.thinking_signature.is_none();

        if has_many_tool_calls {
            processed_message_ids.insert(msg.id.clone());

            let tool_calls = msg.tool_calls.as_ref().unwrap();
            let batches: Vec<_> = tool_calls.chunks(max_tool_calls).collect();
            let total_batches = batches.len();

            for (batch_index, batch) in batches.into_iter().enumerate() {
                let mut batch_msg = msg.clone();
                batch_msg.id = format!("{}_batch_{}", msg.id, batch_index);
                batch_msg.tool_calls = Some(batch.to_vec());

                if batch_index > 0 {
                    batch_msg.content = vec![crate::mcp::types::MCPContent::Text {
                        text: format!(
                            "[Continuing tool calls - Batch {}/{}]",
                            batch_index + 1,
                            total_batches
                        ),
                        is_error: None,
                    }];
                    batch_msg.thinking_signature = None;
                }

                result.push(batch_msg.clone());

                // Find and add corresponding tool responses
                let tool_call_ids: std::collections::HashSet<_> =
                    batch.iter().map(|tc| tc.id.clone()).collect();

                let batch_responses: Vec<_> = messages
                    .iter()
                    .filter(|m| {
                        m.role == "tool"
                            && m.tool_call_id
                                .as_ref()
                                .map(|id| tool_call_ids.contains(id))
                                .unwrap_or(false)
                    })
                    .cloned()
                    .collect();

                for r in &batch_responses {
                    processed_message_ids.insert(r.id.clone());
                }

                result.extend(batch_responses);
            }
        } else {
            result.push(msg.clone());
            processed_message_ids.insert(msg.id.clone());
        }
    }

    result
}

pub fn select_messages_within_context(
    messages: &[Message],
    provider_id: &str,
    max_tokens: Option<usize>,
    options: Option<&SelectionOptions>,
    model_info: Option<&ModelContextInfo>,
) -> Vec<Message> {
    let max_tool_calls = options
        .and_then(|o| o.max_tool_calls_per_message)
        .unwrap_or(4);
    let batched_messages = batch_tool_calls_in_messages(messages, max_tool_calls);

    let context_window = model_info.map(|m| m.context_window).unwrap_or(128_000);
    let base_token_limit =
        max_tokens.unwrap_or_else(|| (context_window as f64 * 0.9).floor() as usize);

    let system_prompt_tokens = options
        .and_then(|o| o.system_prompt.as_ref())
        .map(|s| token_utils::estimate_text_tokens(s))
        .unwrap_or(0);

    let tools_tokens = options
        .and_then(|o| o.tools_json.as_ref())
        .map(|s| token_utils::estimate_text_tokens(s))
        .unwrap_or(0);

    let mut pinned_message: Option<Message> = None;
    let mut pinned_message_tokens = 0;

    if !batched_messages.is_empty() && batched_messages[0].role == "user" {
        pinned_message = Some(batched_messages[0].clone());
        pinned_message_tokens = token_utils::estimate_tokens_bpe(&batched_messages[0]);
    }

    let non_message_reserved = system_prompt_tokens + tools_tokens;
    if non_message_reserved >= base_token_limit {
        log::warn!("System prompt + tools tokens exhaust the entire context window.");
        if let Some(most_recent) = batched_messages.last() {
            return vec![most_recent.clone()];
        } else {
            return vec![];
        }
    }

    // --- Token Calibration Multiplier ---
    let total_local_msg_tokens: usize = batched_messages
        .iter()
        .map(token_utils::estimate_tokens_bpe)
        .sum();
    let total_local_bpe = total_local_msg_tokens + non_message_reserved + pinned_message_tokens;

    let api_grounded_tokens = token_utils::calculate_grounded_total_tokens(
        &batched_messages,
        system_prompt_tokens,
        tools_tokens,
    );

    let calibration_ratio = if total_local_bpe > 0 {
        api_grounded_tokens as f64 / total_local_bpe as f64
    } else {
        1.0
    };

    log::debug!(
        "Calibration ratio for context selection: {:.4} (Grounded API: {}, Local BPE: {})",
        calibration_ratio,
        api_grounded_tokens,
        total_local_bpe
    );
    // ------------------------------------

    let reserved_tokens = non_message_reserved + pinned_message_tokens;
    let token_limit = std::cmp::max(1024, base_token_limit.saturating_sub(reserved_tokens));
    let pinned_message_budget = base_token_limit.saturating_sub(non_message_reserved);

    let mut total_tokens = 0;
    let mut selected = std::collections::VecDeque::new();

    let max_msgs = options.and_then(|o| o.max_messages);

    for msg in batched_messages.iter().rev() {
        if let Some(pinned) = &pinned_message {
            if msg.id == pinned.id {
                continue;
            }
        }

        let tokens = token_utils::estimate_tokens_bpe(msg);
        let calibrated_tokens = (tokens as f64 * calibration_ratio).ceil() as usize;

        if total_tokens + calibrated_tokens > token_limit {
            if ["anthropic", "gemini", "openai", "openrouter", "groq"].contains(&provider_id) {
                let adjusted = remove_incomplete_tool_chains(Vec::from(selected));
                return build_selected_with_optional_pinned(
                    pinned_message.clone(),
                    pinned_message_tokens,
                    pinned_message_budget,
                    adjusted,
                );
            }
            break;
        }

        let current_max = if pinned_message.is_some() {
            max_msgs.map(|m| m.saturating_sub(1))
        } else {
            max_msgs
        };

        if let Some(max) = current_max {
            if selected.len() >= max {
                if selected.is_empty() {
                    selected.push_front(msg.clone());
                }
                if ["anthropic", "gemini", "openai", "openrouter", "groq"].contains(&provider_id) {
                    let adjusted = remove_incomplete_tool_chains(Vec::from(selected));
                    return build_selected_with_optional_pinned(
                        pinned_message.clone(),
                        pinned_message_tokens,
                        pinned_message_budget,
                        adjusted,
                    );
                }
                break;
            }
        }

        selected.push_front(msg.clone());
        total_tokens += calibrated_tokens;
    }

    build_selected_with_optional_pinned(
        pinned_message,
        pinned_message_tokens,
        pinned_message_budget,
        Vec::from(selected),
    )
}

fn prepend_pinned_message(pinned_msg: Message, mut selected_msgs: Vec<Message>) -> Vec<Message> {
    if selected_msgs.is_empty() {
        return selected_msgs;
    }

    if pinned_msg.role == "user" && selected_msgs[0].role == "user" {
        let separator = crate::mcp::types::MCPContent::Text {
            text: "\n\n---\n\n(Merging context...)\n\n".to_string(),
            is_error: None,
        };

        let mut merged_msg = pinned_msg.clone();
        merged_msg.id = format!("merged_{}_{}", pinned_msg.id, selected_msgs[0].id);

        let mut new_content = merged_msg.content;
        new_content.push(separator);
        new_content.extend(selected_msgs[0].content.clone());

        merged_msg.content = new_content;

        selected_msgs.remove(0);
        selected_msgs.insert(0, merged_msg);
        return selected_msgs;
    }

    selected_msgs.insert(0, pinned_msg);
    selected_msgs
}

fn build_selected_with_optional_pinned(
    pinned_message: Option<Message>,
    pinned_message_tokens: usize,
    pinned_message_budget: usize,
    selected_msgs: Vec<Message>,
) -> Vec<Message> {
    if let Some(pinned) = pinned_message {
        if selected_msgs.is_empty() {
            if pinned_message_tokens <= pinned_message_budget {
                return vec![pinned];
            }
            return vec![];
        }

        return prepend_pinned_message(pinned, selected_msgs);
    }

    selected_msgs
}
