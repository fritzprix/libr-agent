use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};
use tiktoken_rs::{cl100k_base, CoreBPE};

/// Singleton tiktoken encoder for cl100k_base.
/// Creating/freeing the BPE encoder can be expensive. We keep one instance alive for the app lifetime.
static SHARED_ENCODER: OnceLock<Option<CoreBPE>> = OnceLock::new();
static TOKEN_ESTIMATE_CACHE: OnceLock<Mutex<TokenEstimateCache>> = OnceLock::new();

const TOKEN_ESTIMATE_CACHE_MAX_ENTRIES: usize = 1024;
const TOKEN_ESTIMATE_CACHE_MAX_TEXT_BYTES: usize = 65_536;

#[derive(Default)]
struct TokenEstimateCache {
    entries: HashMap<String, usize>,
    order: VecDeque<String>,
}

impl TokenEstimateCache {
    fn get(&self, text: &str) -> Option<usize> {
        self.entries.get(text).copied()
    }

    fn insert(&mut self, text: String, tokens: usize) {
        if self.entries.contains_key(&text) {
            return;
        }

        while self.entries.len() >= TOKEN_ESTIMATE_CACHE_MAX_ENTRIES {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            self.entries.remove(&oldest);
        }

        self.order.push_back(text.clone());
        self.entries.insert(text, tokens);
    }
}

/// Gets a reference to the shared cl100k_base encoder, initializing it if necessary.
pub fn get_shared_encoder() -> Option<&'static CoreBPE> {
    SHARED_ENCODER.get_or_init(|| cl100k_base().ok()).as_ref()
}

fn get_token_estimate_cache() -> &'static Mutex<TokenEstimateCache> {
    TOKEN_ESTIMATE_CACHE.get_or_init(|| Mutex::new(TokenEstimateCache::default()))
}

fn estimate_text_tokens_uncached(text: &str) -> usize {
    if let Some(encoder) = get_shared_encoder() {
        encoder.encode_with_special_tokens(text).len()
    } else {
        // Fallback: Use character-based estimation (~4 chars per token, conservative OpenAI estimate)
        (text.len() as f64 / 4.0).ceil() as usize
    }
}

/// Estimates the token count for arbitrary text using the `cl100k_base`
/// Byte-Pair Encoding (BPE), which is a common encoding for many modern LLMs.
/// Falls back to character-based estimation if tiktoken fails.
pub fn estimate_text_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }

    if text.len() > TOKEN_ESTIMATE_CACHE_MAX_TEXT_BYTES {
        return estimate_text_tokens_uncached(text);
    }

    let cache = get_token_estimate_cache();
    let mut cache = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(tokens) = cache.get(text) {
        return tokens;
    }

    let tokens = estimate_text_tokens_uncached(text);
    cache.insert(text.to_string(), tokens);
    tokens
}

use crate::mcp::types::MCPContent;
use crate::models::chat::Message;

/// Reserves extra headroom for provider-side tokenization drift and frontend-side
/// payload expansion (attachments, context injection, multimodal wrappers).
pub fn calculate_context_safety_margin(effective_limit: usize) -> usize {
    let five_percent = (effective_limit as f64 * 0.05).ceil() as usize;
    five_percent.clamp(1024, 8192)
}

const CONSERVATIVE_DELTA_SAFETY_MULTIPLIER: f64 = 1.05;
// Cross-tokenizer providers like Gemini can legitimately report promptTokens
// around ~0.63 of the equivalent cl100k_base estimate, so the acceptance band
// must stay wide enough to preserve those grounded anchors.
pub const PROMPT_ANCHOR_RATIO_MIN: f64 = 0.50;
pub const PROMPT_ANCHOR_RATIO_MAX: f64 = 1.50;
const MIN_PROMPT_ANCHOR_PREFIX_MESSAGE_BPE: usize = 2_048;

#[derive(Debug)]
struct MessageTokenStats {
    per_message: Vec<usize>,
    prefix_sums: Vec<usize>,
    total_message_tokens: usize,
}

impl MessageTokenStats {
    fn from_messages(messages: &[Message]) -> Self {
        let mut per_message = Vec::with_capacity(messages.len());
        let mut prefix_sums = Vec::with_capacity(messages.len() + 1);
        let mut running_total = 0usize;
        prefix_sums.push(0);

        for message in messages {
            let tokens = estimate_tokens_bpe(message);
            running_total += tokens;
            per_message.push(tokens);
            prefix_sums.push(running_total);
        }

        Self {
            per_message,
            prefix_sums,
            total_message_tokens: running_total,
        }
    }

    fn tokens_before(&self, index: usize) -> usize {
        self.prefix_sums.get(index).copied().unwrap_or(0)
    }

