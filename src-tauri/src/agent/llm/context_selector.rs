use crate::agent::llm::token_utils;
use crate::models::chat::Message;
use serde_json::json;
use std::collections::{HashMap, HashSet, VecDeque};

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
    pub pin_first_user_message: bool,
}

/// Selected Context Result
#[derive(Debug)]
pub struct SelectedContext {
    pub messages: Vec<Message>,
}

const LOSSY_TRIM_MIN_MESSAGE_BUDGET: usize = 128;
const LOSSY_TRIM_TOOL_ARGUMENT_BUDGET: usize = 96;
const LOSSY_TRIM_THINKING_BUDGET: usize = 160;

fn provider_needs_tool_cleanup(provider_id: &str) -> bool {
    ["anthropic", "gemini", "openai", "openrouter", "groq"].contains(&provider_id)
}

fn build_trim_placeholder(label: &str, removed_tokens: usize) -> String {
    format!(
        "...[trimmed middle of {}; removed ~{} estimated tokens]...",
        label, removed_tokens
    )
}

fn trim_text_middle(text: &str, budget_tokens: usize, label: &str) -> Option<String> {
    let normalized = text.trim();
    if normalized.is_empty() {
        return Some(String::new());
    }

    if token_utils::estimate_text_tokens(normalized) <= budget_tokens {
        return Some(normalized.to_string());
    }

    let placeholder = build_trim_placeholder(label, token_utils::estimate_text_tokens(normalized));
    if token_utils::estimate_text_tokens(&placeholder) > budget_tokens {
        return None;
    }

    let chars = normalized.chars().collect::<Vec<_>>();
    let max_anchor = chars.len() / 2;
    let mut low = 0usize;
    let mut high = max_anchor;
    let mut best = placeholder.clone();

    while low <= high {
        let anchor_len = (low + high) / 2;
        let head = chars.iter().take(anchor_len).copied().collect::<String>();
        let tail = chars
            .iter()
            .rev()
            .take(anchor_len)
            .copied()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<String>();
        let kept_tokens = token_utils::estimate_text_tokens(&(head.clone() + &tail));
        let removed_tokens =
            token_utils::estimate_text_tokens(normalized).saturating_sub(kept_tokens);
        let candidate = if anchor_len == 0 {
            build_trim_placeholder(label, removed_tokens)
        } else {
            format!(
                "{}\n{}\n{}",
                head,
                build_trim_placeholder(label, removed_tokens),
                tail
            )
        };
        let candidate_tokens = token_utils::estimate_text_tokens(&candidate);
        if candidate_tokens <= budget_tokens {
            best = candidate;
            low = anchor_len.saturating_add(1);
        } else {
            high = anchor_len.saturating_sub(1);
        }
    }

    Some(best)
}

fn content_to_trimmed_text(content: &[crate::mcp::types::MCPContent]) -> String {
    let mut parts = Vec::new();
    for item in content {
        match item {
            crate::mcp::types::MCPContent::Text { text, .. } => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed.to_string());
                }
            }
            crate::mcp::types::MCPContent::Resource { resource, .. } => {
                if let Some(text) = resource.get("text").and_then(|value| value.as_str()) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        parts.push(trimmed.to_string());
                    }
                } else {
                    let mime_type = resource
                        .get("mimeType")
                        .and_then(|value| value.as_str())
                        .unwrap_or("resource");
                    parts.push(format!("[resource:{}]", mime_type));
                }
            }
            crate::mcp::types::MCPContent::Image { .. } => {
                parts.push("[image content omitted during lossy trim]".to_string());
            }
            crate::mcp::types::MCPContent::Audio { .. } => {
                parts.push("[audio content omitted during lossy trim]".to_string());
            }
            crate::mcp::types::MCPContent::Thinking { thinking, .. } => {
                let trimmed = thinking.trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed.to_string());
                }
            }
            crate::mcp::types::MCPContent::ToolCall { name, .. } => {
                parts.push(format!("[tool call:{}]", name));
            }
        }
    }

    if parts.is_empty() {
        "[content omitted during lossy trim]".to_string()
    } else {
        parts.join("\n")
    }
}

