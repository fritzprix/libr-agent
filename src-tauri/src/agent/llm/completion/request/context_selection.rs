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
) -> Result<bool, AgentRuntimeError> {
    crate::agent::llm::completion::compaction::try_trigger_preflight_compaction(
        active_sessions,
        app_handle,
        session_id,
        session_name,
        messages,
        parent_request,
        true,
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
) -> Option<f64> {
    crate::agent::llm::token_utils::try_derive_bpe_calibration_ratio(
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
    })
}

pub fn try_apply_lossy_main_request_fallback(
    messages: &[Message],
    provider: &str,
    safe_input_token_limit: usize,
    system_prompt_tokens: usize,
    tools_tokens: usize,
    fallback_calibration_ratio: Option<f64>,
) -> Option<Vec<Message>> {
    let mut lossy_messages =
        crate::agent::llm::context_selector::trim_messages_to_fit_conservative_limit(
            messages,
            provider,
            safe_input_token_limit,
            system_prompt_tokens,
            tools_tokens,
            fallback_calibration_ratio,
        );

    lossy_messages =
        crate::agent::llm::context_selector::truncate_single_oversized_message_to_fit_conservative_limit(
            &lossy_messages,
            safe_input_token_limit,
            system_prompt_tokens,
            tools_tokens,
            fallback_calibration_ratio,
        );

    if lossy_messages.is_empty() {
        return None;
    }

    let conservative_total =
        crate::agent::llm::token_utils::calculate_conservative_preflight_prompt_tokens(
            &lossy_messages,
            system_prompt_tokens,
            tools_tokens,
            fallback_calibration_ratio,
        );

    if conservative_total >= safe_input_token_limit {
        return None;
    }

    Some(lossy_messages)
}
