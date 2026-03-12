use crate::agent::llm::types::CompletionRequest;
use crate::mcp::types::{MCPContent, MCPTool};
use crate::models::chat::Message;
use crate::repositories::settings_repository::SettingsRepository;
use crate::repositories::CompactContextRecord;
use crate::state::get_settings_repository;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::OnceLock;
use tauri::Emitter;

const DEFAULT_CONTEXT_WINDOW: usize = 64 * 1024;
const DEFAULT_MAX_OUTPUT_TOKENS: usize = 8192;
pub const MAX_COMPACTION_RETRIES: u8 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextUsageSummary {
    pub total_tokens: usize,
    pub context_window: usize,
    pub model_max_context: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactedRange {
    pub from_id: String,
    pub to_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionStateEvent {
    pub session_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_usage: Option<ContextUsageSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compacted_range: Option<CompactedRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionRequest {
    pub request_id: String,
    pub session_id: String,
    pub messages: Vec<Message>,
    pub model: String,
    pub provider: String,
}

#[derive(Debug, Clone)]
pub struct PendingCompactionRequest {
    pub request: CompactionRequest,
    pub from_id: String,
    pub to_id: String,
    pub retry_count: u8,
    pub context_usage: ContextUsageSummary,
    pub compacted_range: Option<CompactedRange>,
}

#[derive(Debug, Clone)]
pub struct PreparedCompletion {
    pub request: CompletionRequest,
    pub state: Option<CompactionStateEvent>,
    pub invalidate_compact_record: bool,
}

#[derive(Debug, Clone)]
pub struct PendingCompaction {
    pub request: CompactionRequest,
    pub pending: PendingCompactionRequest,
    pub state: CompactionStateEvent,
    pub invalidate_compact_record: bool,
}

pub enum CompletionPreparation {
    Ready(PreparedCompletion),
    NeedsCompaction(PendingCompaction),
}

#[derive(Debug)]
struct CompactionSettings {
    context_strategy: String,
    max_input_context: Option<usize>,
    default_max_output_tokens: usize,
}

#[derive(Debug)]
struct CandidateStack {
    messages: Vec<Message>,
    summary: Option<String>,
    compacted_range: Option<CompactedRange>,
    stale_record: bool,
}

pub struct PrepareCompletionInput {
    pub session_id: String,
    pub messages: Vec<Message>,
    pub model: String,
    pub provider: String,
    pub system_prompt: Option<String>,
    pub session_context: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub available_tools: Option<Vec<MCPTool>>,
    pub compact_record: Option<CompactContextRecord>,
}

pub async fn prepare_completion_request(
    input: PrepareCompletionInput,
) -> Result<CompletionPreparation, String> {
    let settings = load_compaction_settings().await?;
    let PrepareCompletionInput {
        session_id,
        messages,
        model,
        provider,
        system_prompt,
        session_context,
        temperature,
        max_tokens,
        available_tools,
        compact_record,
    } = input;

    if settings.context_strategy != "compact" {
        return Ok(CompletionPreparation::Ready(PreparedCompletion {
            request: CompletionRequest {
                session_id,
                messages,
                model,
                provider,
                system_prompt,
                session_context,
                temperature,
                max_tokens,
                available_tools,
                backend_owned_compaction: true,
            },
            state: None,
            invalidate_compact_record: false,
        }));
    }

    let candidate = build_candidate_stack(messages, compact_record.as_ref());
    let base_system_prompt = system_prompt.unwrap_or_default();
    let final_system_prompt = match &candidate.summary {
        Some(summary) => format!("{}\n\n{}", base_system_prompt, summary)
            .trim()
            .to_string(),
        None => base_system_prompt,
    };

    let system_prompt_tokens = estimate_text_tokens(&final_system_prompt);
    let session_context_tokens = session_context
        .as_deref()
        .map(estimate_text_tokens)
        .unwrap_or(0);
    let tools_json = available_tools
        .as_ref()
        .and_then(|tools| serde_json::to_string(tools).ok());
    let tools_tokens = tools_json.as_deref().map(estimate_text_tokens).unwrap_or(0);

    let model_max_limit =
        lookup_context_window(&provider, &model).unwrap_or(DEFAULT_CONTEXT_WINDOW);
    let reserved_output_tokens = max_tokens
        .map(|value| value as usize)
        .unwrap_or(settings.default_max_output_tokens);
    let (effective_limit, model_max_context) = calculate_effective_context_limit(
        model_max_limit,
        reserved_output_tokens,
        settings.max_input_context,
    );

    let total_tokens = estimate_grounded_total_tokens(
        &candidate.messages,
        system_prompt_tokens + session_context_tokens,
        tools_tokens,
    );

    let usage = ContextUsageSummary {
        total_tokens,
        context_window: effective_limit,
        model_max_context: Some(model_max_context),
    };
    let threshold = ((effective_limit as f64) * 0.9).floor() as usize;

    if total_tokens >= threshold {
        // Compact the entire candidate message stack — no arbitrary split.
        // Using a split index risks cutting through an assistant+tool_result pair,
        // producing orphaned tool messages that the normalizer drops every turn.
        // Instead, to_id is always the last message at trigger time; the next turn
        // will submit summary + messages_after_to_id with no orphans.
        let old_messages = candidate.messages.clone();

        if !old_messages.is_empty()
            && (old_messages.len() >= 5 || (compact_record.is_some() && old_messages.len() > 1))
        {
            let from_id = old_messages
                .first()
                .map(|message| message.id.clone())
                .ok_or_else(|| "Compaction candidate is empty".to_string())?;
            let to_id = old_messages
                .last()
                .map(|message| message.id.clone())
                .ok_or_else(|| "Compaction candidate is empty".to_string())?;
            let request_id = uuid::Uuid::new_v4().to_string();
            let compacted_range = candidate
                .compacted_range
                .clone()
                .or_else(|| compact_record.as_ref().map(compact_record_to_range));

            let request = CompactionRequest {
                request_id: request_id.clone(),
                session_id: session_id.clone(),
                messages: old_messages,
                model: model.clone(),
                provider: provider.clone(),
            };
            let pending = PendingCompactionRequest {
                request: request.clone(),
                from_id,
                to_id,
                retry_count: 0,
                context_usage: usage.clone(),
                compacted_range: compacted_range.clone(),
            };

            return Ok(CompletionPreparation::NeedsCompaction(PendingCompaction {
                request,
                pending,
                state: CompactionStateEvent {
                    session_id: session_id.clone(),
                    status: "awaiting".to_string(),
                    context_usage: Some(usage),
                    compacted_range,
                },
                invalidate_compact_record: candidate.stale_record,
            }));
        }
    }

    let compacted_range = candidate
        .compacted_range
        .clone()
        .or_else(|| compact_record.as_ref().map(compact_record_to_range));

    Ok(CompletionPreparation::Ready(PreparedCompletion {
        request: CompletionRequest {
            session_id: session_id.clone(),
            messages: candidate.messages,
            model,
            provider,
            system_prompt: if final_system_prompt.is_empty() {
                None
            } else {
                Some(final_system_prompt)
            },
            session_context,
            temperature,
            max_tokens,
            available_tools,
            backend_owned_compaction: true,
        },
        state: Some(CompactionStateEvent {
            session_id,
            status: "idle".to_string(),
            context_usage: Some(usage),
            compacted_range,
        }),
        invalidate_compact_record: candidate.stale_record,
    }))
}

pub fn emit_compaction_state(
    app_handle: &tauri::AppHandle,
    event: &CompactionStateEvent,
) -> Result<(), String> {
    app_handle
        .emit("llm:compaction-state", event)
        .map_err(|error| format!("Failed to emit compaction state: {}", error))
}

pub fn build_compaction_retry(
    pending: &PendingCompactionRequest,
) -> Option<PendingCompactionRequest> {
    if pending.retry_count >= MAX_COMPACTION_RETRIES {
        return None;
    }

    let mut next = pending.clone();
    next.retry_count += 1;
    next.request.request_id = uuid::Uuid::new_v4().to_string();
    Some(next)
}

pub fn build_awaiting_compaction_state(
    session_id: &str,
    pending: &PendingCompactionRequest,
) -> CompactionStateEvent {
    CompactionStateEvent {
        session_id: session_id.to_string(),
        status: "awaiting".to_string(),
        context_usage: Some(pending.context_usage.clone()),
        compacted_range: pending.compacted_range.clone(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompactionFailureDecision {
    Retry { attempts: u8, request_id: String },
    Exhausted { attempts: u8 },
}

pub fn classify_compaction_failure(
    pending: &PendingCompactionRequest,
) -> CompactionFailureDecision {
    if pending.retry_count < MAX_COMPACTION_RETRIES {
        CompactionFailureDecision::Retry {
            attempts: pending.retry_count + 1,
            request_id: pending.request.request_id.clone(),
        }
    } else {
        CompactionFailureDecision::Exhausted {
            attempts: pending.retry_count + 1,
        }
    }
}

fn build_candidate_stack(
    messages: Vec<Message>,
    compact_record: Option<&CompactContextRecord>,
) -> CandidateStack {
    let Some(record) = compact_record else {
        return CandidateStack {
            messages,
            summary: None,
            compacted_range: None,
            stale_record: false,
        };
    };

    let to_id_index = messages
        .iter()
        .position(|message| message.id == record.to_id);
    if let Some(index) = to_id_index {
        return CandidateStack {
            messages: messages.into_iter().skip(index + 1).collect(),
            summary: Some(format!(
                "### Previous Conversation Summary\n{}",
                record.summary
            )),
            compacted_range: Some(compact_record_to_range(record)),
            stale_record: false,
        };
    }

    CandidateStack {
        messages,
        summary: None,
        compacted_range: None,
        stale_record: true,
    }
}

fn compact_record_to_range(record: &CompactContextRecord) -> CompactedRange {
    CompactedRange {
        from_id: record.from_id.clone(),
        to_id: record.to_id.clone(),
    }
}

async fn load_compaction_settings() -> Result<CompactionSettings, String> {
    let repo = get_settings_repository();

    let context_strategy = repo
        .get("contextStrategy")
        .await
        .map_err(|error| error.to_string())?
        .and_then(|model| serde_json::from_str::<Value>(&model.value).ok())
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| "window".to_string());

    let max_input_context = repo
        .get("maxInputContext")
        .await
        .map_err(|error| error.to_string())?
        .and_then(|model| serde_json::from_str::<Value>(&model.value).ok())
        .and_then(|value| value.as_u64())
        .map(|value| value as usize)
        .filter(|value| *value > 0);

    let default_max_output_tokens = repo
        .get("advancedSettings")
        .await
        .map_err(|error| error.to_string())?
        .and_then(|model| serde_json::from_str::<Value>(&model.value).ok())
        .and_then(|value| {
            value
                .get("defaultMaxOutputTokens")
                .and_then(|field| field.as_u64())
        })
        .map(|value| value as usize)
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_MAX_OUTPUT_TOKENS);

    Ok(CompactionSettings {
        context_strategy,
        max_input_context,
        default_max_output_tokens,
    })
}

fn lookup_context_window(provider: &str, model: &str) -> Option<usize> {
    static CONFIG: OnceLock<Value> = OnceLock::new();

    let config = CONFIG.get_or_init(|| {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../src/config/llm-config.json"
        )))
        .unwrap_or(Value::Null)
    });

    config
        .get("providers")
        .and_then(|providers| providers.get(provider))
        .and_then(|provider_entry| provider_entry.get("models"))
        .and_then(|models| models.get(model))
        .and_then(|model_entry| model_entry.get("contextWindow"))
        .and_then(|value| value.as_u64())
        .map(|value| value as usize)
}

fn calculate_effective_context_limit(
    model_max_limit: usize,
    max_output_tokens: usize,
    max_input_context: Option<usize>,
) -> (usize, usize) {
    let reserved = max_output_tokens + 100;
    let safe_input_limit = if reserved < model_max_limit {
        model_max_limit - reserved
    } else {
        model_max_limit
    };

    let effective_limit = max_input_context
        .filter(|value| *value > 0 && *value < safe_input_limit)
        .unwrap_or(safe_input_limit);

    (effective_limit, model_max_limit)
}

fn estimate_text_tokens(text: &str) -> usize {
    ((text.chars().count() as f64) / 4.0).ceil() as usize
}

fn estimate_message_tokens(message: &Message) -> usize {
    let mut parts: Vec<String> = vec![message.role.clone()];

    for item in &message.content {
        match item {
            MCPContent::Text { text, .. } => parts.push(text.clone()),
            MCPContent::Thinking { thinking, .. } => parts.push(thinking.clone()),
            MCPContent::ToolCall {
                name, arguments, ..
            } => {
                parts.push(name.clone());
                parts.push(arguments.clone());
            }
            MCPContent::Resource { resource, .. } => {
                if let Some(text) = resource.get("text").and_then(|value| value.as_str()) {
                    parts.push(text.to_string());
                } else {
                    parts.push(resource.to_string());
                }
            }
            MCPContent::Image { .. } | MCPContent::Audio { .. } => {}
        }
    }

    if let Some(tool_calls) = &message.tool_calls {
        for call in tool_calls {
            parts.push(call.function.name.clone());
            parts.push(call.function.arguments.clone());
        }
    }

    if let Some(tool_use) = &message.tool_use {
        if let Some(name) = tool_use.get("name").and_then(|value| value.as_str()) {
            parts.push(name.to_string());
        }
        if let Some(input) = tool_use.get("input") {
            parts.push(input.to_string());
        }
    }

    if let Some(thinking) = &message.thinking {
        parts.push(thinking.clone());
    }

    estimate_text_tokens(&parts.join(" "))
}

/// Estimates total input tokens using grounded counting:
/// finds the most recent assistant message with actual API-reported promptTokens,
/// then adds BPE estimates only for messages appended after that point.
/// Falls back to full BPE estimation when no grounded usage is available.
fn estimate_grounded_total_tokens(
    messages: &[Message],
    system_and_context_tokens: usize,
    tools_tokens: usize,
) -> usize {
    // Search backwards for the most recent assistant message with valid promptTokens
    let grounded = messages.iter().enumerate().rev().find_map(|(idx, m)| {
        if m.role != "assistant" {
            return None;
        }
        let prompt_tokens = m
            .usage
            .as_ref()
            .and_then(|u| u.get("promptTokens"))
            .and_then(|v| v.as_u64())
            .filter(|&v| v > 0);
        prompt_tokens.map(|pt| (idx, pt as usize))
    });

    match grounded {
        Some((grounded_idx, prompt_tokens)) => {
            // prompt_tokens already includes system prompt, tools, and all messages up to
            // grounded_idx (as reported by the API). Add BPE estimates for newer messages only.
            let incremental: usize = messages[grounded_idx + 1..]
                .iter()
                .map(estimate_message_tokens)
                .sum();
            prompt_tokens + incremental
        }
        None => {
            // No grounded base available — full BPE estimation
            let message_tokens: usize = messages.iter().map(estimate_message_tokens).sum();
            message_tokens + system_and_context_tokens + tools_tokens
        }
    }
}