fn trim_tool_arguments_json(arguments: &str, budget_tokens: usize) -> String {
    if token_utils::estimate_text_tokens(arguments) <= budget_tokens {
        return arguments.to_string();
    }

    json!({
        "_lossyTrimmed": true,
        "_note": build_trim_placeholder(
            "tool arguments",
            token_utils::estimate_text_tokens(arguments)
        ),
    })
    .to_string()
}

fn lossy_trim_message_to_budget(message: &Message, budget_tokens: usize) -> Option<Message> {
    if budget_tokens < LOSSY_TRIM_MIN_MESSAGE_BUDGET {
        return None;
    }

    if token_utils::estimate_message_selection_tokens(message) <= budget_tokens {
        return Some(message.clone());
    }

    let mut candidate = message.clone();
    let mut trim_notes = Vec::new();

    let had_multimodal = candidate.content.iter().any(|content| {
        matches!(
            content,
            crate::mcp::types::MCPContent::Image { .. }
                | crate::mcp::types::MCPContent::Audio { .. }
        )
    });
    if had_multimodal {
        trim_notes.push("[multimodal content omitted during lossy trim]".to_string());
    }
    candidate.content.retain(|content| {
        !matches!(
            content,
            crate::mcp::types::MCPContent::Image { .. }
                | crate::mcp::types::MCPContent::Audio { .. }
        )
    });

    if candidate.attachments.is_some() {
        candidate.attachments = None;
        trim_notes.push("[attachments omitted during lossy trim]".to_string());
    }

    if let Some(thinking) = &candidate.thinking {
        let thinking_tokens = token_utils::estimate_text_tokens(thinking);
        if thinking_tokens > LOSSY_TRIM_THINKING_BUDGET {
            candidate.thinking = None;
            candidate.thinking_signature = None;
            trim_notes.push(build_trim_placeholder("thinking block", thinking_tokens));
        }
    }

    if let Some(tool_calls) = &candidate.tool_calls {
        let mut trimmed_tool_calls = tool_calls.clone();
        for tool_call in &mut trimmed_tool_calls {
            tool_call.function.arguments = trim_tool_arguments_json(
                &tool_call.function.arguments,
                LOSSY_TRIM_TOOL_ARGUMENT_BUDGET,
            );
        }
        candidate.tool_calls = Some(trimmed_tool_calls);
    }

    if let Some(tool_use) = &candidate.tool_use {
        let tool_name = tool_use
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or("tool");
        candidate.tool_use = Some(json!({
            "name": tool_name,
            "input": {
                "_lossyTrimmed": true,
                "_note": "[tool input omitted during lossy trim]"
            }
        }));
    }

    let structured_candidate = candidate.clone();
    let structural_tokens = {
        let mut baseline = structured_candidate.clone();
        baseline.content = Vec::new();
        baseline.thinking = None;
        baseline.thinking_signature = None;
        token_utils::estimate_message_selection_tokens(&baseline)
    };

    let available_text_budget = budget_tokens.saturating_sub(structural_tokens).max(32);
    let mut flattened_text = content_to_trimmed_text(&candidate.content);
    if !trim_notes.is_empty() {
        flattened_text = format!("{}\n{}", trim_notes.join("\n"), flattened_text);
    }

    let trimmed_text = trim_text_middle(&flattened_text, available_text_budget, "message content")?;
    candidate.content = vec![crate::mcp::types::MCPContent::Text {
        text: trimmed_text,
        is_error: None,
    }];

    if token_utils::estimate_message_selection_tokens(&candidate) <= budget_tokens {
        return Some(candidate);
    }

    let minimal_text = trim_text_middle(
        &flattened_text,
        budget_tokens.saturating_sub(structural_tokens).max(16),
        "message content",
    )?;
    candidate.content = vec![crate::mcp::types::MCPContent::Text {
        text: minimal_text,
        is_error: None,
    }];

    if token_utils::estimate_message_selection_tokens(&candidate) <= budget_tokens {
        Some(candidate)
    } else {
        None
    }
}

