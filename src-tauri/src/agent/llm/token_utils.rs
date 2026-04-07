use std::sync::OnceLock;
use tiktoken_rs::{cl100k_base, CoreBPE};

/// Singleton tiktoken encoder for cl100k_base.
/// Creating/freeing the BPE encoder can be expensive. We keep one instance alive for the app lifetime.
static SHARED_ENCODER: OnceLock<Option<CoreBPE>> = OnceLock::new();

/// Gets a reference to the shared cl100k_base encoder, initializing it if necessary.
pub fn get_shared_encoder() -> Option<&'static CoreBPE> {
    SHARED_ENCODER.get_or_init(|| cl100k_base().ok()).as_ref()
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

/// Reserves extra headroom for provider-side tokenization drift and frontend-side
/// payload expansion (attachments, context injection, multimodal wrappers).
pub fn calculate_context_safety_margin(effective_limit: usize) -> usize {
    let five_percent = (effective_limit as f64 * 0.05).ceil() as usize;
    five_percent.clamp(1024, 8192)
}

pub fn calculate_usable_context_budget(effective_limit: usize) -> usize {
    (effective_limit as f64 * 0.9).floor() as usize
}

pub fn calculate_compact_target_budget(effective_limit: usize) -> usize {
    let ratio_target = (effective_limit as f64 * 0.125).floor() as usize;
    ratio_target.clamp(1024, 4096)
}

pub fn calculate_compact_hard_max_budget(effective_limit: usize) -> usize {
    let ratio_cap = (effective_limit as f64 * 0.25).floor() as usize;
    ratio_cap.clamp(1536, 6144)
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
            MCPContent::Image { .. } | MCPContent::Audio { .. } => {
                // Assign a conservative base token cost for media (e.g., 1000 tokens)
                // This ensures non-zero local BPE sums so the "calibration ratio" algorithm can scale it properly.
                parts.push(" ".repeat(1000));
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

pub fn estimate_message_selection_tokens(message: &Message) -> usize {
    let base_tokens = estimate_tokens_bpe(message);
    let mut multiplier = 1.0_f64;
    let mut additive = 0usize;

    if message.role == "tool" {
        multiplier = multiplier.max(1.25);
        additive += 64;
    }

    if message
        .tool_calls
        .as_ref()
        .is_some_and(|calls| !calls.is_empty())
    {
        multiplier = multiplier.max(1.2);
        additive += 48;
    }

    if message.tool_use.is_some() {
        multiplier = multiplier.max(1.15);
        additive += 48;
    }

    if message.attachments.is_some() {
        multiplier = multiplier.max(1.2);
        additive += 128;
    }

    if message.content.iter().any(|content| {
        matches!(
            content,
            MCPContent::Resource { .. } | MCPContent::ToolCall { .. }
        )
    }) {
        multiplier = multiplier.max(1.2);
        additive += 48;
    }

    if message
        .content
        .iter()
        .any(|content| matches!(content, MCPContent::Image { .. } | MCPContent::Audio { .. }))
    {
        multiplier = multiplier.max(1.35);
        additive += 256;
    }

    if message
        .content
        .iter()
        .any(|content| matches!(content, MCPContent::Text { text, .. } if text.contains("```")))
    {
        multiplier = multiplier.max(1.2);
    }

    ((base_tokens as f64) * multiplier).ceil() as usize + additive
}

/// Calculates a grounded token estimate by finding the last assistant message with
/// valid API-reported usage (ground truth). Translates `calculateGroundedTotalTokens` from TS.
pub fn calculate_grounded_total_tokens(
    messages: &[Message],
    system_prompt_tokens: usize,
    tools_tokens: usize,
) -> usize {
    let message_tokens: usize = messages.iter().map(estimate_tokens_bpe).sum();
    let full_estimate = message_tokens + system_prompt_tokens + tools_tokens;
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
        // Check if a compact summary appears BEFORE the grounded assistant.
        // After compaction, Step A rebuilds messages as [compact-summary, ...tail].
        // The compact-summary is always at index 0, so it will be at a position
        // before any assistant message — checking after the grounded point would
        // never find it and would cause the stale pre-compact usage value to be
        // used as the base, keeping the token estimate artificially high and
        // triggering repeated compaction.
        let has_summary_before_grounded = messages[..idx]
            .iter()
            .any(|m| m.id.starts_with("compact-summary-"));

        if !has_summary_before_grounded {
            let mut incremental_tokens = 0;
            for msg in &messages[idx + 1..] {
                incremental_tokens += estimate_tokens_bpe(msg);
            }
            let grounded_estimate = base_tokens + incremental_tokens;
            let final_estimate = grounded_estimate.max(full_estimate);
            log::debug!(
                "Using grounded token estimation. base={}, inc={}, grounded={}, full_bpe={}, final={}",
                base_tokens,
                incremental_tokens,
                grounded_estimate,
                full_estimate,
                final_estimate
            );
            return final_estimate;
        }
    }

    log::debug!(
        "Using full BPE token estimation. msg={}, sys={}, tools={}, final={}",
        message_tokens,
        system_prompt_tokens,
        tools_tokens,
        full_estimate
    );

    full_estimate
}
