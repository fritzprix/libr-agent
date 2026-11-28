use super::config::ExecutorConfig;
use super::convert::{self, GenerationOptions};
use super::events::{fallback_notice_event, stream_delta_event, ResponseSink};
use super::settings::{
    chat_is_empty, generation_settings, provider_for, read_fallback_model, runtime_error,
};
use crate::agent::llm::types::{
    AgentRuntimeError, AgentRuntimeErrorType, CompactRequest, CompletionRequest,
};
use crate::agent::session_manager::CompactStreamMeta;
use crate::mcp::types::{MCPContent, MCPTool};
use crate::models::chat::Message;
use llm_client::{CompletedChat, RetryPolicy, ThinkingEffort};

/// The assistant message the agent loop expects back for `request`.
pub(super) fn assistant_message(request: &CompletionRequest, chat: CompletedChat) -> Message {
    let now = chrono::Utc::now().timestamp_millis();
    let prompt_tokens = chat
        .usage
        .as_ref()
        .and_then(|usage| i64::try_from(usage.prompt_tokens).ok());
    let usage = chat
        .usage
        .as_ref()
        .and_then(|usage| serde_json::to_value(usage).ok());
    Message {
        id: request.response_message_id.clone(),
        session_id: request.session_id.clone(),
        role: "assistant".to_string(),
        content: if chat.content.is_empty() {
            Vec::new()
        } else {
            vec![MCPContent::Text { text: chat.content }]
        },
        tool_calls: (!chat.tool_calls.is_empty()).then_some(chat.tool_calls),
        tool_call_id: None,
        is_streaming: Some(false),
        thinking: (!chat.thinking.is_empty()).then_some(chat.thinking),
        thinking_signature: chat.thinking_signature,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage,
        prompt_tokens,
        created_at: now,
        updated_at: now,
        source: None,
        error: None,
        metadata: None,
    }
}

#[allow(clippy::too_many_arguments)] // 7-arg shape from #2804 plus the #2796 streaming sink
async fn complete_once(
    provider_id: &str,
    model: &str,
    request: &CompletionRequest,
    config: &ExecutorConfig,
    sink: &dyn ResponseSink,
    options: GenerationOptions,
    retry: RetryPolicy,
    max_tokens: Option<u32>,
    tools: &[MCPTool],
    stream: bool,
) -> Result<CompletedChat, Box<AgentRuntimeError>> {
    let provider = provider_for(provider_id, config).await?;
    let chat_request = convert::chat_request(
        model,
        request.system_prompt.as_deref(),
        &request.messages,
        tools,
        max_tokens,
        options,
    );
    // Each delta goes out as it is read, so a listener shows the turn live.
    // `stream == false` holds the deltas back: the caller may throw this
    // attempt away (the #3089 retry), and its text must not reach the chat.
    let emit = |delta| {
        if stream {
            sink.llm_stream_delta(stream_delta_event(request, delta));
        }
    };
    let chat = llm_client::complete_with_deltas(provider.as_ref(), &chat_request, retry, emit)
        .await
        .map_err(runtime_error)?;
    if chat_is_empty(&chat) {
        // One more try: empty completions are often transient.
        let chat = llm_client::complete_with_deltas(
            provider.as_ref(),
            &chat_request,
            RetryPolicy::NONE,
            emit,
        )
        .await
        .map_err(runtime_error)?;
        if chat_is_empty(&chat) {
            return Err(Box::new(
                AgentRuntimeError::new(
                    AgentRuntimeErrorType::AiServiceError,
                    "The AI model returned an empty response with no content, tool calls, or thinking."
                        .to_string(),
                )
                .with_code("RUST_EXECUTOR_EMPTY_RESPONSE"),
            ));
        }
        return Ok(chat);
    }
    Ok(chat)
}

/// The provider this retry policy applies to (#3089).
fn is_gemini(provider: &str) -> bool {
    provider == "gemini"
}

/// The lowercased finish reason `llm-client` reports for Gemini's
/// `MALFORMED_FUNCTION_CALL` (see `finish_reason_of` in `llm-client`).
fn is_malformed_function_call(finish_reason: Option<&str>) -> bool {
    finish_reason == Some("malformed_function_call")
}

/// The error the session settles with when a Gemini turn still ends in a
/// malformed function call after the one tool-less retry (#3089).
fn malformed_function_call_error() -> Box<AgentRuntimeError> {
    Box::new(
        AgentRuntimeError::new(
            AgentRuntimeErrorType::MalformedFunctionCall,
            "Gemini returned a malformed function call even after retrying without tools."
                .to_string(),
        )
        .with_code("MALFORMED_FUNCTION_CALL"),
    )
}