fn build_selection_blocks(messages: &[Message]) -> Vec<Vec<Message>> {
    let mut blocks = Vec::new();
    let mut index = 0usize;

    while index < messages.len() {
        let message = &messages[index];
        if message.role == "assistant"
            && message
                .tool_calls
                .as_ref()
                .is_some_and(|tool_calls| !tool_calls.is_empty())
        {
            let mut block = vec![message.clone()];
            let tool_call_ids = message
                .tool_calls
                .as_ref()
                .map(|tool_calls| {
                    tool_calls
                        .iter()
                        .map(|tool_call| tool_call.id.clone())
                        .collect::<HashSet<_>>()
                })
                .unwrap_or_default();
            let mut cursor = index + 1;

            while cursor < messages.len() {
                let next = &messages[cursor];
                let matches_tool_chain = next.role == "tool"
                    && next
                        .tool_call_id
                        .as_ref()
                        .is_some_and(|tool_call_id| tool_call_ids.contains(tool_call_id));
                if !matches_tool_chain {
                    break;
                }
                block.push(next.clone());
                cursor += 1;
            }

            if cursor < messages.len() {
                let next = &messages[cursor];
                if next.role == "assistant"
                    && next
                        .tool_calls
                        .as_ref()
                        .is_none_or(|tool_calls| tool_calls.is_empty())
                {
                    block.push(next.clone());
                    cursor += 1;
                }
            }

            blocks.push(block);
            index = cursor;
            continue;
        }

        blocks.push(vec![message.clone()]);
        index += 1;
    }

    blocks
}

fn flatten_selection_blocks(blocks: Vec<Vec<Message>>) -> Vec<Message> {
    blocks.into_iter().flatten().collect()
}

fn estimate_block_tokens(block: &[Message]) -> usize {
    block
        .iter()
        .map(token_utils::estimate_message_selection_tokens)
        .sum()
}

fn lossy_trim_block_to_budget(block: &[Message], budget_tokens: usize) -> Option<Vec<Message>> {
    if block.is_empty() {
        return Some(Vec::new());
    }

    if estimate_block_tokens(block) <= budget_tokens {
        return Some(block.to_vec());
    }

    let mut trimmed_block = block.to_vec();

    loop {
        let total_tokens = estimate_block_tokens(&trimmed_block);
        if total_tokens <= budget_tokens {
            return Some(trimmed_block);
        }

        let mut largest_index = None;
        let mut largest_tokens = 0usize;
        for (index, message) in trimmed_block.iter().enumerate() {
            let estimated = token_utils::estimate_message_selection_tokens(message);
            if estimated > largest_tokens {
                largest_tokens = estimated;
                largest_index = Some(index);
            }
        }

        let index = largest_index?;

        let other_tokens = total_tokens.saturating_sub(largest_tokens);
        let available_for_message = budget_tokens
            .saturating_sub(other_tokens)
            .max(LOSSY_TRIM_MIN_MESSAGE_BUDGET);

        if available_for_message >= largest_tokens {
            return None;
        }

        let trimmed_message =
            lossy_trim_message_to_budget(&trimmed_block[index], available_for_message)?;
        let trimmed_tokens = token_utils::estimate_message_selection_tokens(&trimmed_message);
        if trimmed_tokens >= largest_tokens {
            return None;
        }

        trimmed_block[index] = trimmed_message;
    }
}

