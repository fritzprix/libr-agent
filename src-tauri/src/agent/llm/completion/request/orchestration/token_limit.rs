//! Token-limit enforcement for the final message selection.

use crate::agent::events::AgentEventDispatcher;
use crate::agent::llm::completion::context::{uses_compaction_strategy, ContextManagementSettings};
use crate::agent::llm::completion::request::context_selection::trigger_preflight_compaction_for_messages_or_error;
use crate::agent::llm::types::{AgentRuntimeError, AgentRuntimeErrorType, CompactionParentRequest};
use crate::agent::state::AgentSession;
use crate::agent::tauri_events::TauriEventDispatcher;
use crate::models::chat::Message;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

#[allow(clippy::too_many_arguments)]
pub(super) async fn check_token_limit(
    final_messages: Vec<Message>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    session_name: &str,
    context_settings: &ContextManagementSettings,
    system_prompt_tokens: usize,
    tools_tokens: usize,
    preserved_calibration_ratio: Option<f64>,
    compact_summary_injected: bool,
    normalized_messages: &[Message],
    compaction_parent_request: Option<CompactionParentRequest>,
    measured_output_tokens_reserve: usize,
) -> Result<Vec<Message>, AgentRuntimeError> {
    if !uses_compaction_strategy(&context_settings.context_strategy) {
        return Ok(final_messages);
    }

    let safe_input_token_limit = context_settings.max_input_context;
    let prompt_anchored_total_tokens =
        crate::agent::llm::token_utils::calculate_prompt_anchored_total_tokens(
            &final_messages,
            system_prompt_tokens,
            tools_tokens,
        );
    let conservative_preflight_tokens =
        crate::agent::llm::token_utils::calculate_conservative_preflight_prompt_tokens(
            &final_messages,
            system_prompt_tokens,
            tools_tokens,
            preserved_calibration_ratio,
        );
    let effective_input_budget = crate::agent::llm::token_utils::calculate_effective_input_budget(
        safe_input_token_limit,
        measured_output_tokens_reserve,
    );
    let total_budget_tokens =
        conservative_preflight_tokens.saturating_add(measured_output_tokens_reserve);
    let preflight_metrics = crate::agent::events::PreflightTokenMetrics {
        conservative_prompt_tokens: conservative_preflight_tokens,
        prompt_anchored_total_tokens,
        safe_input_token_limit,
        measured_output_tokens_reserve,
        effective_input_budget,
        total_budget_tokens,
        system_prompt_tokens,
        tools_tokens,
        selected_message_count: final_messages.len(),
        compact_summary_injected,
        preserved_calibration_ratio,
    };
    let event = crate::agent::events::AgentEvent::PreflightTokenMetricsUpdated {
        session_id: session_id.to_string(),
        metrics: preflight_metrics,
    };
    let _ = TauriEventDispatcher::from_handle(app_handle).emit_agent_event(event);

    if total_budget_tokens < safe_input_token_limit {
        return Ok(final_messages);
    }

    let selected_message_breakdown =
        crate::agent::llm::token_utils::summarize_message_token_breakdown(&final_messages);
    log::info!(
        "⛔ Rust preflight blocked LLM request: session={}, conservative_prompt_tokens={}, measured_output_tokens_reserve={}, total_budget_tokens={}, effective_input_budget={}, safe_input_token_limit={}, compact_summary_injected={}, selected_message_count={}, system_prompt_tokens={}, tools_tokens={}, preserved_calibration_ratio={}, selected_breakdown=[{}]",
        session_id,
        conservative_preflight_tokens,
        measured_output_tokens_reserve,
        total_budget_tokens,
        effective_input_budget,
        safe_input_token_limit,
        compact_summary_injected,
        final_messages.len(),
        system_prompt_tokens,
        tools_tokens,
        preserved_calibration_ratio
            .map(|ratio| format!("{:.4}", ratio))
            .unwrap_or_else(|| "none".to_string()),
        selected_message_breakdown
    );

    let compact_context_record = {
        let active = active_sessions.read().await;
        active
            .get(session_id)
            .map(|session| session.compact_context.clone())
    };
    let compact_context_record = match compact_context_record {
        Some(handle) => handle.read().await.clone(),
        None => None,
    };

    let compaction_limit = effective_input_budget
        .saturating_sub(system_prompt_tokens)
        .saturating_sub(tools_tokens)
        .saturating_sub(1500); // 1500 tokens for summary safety margin

    // INVALID_CONTEXT_STATE only when no ownership-safe *resume-fit* split exists.
    // Do not hard-fail merely because a prompt-token checkpoint is missing —
    // checkpoints seed candidates; resume fitness is the gate.
    // Spec: docs/specs/message-compaction.md §6.
    if !crate::agent::llm::completion::compaction::has_prompt_checkpoint_compaction_target(
        normalized_messages,
        compact_context_record.as_ref(),
        compaction_limit,
    ) {
        return Err(
            AgentRuntimeError::new(
                AgentRuntimeErrorType::ContextLimitError,
                "This session needs a larger context limit. Open Settings, raise Max Input Context, then try again.",
            )
            .with_code("INVALID_CONTEXT_STATE")
            .with_original_error(serde_json::json!({
                "sessionId": session_id,
                "conservativePromptTokens": conservative_preflight_tokens,
                "measuredOutputTokensReserve": measured_output_tokens_reserve,
                "totalBudgetTokens": total_budget_tokens,
                "effectiveInputBudget": effective_input_budget,
                "safeInputTokenLimit": safe_input_token_limit,
                "compactSummaryInjected": compact_summary_injected,
                "selectedMessageCount": final_messages.len(),
                "systemPromptTokens": system_prompt_tokens,
                "toolsTokens": tools_tokens,
                "preservedCalibrationRatio": preserved_calibration_ratio,
                "selectedBreakdown": selected_message_breakdown,
                "normalPathLossyFallbackAllowed": false,
                // Historical field: kept false so clients do not reintroduce a
                // checkpoint-only hard gate. Resume-fit is authoritative.
                "requiresPromptTokenCheckpoint": false,
                "requiresOwnershipSafeResumeFitSplit": true,
                "technicalMessage": "Prepared payload exceeds the effective context limit, but there is no ownership-safe compaction split that can reduce the live prompt. This session state is invalid and must not be committed.",
            })),
        );
    }

    if trigger_preflight_compaction_for_messages_or_error(
        active_sessions,
        app_handle,
        session_id,
        session_name,
        normalized_messages,
        compaction_parent_request.clone(),
        measured_output_tokens_reserve,
    )
    .await?
    {
        return Err(AgentRuntimeError::new(
            AgentRuntimeErrorType::AiServiceError,
            "Compaction triggered",
        )
        .with_code("COMPACTION_TRIGGERED_OK"));
    }

    Err(
        AgentRuntimeError::new(
            AgentRuntimeErrorType::ContextLimitError,
            format!(
                "Prepared payload exceeds the effective context limit before send ({} total budget tokens >= {} input limit; conservative input={}, reserved output={}), and normal compact-mode requests must not be silently trimmed. Trigger preflight compaction or fail explicitly.",
                total_budget_tokens,
                safe_input_token_limit,
                conservative_preflight_tokens,
                measured_output_tokens_reserve
            ),
        )
        .with_code("RUST_PREFLIGHT_CONTEXT_LIMIT")
        .with_original_error(serde_json::json!({
            "sessionId": session_id,
            "conservativePromptTokens": conservative_preflight_tokens,
            "measuredOutputTokensReserve": measured_output_tokens_reserve,
            "totalBudgetTokens": total_budget_tokens,
            "effectiveInputBudget": effective_input_budget,
            "safeInputTokenLimit": safe_input_token_limit,
            "compactSummaryInjected": compact_summary_injected,
            "selectedMessageCount": final_messages.len(),
            "systemPromptTokens": system_prompt_tokens,
            "toolsTokens": tools_tokens,
            "preservedCalibrationRatio": preserved_calibration_ratio,
            "selectedBreakdown": selected_message_breakdown,
            "normalPathLossyFallbackAllowed": false,
        })),
    )
}