#[allow(clippy::too_many_arguments)] // Same shape as `complete_once`: settings, tools, stream flag.
async fn run_completion_with_fallback(
    request: &CompletionRequest,
    config: &ExecutorConfig,
    sink: &dyn ResponseSink,
    options: GenerationOptions,
    retry: RetryPolicy,
    max_tokens: Option<u32>,
    tools: &[MCPTool],
    stream: bool,
) -> Result<CompletedChat, Box<AgentRuntimeError>> {
    let primary = complete_once(
        &request.provider,
        &request.model,
        request,
        config,
        sink,
        options,
        retry,
        max_tokens,
        tools,
        stream,
    )
    .await;
    match primary {
        Ok(chat) => Ok(chat),
        Err(primary_error) => {
            let Some(fallback) = read_fallback_model().await else {
                return Err(primary_error);
            };
            if fallback.provider == request.provider && fallback.model == request.model {
                return Err(primary_error);
            }
            // Fallback must also be answerable on the Rust path.
            if !llm_client::is_supported(&fallback.provider) {
                return Err(primary_error);
            }
            log::warn!(
                "Rust LLM executor: primary {}:{} failed; trying fallback {}:{}",
                request.provider,
                request.model,
                fallback.provider,
                fallback.model
            );
            // Tell the chat before the retry, so the switch is not silent (#2961).
            sink.llm_fallback_notice(fallback_notice_event(request, &primary_error, &fallback));
            complete_once(
                &fallback.provider,
                &fallback.model,
                request,
                config,
                sink,
                options,
                RetryPolicy::NONE,
                max_tokens,
                tools,
                stream,
            )
            .await
            .map_err(|fallback_error| {
                log::warn!(
                    "Rust LLM executor: fallback {}:{} also failed: {}",
                    fallback.provider,
                    fallback.model,
                    fallback_error.display_message
                );
                primary_error
            })
        }
    }
}

pub(super) async fn run_completion(
    request: &CompletionRequest,
    config: &ExecutorConfig,
    sink: &dyn ResponseSink,
) -> Result<Message, Box<AgentRuntimeError>> {
    let (options, retry, default_max_tokens) = generation_settings().await;
    let max_tokens = request.max_tokens.or(default_max_tokens);
    let request_tools = request.available_tools.as_deref().unwrap_or(&[]);
    // Gemini can end a turn with `MALFORMED_FUNCTION_CALL` when it was offered
    // tools. The GUI retries that one time with no tools (#3089); the Rust
    // path must do the same, or a Gemini turn fails with no recovery.
    let malformed_retry = is_gemini(&request.provider) && !request_tools.is_empty();
    // A discarded attempt must leave nothing in the live view, so the first
    // try streams only when a retry cannot happen.
    let chat = run_completion_with_fallback(
        request,
        config,
        sink,
        options,
        retry,
        max_tokens,
        request_tools,
        !malformed_retry,
    )
    .await?;
    let chat = if is_malformed_function_call(chat.finish_reason.as_deref()) {
        if !is_gemini(&request.provider) {
            // No other provider reports this finish reason; pass it through.
            chat
        } else if !malformed_retry {
            // Gemini with no tools offered: nothing can recover here.
            return Err(malformed_function_call_error());
        } else {
            log::warn!(
                "Rust LLM executor: {} ended with MALFORMED_FUNCTION_CALL; retrying once without tools",
                request.model
            );
            let retry_chat = run_completion_with_fallback(
                request,
                config,
                sink,
                options,
                RetryPolicy::NONE,
                max_tokens,
                &[],
                true,
            )
            .await?;
            if is_malformed_function_call(retry_chat.finish_reason.as_deref()) {
                // Never a second retry: the session settles with the error instead.
                return Err(malformed_function_call_error());
            }
            retry_chat
        }
    } else {
        chat
    };
    Ok(assistant_message(request, chat))
}

pub(super) async fn run_compaction(
    request: &CompactRequest,
    model: &str,
    provider: &str,
    system_prompt: Option<&str>,
    config: &ExecutorConfig,
) -> Result<(String, CompactStreamMeta), Box<AgentRuntimeError>> {
    let provider = provider_for(provider, config).await?;
    let (mut options, retry, default_max_tokens) = generation_settings().await;
    // Compaction summaries must not spend a thinking budget (#2760).
    options.thinking_effort = ThinkingEffort::Off;
    // No tools: the summary must be plain text.
    let chat_request = convert::chat_request(
        model,
        system_prompt,
        &request.messages,
        &[],
        default_max_tokens,
        options,
    );
    let chat = llm_client::complete(provider.as_ref(), &chat_request, retry)
        .await
        .map_err(runtime_error)?;
    let usage = chat.usage.as_ref();
    let meta = CompactStreamMeta {
        finish_reason: chat.finish_reason.clone(),
        prompt_tokens: usage.map(|usage| usage.prompt_tokens),
        completion_tokens: usage.map(|usage| usage.completion_tokens),
        total_tokens: usage.map(|usage| usage.total_tokens),
    };
    Ok((chat.content, meta))
}
