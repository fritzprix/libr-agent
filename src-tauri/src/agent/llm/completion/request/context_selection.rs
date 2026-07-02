use crate::agent::llm::types::{AgentRuntimeError, AgentRuntimeErrorType, CompactionParentRequest};
use crate::agent::state::AgentSession;
use crate::models::chat::Message;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

pub async fn trigger_preflight_compaction_for_messages_or_error(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    session_name: &str,
    messages: &[Message],
    parent_request: Option<CompactionParentRequest>,
    measured_output_tokens_reserve: usize,
) -> Result<bool, AgentRuntimeError> {
    crate::agent::llm::completion::compaction::try_trigger_preflight_compaction(
        active_sessions,
        app_handle,
        crate::agent::llm::completion::compaction::PreflightCompactionTriggerInput {
            session_id,
            session_name,
            messages,
            parent_request,
            measured_output_tokens_reserve,
            resume_completion_after_compact: true,
        },
    )
    .await
    .map_err(|error| {
        AgentRuntimeError::new(
            AgentRuntimeErrorType::AiServiceError,
            format!("Failed to trigger Rust preflight compaction: {}", error),
        )
        .with_code("PREFLIGHT_COMPACTION_TRIGGER_FAILED")
    })
}

pub fn build_compact_context_selection_options(
    system_prompt: Option<String>,
    tools_json: Option<String>,
    provider: &str,
    tool_call_group_visible_count: usize,
    fallback_calibration_ratio: Option<f64>,
) -> crate::agent::llm::context_selector::SelectionOptions {
    crate::agent::llm::context_selector::SelectionOptions {
        system_prompt,
        tools_json,
        max_messages: None,
        max_tool_calls_per_message: Some(if provider == "gemini" {
            100
        } else {
            tool_call_group_visible_count
        }),
        pin_first_user_message: false,
        fallback_calibration_ratio,
    }
}

pub fn resolve_preserved_calibration_ratio(
    raw_messages: &[Message],
    prompt_messages: &[Message],
    system_prompt_tokens: usize,
    tools_tokens: usize,
    provider: &str,
) -> Option<f64> {
    let ratio = crate::agent::llm::token_utils::try_derive_bpe_calibration_ratio(
        prompt_messages,
        system_prompt_tokens,
        tools_tokens,
    )
    .or_else(|| {
        crate::agent::llm::token_utils::try_derive_bpe_calibration_ratio(
            raw_messages,
            system_prompt_tokens,
            tools_tokens,
        )
    })?;

    // Invalidate if the ratio doesn't match the current provider's tokenizer family.
    // - "gemini" uses SentencePiece/Gemma-based tokenization which generally yields
    //   lower token counts compared to cl100k BPE, typically in the 0.55 to 0.75 range.
    // - Other providers (OpenAI, Anthropic, DeepSeek, etc.) use cl100k-based BPE
    //   tokenizers where BPE estimation matches the API's count closely (around 0.85 to 1.20).
    let is_valid = if provider == "gemini" {
        (0.55..=0.75).contains(&ratio)
    } else {
        (0.85..=1.20).contains(&ratio)
    };

    is_valid.then_some(ratio)
}