/// Calculates the split index for compaction.
/// Returns the earliest unresolved assistant tool-call boundary, or `messages.len()`
/// when the stack contains no in-flight tool chains. This prevents async compaction
/// from swallowing an assistant tool-call message and leaving future tool results
/// orphaned behind the compact summary.
pub fn find_compaction_split_index(messages: &[Message]) -> usize {
    let mut tool_call_owner: HashMap<String, usize> = HashMap::new();
    let mut open_tool_counts: HashMap<usize, usize> = HashMap::new();

    for (idx, msg) in messages.iter().enumerate() {
        if msg.role == "assistant" {
            if let Some(tool_calls) = &msg.tool_calls {
                if !tool_calls.is_empty() {
                    open_tool_counts.insert(idx, tool_calls.len());
                    for tool_call in tool_calls {
                        tool_call_owner.insert(tool_call.id.clone(), idx);
                    }
                }
            }
            continue;
        }

        if msg.role != "tool" {
            continue;
        }

        let Some(tool_call_id) = &msg.tool_call_id else {
            continue;
        };

        let Some(owner_idx) = tool_call_owner.remove(tool_call_id) else {
            continue;
        };

        if let Some(open_count) = open_tool_counts.get_mut(&owner_idx) {
            *open_count = open_count.saturating_sub(1);
            if *open_count == 0 {
                open_tool_counts.remove(&owner_idx);
            }
        }
    }

    open_tool_counts
        .keys()
        .min()
        .copied()
        .unwrap_or(messages.len())
}