    fn tokens_from(&self, index: usize) -> usize {
        self.total_message_tokens
            .saturating_sub(self.tokens_before(index))
    }
}

#[derive(Debug, Clone, Copy)]
struct PromptAnchorRatio {
    anchor_index: usize,
    prompt_tokens: usize,
    ratio: f64,
}

fn usage_metric_as_usize(usage: &serde_json::Value, key: &str) -> Option<usize> {
    usage
        .as_object()?
        .get(key)
        .and_then(|value| value.as_u64().map(|n| n as usize))
        .or_else(|| {
            usage
                .as_object()?
                .get(key)
                .and_then(|value| value.as_f64().map(|n| n as usize))
        })
}

fn find_latest_assistant_usage_anchor(messages: &[Message], key: &str) -> Option<(usize, usize)> {
    for (index, message) in messages.iter().enumerate().rev() {
        if message.role != "assistant" {
            continue;
        }

        if let Some(usage) = message.usage.as_ref() {
            if let Some(value) = usage_metric_as_usize(usage, key) {
                if value > 0 {
                    return Some((index, value));
                }
            }
        }
    }

    None
}

fn get_prompt_anchor_ratio(
    messages: &[Message],
    token_stats: &MessageTokenStats,
    system_prompt_tokens: usize,
    tools_tokens: usize,
) -> Option<PromptAnchorRatio> {
    for (anchor_index, anchor_message) in messages.iter().enumerate().rev() {
        if anchor_message.role != "assistant" {
            continue;
        }

        let Some(usage) = anchor_message.usage.as_ref() else {
            continue;
        };
        let Some(prompt_tokens) = usage_metric_as_usize(usage, "promptTokens") else {
            continue;
        };
        if prompt_tokens == 0 {
            continue;
        }

        let prefix_message_bpe = token_stats.tokens_before(anchor_index);
        let bpe_input = prefix_message_bpe + system_prompt_tokens + tools_tokens;
        let ratio = if bpe_input > 0 {
            prompt_tokens as f64 / bpe_input as f64
        } else {
            1.0
        };
        if prefix_message_bpe < MIN_PROMPT_ANCHOR_PREFIX_MESSAGE_BPE {
            continue;
        }

        if !(PROMPT_ANCHOR_RATIO_MIN..=PROMPT_ANCHOR_RATIO_MAX).contains(&ratio) {
            continue;
        }

        return Some(PromptAnchorRatio {
            anchor_index,
            prompt_tokens,
            ratio,
        });
    }

    None
}

fn normalize_calibration_ratio(ratio: f64) -> Option<f64> {
    if ratio.is_finite() && ratio > 0.0 {
        Some(ratio)
    } else {
        None
    }
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

    estimate_text_tokens(&format!("{}: {}", message.role, parts.join(" ")))
}

