use std::sync::OnceLock;
use tiktoken_rs::{cl100k_base, CoreBPE};

/// Singleton tiktoken encoder for cl100k_base.
/// Creating/freeing the BPE encoder can be expensive. We keep one instance alive for the app lifetime.
static SHARED_ENCODER: OnceLock<CoreBPE> = OnceLock::new();

/// Gets a reference to the shared cl100k_base encoder, initializing it if necessary.
pub fn get_shared_encoder() -> Option<&'static CoreBPE> {
    Some(
        SHARED_ENCODER
            .get_or_init(|| cl100k_base().expect("Failed to initialize cl100k_base encoder")),
    )
}

/// Estimates the token count for arbitrary text using the `cl100k_base`
/// Byte-Pair Encoding (BPE), which is a common encoding for many modern LLMs.
/// Falls back to character-based estimation if tiktoken fails.
pub fn estimate_text_tokens(text: &str) -> usize {
    if let Some(encoder) = get_shared_encoder() {
        encoder.encode_with_special_tokens(text).len()
    } else {
        // Fallback: Use character-based estimation (~4 chars per token, conservative OpenAI estimate)
        log::debug!("tiktoken encoding failed, using character-based fallback");
        (text.len() as f64 / 4.0).ceil() as usize
    }
}

use crate::mcp::types::MCPContent;
use crate::models::chat::Message;

/// Calculates the threshold for compaction based on the effective limit.
/// Corresponds to `calculateCompactThreshold` in TS.
pub fn calculate_compact_threshold(effective_limit: usize) -> usize {
    (effective_limit as f64 * 0.9).floor() as usize
}

/// Estimates the token count for a given message using BPE or character fallback.
/// Translates `estimateTokensBPE` from TS.
pub fn estimate_tokens_bpe(message: &Message) -> usize {
    let mut parts = Vec::new();

    for c in &message.content {
        match c {
            MCPContent::Text { text, .. } => parts.push(text.clone()),
            MCPContent::Resource { resource, .. } => {
                if let Some(res_text) = resource.get("text").and_then(|v| v.as_str()) {
                    parts.push(res_text.to_string());
                }
            }
            MCPContent::ToolCall {
                name, arguments, ..
            } => {
                parts.push(name.clone());
                parts.push(arguments.clone());
            }
            MCPContent::Thinking { thinking, .. } => parts.push(thinking.clone()),
            _ => {
                // Ignore Image/Audio binary payload for tokens
            }
        }
    }

    if let Some(tool_calls) = &message.tool_calls {
        for tc in tool_calls {
            parts.push(tc.function.name.clone());
            parts.push(tc.function.arguments.clone());
        }
    }

    if let Some(tool_use) = &message.tool_use {
        if let Some(name) = tool_use.get("name").and_then(|v| v.as_str()) {
            parts.push(name.to_string());
        }
        if let Some(input) = tool_use.get("input") {
            parts.push(input.to_string());
        }
    }

    if let Some(thinking) = &message.thinking {
        parts.push(thinking.clone());
    }

    let text = format!("{}: {}", message.role, parts.join(" "));
    estimate_text_tokens(&text)
}

/// Calculates a grounded token estimate by finding the last assistant message with
/// valid API-reported usage (ground truth). Translates `calculateGroundedTotalTokens` from TS.
pub fn calculate_grounded_total_tokens(
    messages: &[Message],
    system_prompt_tokens: usize,
    tools_tokens: usize,
) -> usize {
    let mut grounded_index = None;
    let mut base_tokens = 0;

    // Search backwards for the most recent message with valid API usage
    for (i, msg) in messages.iter().enumerate().rev() {
        if msg.role == "assistant" {
            if let Some(usage) = &msg.usage {
                if let Some(total) = usage.get("totalTokens").and_then(|v| v.as_f64()) {
                    let total_usize = total as usize;
                    if total_usize > 0 {
                        grounded_index = Some(i);
                        base_tokens = total_usize;
                        break;
                    }
                }
            }
        }
    }

    if let Some(idx) = grounded_index {
        // Check if there is a summary message AFTER the grounded point.
        let has_summary_after_grounded = messages[idx + 1..]
            .iter()
            .any(|m| m.id.starts_with("compact-summary-"));

        if !has_summary_after_grounded {
            let mut incremental_tokens = 0;
            for msg in &messages[idx + 1..] {
                incremental_tokens += estimate_tokens_bpe(msg);
            }
            log::debug!(
                "Using grounded token estimation. base={}, inc={}, final={}",
                base_tokens,
                incremental_tokens,
                base_tokens + incremental_tokens
            );
            return base_tokens + incremental_tokens;
        }
    }

    // Fallback: Full BPE estimation
    let message_tokens: usize = messages.iter().map(estimate_tokens_bpe).sum();
    let full_estimate = message_tokens + system_prompt_tokens + tools_tokens;

    log::debug!(
        "Using full BPE token estimation. msg={}, sys={}, tools={}, final={}",
        message_tokens,
        system_prompt_tokens,
        tools_tokens,
        full_estimate
    );

    full_estimate
}