/// Removes incomplete tool chains.
/// An incomplete chain is a `tool_calls` message without its corresponding `tool` result message.
pub fn remove_incomplete_tool_chains(messages: Vec<Message>) -> Vec<Message> {
    use std::collections::{HashMap, HashSet};

    let mut tool_use_ids = HashSet::new();
    let mut completed_tool_use_ids = HashSet::new();
    let mut tool_call_owner_indices = HashMap::new();
    let mut unresolved_owner_counts = HashMap::new();
    let mut first_orphan_tool_index: Option<usize> = None;

    // First pass: collect all tool call IDs
    for (idx, msg) in messages.iter().enumerate() {
        if msg.role == "assistant" {
            if let Some(tool_calls) = &msg.tool_calls {
                for tc in tool_calls {
                    tool_use_ids.insert(tc.id.clone());
                    tool_call_owner_indices.insert(tc.id.clone(), idx);
                    *unresolved_owner_counts.entry(idx).or_insert(0usize) += 1;
                }
            }
        }
    }

    // Second pass: collect the IDs of tool calls that have a corresponding result
    for (idx, msg) in messages.iter().enumerate() {
        if msg.role == "tool" {
            if let Some(tool_call_id) = &msg.tool_call_id {
                if tool_use_ids.contains(tool_call_id) {
                    completed_tool_use_ids.insert(tool_call_id.clone());
                    if let Some(owner_idx) = tool_call_owner_indices.get(tool_call_id) {
                        if let Some(open_count) = unresolved_owner_counts.get_mut(owner_idx) {
                            *open_count = open_count.saturating_sub(1);
                            if *open_count == 0 {
                                unresolved_owner_counts.remove(owner_idx);
                            }
                        }
                    }
                } else if first_orphan_tool_index.is_none() {
                    first_orphan_tool_index = Some(idx);
                }
            }
        }
    }

    let first_unstable_index = unresolved_owner_counts
        .keys()
        .copied()
        .chain(first_orphan_tool_index)
        .min();

    let Some(first_unstable_index) = first_unstable_index else {
        return messages;
    };

    // Third pass: build the result array, filtering out incomplete chains
    let mut result = Vec::with_capacity(messages.len());
    for (idx, msg) in messages.into_iter().enumerate() {
        if idx < first_unstable_index {
            result.push(msg);
            continue;
        }

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
    let mut processed_message_ids = HashSet::new();

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
                let tool_call_ids: HashSet<_> = batch.iter().map(|tc| tc.id.clone()).collect();

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

pub fn select_recent_messages_fifo(
    messages: &[Message],
    provider_id: &str,
    max_messages: usize,
    max_tool_calls_per_message: usize,
) -> Vec<Message> {
    if max_messages == 0 {
        return Vec::new();
    }

    let batched_messages = batch_tool_calls_in_messages(messages, max_tool_calls_per_message);
    let start_idx = batched_messages.len().saturating_sub(max_messages);
    let selected = batched_messages[start_idx..].to_vec();

    let adjusted = if ["anthropic", "gemini", "openai", "openrouter", "groq"].contains(&provider_id)
    {
        remove_incomplete_tool_chains(selected)
    } else {
        selected
    };

    if adjusted.is_empty() {
        if let Some(latest_non_tool) = batched_messages.iter().rev().find(|msg| msg.role != "tool")
        {
            return vec![latest_non_tool.clone()];
        }
    }

    adjusted
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
    let provider_requires_cleanup = provider_needs_tool_cleanup(provider_id);

    let context_window = model_info.map(|m| m.context_window).unwrap_or(128_000);
    let base_token_limit =
        max_tokens.unwrap_or_else(|| token_utils::calculate_usable_context_budget(context_window));

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

    if options.is_some_and(|selection| selection.pin_first_user_message)
        && !batched_messages.is_empty()
        && batched_messages[0].role == "user"
    {
        pinned_message = Some(batched_messages[0].clone());
        pinned_message_tokens =
            token_utils::estimate_message_selection_tokens(&batched_messages[0]);
    }

    let selection_candidates = if pinned_message.is_some() {
        &batched_messages[1..]
    } else {
        &batched_messages[..]
    };
    let selection_blocks = build_selection_blocks(selection_candidates);

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
        .map(token_utils::estimate_message_selection_tokens)
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
    let mut selected_blocks = VecDeque::new();

    let max_msgs = options.and_then(|o| o.max_messages);

    for block in selection_blocks.iter().rev() {
        let tokens = estimate_block_tokens(block);
        let calibrated_tokens = (tokens as f64 * calibration_ratio).ceil() as usize;

        if total_tokens + calibrated_tokens > token_limit {
            if selected_blocks.is_empty() {
                let remaining_budget = token_limit.saturating_sub(total_tokens);
                if let Some(trimmed_block) = lossy_trim_block_to_budget(block, remaining_budget) {
                    selected_blocks.push_front(trimmed_block);
                } else {
                    selected_blocks.push_front(block.clone());
                }
            }

            let selected_messages =
                flatten_selection_blocks(selected_blocks.iter().cloned().collect());
            if provider_requires_cleanup {
                let adjusted = remove_incomplete_tool_chains(selected_messages);
                return build_selected_with_optional_pinned(
                    pinned_message.clone(),
                    pinned_message_tokens,
                    pinned_message_budget,
                    adjusted,
                );
            }
            return build_selected_with_optional_pinned(
                pinned_message.clone(),
                pinned_message_tokens,
                pinned_message_budget,
                selected_messages,
            );
        }

        let current_max = if pinned_message.is_some() {
            max_msgs.map(|m| m.saturating_sub(1))
        } else {
            max_msgs
        };
        let current_message_count: usize = selected_blocks.iter().map(Vec::len).sum();

        if let Some(max) = current_max {
            if current_message_count >= max {
                if selected_blocks.is_empty() {
                    selected_blocks.push_front(block.clone());
                }
                let selected_messages =
                    flatten_selection_blocks(selected_blocks.iter().cloned().collect());
                if provider_requires_cleanup {
                    let adjusted = remove_incomplete_tool_chains(selected_messages);
                    return build_selected_with_optional_pinned(
                        pinned_message.clone(),
                        pinned_message_tokens,
                        pinned_message_budget,
                        adjusted,
                    );
                }
                return build_selected_with_optional_pinned(
                    pinned_message.clone(),
                    pinned_message_tokens,
                    pinned_message_budget,
                    selected_messages,
                );
            }
        }

        selected_blocks.push_front(block.clone());
        total_tokens += calibrated_tokens;
    }

    let final_selected = flatten_selection_blocks(selected_blocks.into_iter().collect());
    let adjusted = if provider_requires_cleanup {
        remove_incomplete_tool_chains(final_selected)
    } else {
        final_selected
    };

    build_selected_with_optional_pinned(
        pinned_message,
        pinned_message_tokens,
        pinned_message_budget,
        adjusted,
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
            if let Some(trimmed_pinned) =
                lossy_trim_message_to_budget(&pinned, pinned_message_budget)
            {
                return vec![trimmed_pinned];
            }
            return vec![];
        }

        return prepend_pinned_message(pinned, selected_msgs);
    }

    selected_msgs
}