pub fn summarize_message_token_breakdown(messages: &[Message]) -> String {
    if messages.is_empty() {
        return "none".to_string();
    }

    let token_stats = MessageTokenStats::from_messages(messages);
    messages
        .iter()
        .zip(token_stats.per_message.iter())
        .map(|(message, tokens)| {
            format!(
                "{}:{}:bpe={}",
                message.role,
                shorten_message_id(&message.id),
                tokens
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn shorten_message_id(message_id: &str) -> &str {
    const SHORT_ID_LEN: usize = 12;
    if message_id.len() <= SHORT_ID_LEN {
        message_id
    } else {
        &message_id[..SHORT_ID_LEN]
    }
}

/// Calculates a grounded token estimate by finding the last assistant message with
/// valid API-reported usage (ground truth). Translates `calculateGroundedTotalTokens` from TS.
pub fn calculate_grounded_total_tokens(
    messages: &[Message],
    system_prompt_tokens: usize,
    tools_tokens: usize,
) -> usize {
    let token_stats = MessageTokenStats::from_messages(messages);
    if let Some((anchor_index, base_tokens)) =
        find_latest_assistant_usage_anchor(messages, "totalTokens")
    {
        let incremental_tokens = token_stats.tokens_from(anchor_index + 1);
        return base_tokens + incremental_tokens;
    }

    // Fallback: Full BPE estimation
    token_stats.total_message_tokens + system_prompt_tokens + tools_tokens
}

/// Derives the BPE-to-provider tokenizer calibration ratio from the most recent
/// API-grounded anchor.
///
/// Uses `usage.promptTokens` (the API-accurate pure-input token count) as the
/// numerator, and the local BPE estimate of that exact same input content as the
/// denominator. This produces a session-specific correction factor that adapts at
/// runtime to whichever provider tokenizer is actually in use.
///
/// For providers like Gemini (SentencePiece), `promptTokens` is typically ~37% lower
/// than the equivalent cl100k_base BPE count, yielding a ratio of ~0.63.
///
/// Using `promptTokens` rather than `totalTokens` keeps the ratio stable: the input
/// side grows monotonically and represents a large corpus, while `completionTokens`
/// swings wildly per turn and would add noise to the ratio.
///
/// Returns 1.0 when no valid anchor exists (no calibration possible on first turn).
pub fn try_derive_bpe_calibration_ratio(
    messages: &[Message],
    system_prompt_tokens: usize,
    tools_tokens: usize,
) -> Option<f64> {
    let token_stats = MessageTokenStats::from_messages(messages);
    if let Some(anchor) =
        get_prompt_anchor_ratio(messages, &token_stats, system_prompt_tokens, tools_tokens)
    {
        let ratio = anchor.ratio;
        return normalize_calibration_ratio(ratio);
    }
    None
}

pub fn derive_bpe_calibration_ratio(
    messages: &[Message],
    system_prompt_tokens: usize,
    tools_tokens: usize,
) -> f64 {
    try_derive_bpe_calibration_ratio(messages, system_prompt_tokens, tools_tokens).unwrap_or(1.0)
}

/// Estimates the current context token count anchored on the API-reported
/// `promptTokens` from the most recent grounded assistant message.
///
/// Formula (when anchor exists):
/// ```text
/// ratio    = promptTokens(anchor) / BPE(messages_before_anchor + sys + tools)
/// estimate = promptTokens(anchor) + BPE(messages[anchor_idx..]) * ratio
/// ```
///
/// - `promptTokens(anchor)` is the API-accurate count of all input fed to the model
///   at that turn — the error-free base.
/// - `messages[anchor_idx..]` covers the anchor's own assistant output plus every
///   message added since — the content whose size must be estimated.
/// - Applying `ratio` to that BPE corrects for the ~37% overcount of cl100k_base
///   relative to provider tokenizers such as Gemini SentencePiece.
///
/// Unlike anchoring on `totalTokens`, this approach derives the calibration ratio
/// purely from the input side, which is large and grows monotonically, giving a
/// more stable ratio than one that includes noisy per-turn `completionTokens`.
///
/// Falls back to full BPE when no grounded anchor is available.
pub fn calculate_prompt_anchored_total_tokens(
    messages: &[Message],
    system_prompt_tokens: usize,
    tools_tokens: usize,
) -> usize {
    let token_stats = MessageTokenStats::from_messages(messages);
    if let Some(anchor) =
        get_prompt_anchor_ratio(messages, &token_stats, system_prompt_tokens, tools_tokens)
    {
        // BPE of anchor's own output + all subsequent messages — everything added
        // after the model processed the anchor turn's input.
        let bpe_output = token_stats.tokens_from(anchor.anchor_index);
        let calibrated_output = (bpe_output as f64 * anchor.ratio).ceil() as usize;
        return anchor.prompt_tokens + calibrated_output;
    }

    // Fallback: full BPE — no API anchor available yet (e.g. first turn).
    token_stats.total_message_tokens + system_prompt_tokens + tools_tokens
}

/// Estimates the next request input size conservatively enough for a Rust-owned
/// pre-send hard gate.
///
/// When a grounded `promptTokens` anchor exists, that anchor already includes the
/// stable prompt prefix (compact summary, system prompt, session context, tool
/// schema, and selected history up to that turn). The only part we need to
/// estimate conservatively is the delta added after the anchor.
pub fn calculate_conservative_preflight_prompt_tokens(
    messages: &[Message],
    system_prompt_tokens: usize,
    tools_tokens: usize,
    fallback_calibration_ratio: Option<f64>,
) -> usize {
    let token_stats = MessageTokenStats::from_messages(messages);
    if let Some(anchor) =
        get_prompt_anchor_ratio(messages, &token_stats, system_prompt_tokens, tools_tokens)
    {
        let delta_bpe = token_stats.tokens_from(anchor.anchor_index);
        let conservative_delta = ((delta_bpe as f64 * anchor.ratio)
            * CONSERVATIVE_DELTA_SAFETY_MULTIPLIER)
            .ceil() as usize;
        return anchor.prompt_tokens + conservative_delta;
    }

    let full_estimate = token_stats.total_message_tokens + system_prompt_tokens + tools_tokens;
    if let Some(ratio) = fallback_calibration_ratio.and_then(normalize_calibration_ratio) {
        let calibrated_estimate = (full_estimate as f64 * ratio).ceil() as usize;
        let conservative_total =
            (calibrated_estimate as f64 * CONSERVATIVE_DELTA_SAFETY_MULTIPLIER).ceil() as usize;
        return conservative_total;
    }

    (full_estimate as f64 * CONSERVATIVE_DELTA_SAFETY_MULTIPLIER).ceil() as usize
}
