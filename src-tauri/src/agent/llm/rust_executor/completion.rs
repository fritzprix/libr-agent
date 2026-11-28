use super::config::ExecutorConfig;
use super::convert::{self, GenerationOptions};
use super::events::{fallback_notice_event, stream_delta_event, stream_reset_event, ResponseSink};
use super::media_assist_fallback::{
    messages_have_multimodal_parts, prepare_messages_for_media_assist_retry,
    remember_strip_for_route, should_attempt_media_assist_fallback, should_strip_for_route,
};
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
use std::sync::atomic::{AtomicBool, Ordering};

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

/// One provider call for `request`: stream it live, and settle it.
///
/// `provider_id` is the provider actually answering, so the #3089 retry
/// applies to the individual call — a Gemini fallback behind another
/// provider gets its own retry, and a non-Gemini answer is never retried.
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
) -> Result<CompletedChat, Box<AgentRuntimeError>> {
    let provider = provider_for(provider_id, config).await?;
    let mut active_messages = request.messages.clone();
    if should_strip_for_route(provider_id, model) && messages_have_multimodal_parts(&active_messages)
    {
        let (prepared, used_plugin) =
            prepare_messages_for_media_assist_retry(&active_messages, &request.session_id).await;
        active_messages = prepared;
        log::info!(
            "Rust LLM executor: media-assist pre-strip for remembered route {provider_id}:{model} (used_plugin={used_plugin})"
        );
    }

    let build_chat_request = |messages: &[Message]| {
        convert::chat_request(
            model,
            request.system_prompt.as_deref(),
            messages,
            tools,
            max_tokens,
            options,
        )
    };
    let mut chat_request = build_chat_request(&active_messages);
    let has_yielded = AtomicBool::new(false);
    // Each delta goes out as it is read, so a listener shows the turn live.
    // `has_yielded` gates the media-assist retry (#3090): once deltas are on
    // screen a retry would duplicate the turn, so only a clean failure may
    // retry with a text-only transcript.
    let emit = |delta| {
        has_yielded.store(true, Ordering::Relaxed);
        sink.llm_stream_delta(stream_delta_event(request, delta));
    };
    let mut chat = match llm_client::complete_with_deltas(
        provider.as_ref(),
        &chat_request,
        retry,
        &emit,
    )
    .await
    {
        Ok(chat) => chat,
        Err(error)
            if !has_yielded.load(Ordering::Relaxed)
                && should_attempt_media_assist_fallback(&error, &active_messages) =>
        {
            let (prepared, used_plugin) =
                prepare_messages_for_media_assist_retry(&active_messages, &request.session_id)
                    .await;
            active_messages = prepared;
            remember_strip_for_route(provider_id, model);
            log::warn!(
                "Rust LLM executor: media-assist fallback for {provider_id}:{model} (used_plugin={used_plugin}); retrying once"
            );
            chat_request = build_chat_request(&active_messages);
            llm_client::complete_with_deltas(
                provider.as_ref(),
                &chat_request,
                RetryPolicy::NONE,
                &emit,
            )
            .await
            .map_err(runtime_error)?
        }
        Err(error) => return Err(runtime_error(error)),
    };

    // The finish reason is checked before the empty-response guard: a
    // malformed turn often carries no content at all (#3089).
    let mut no_tools_request: Option<llm_client::ChatRequest> = None;
    if is_gemini(provider_id) && is_malformed_function_call(chat.finish_reason.as_deref()) {
        if tools.is_empty() {
            // No tools offered: nothing can recover here.
            return Err(malformed_function_call_error());
        }
        log::warn!(
            "Rust LLM executor: {model} ended with MALFORMED_FUNCTION_CALL; retrying once without tools"
        );
        // The attempt streamed live deltas; withdraw its message before the
        // retry so the discarded text does not linger in the chat (#3089).
        sink.llm_stream_reset(stream_reset_event(request));
        let request = convert::chat_request(
            model,
            request.system_prompt.as_deref(),
            &active_messages,
            &[],
            max_tokens,
            options,
        );
        chat =
            llm_client::complete_with_deltas(provider.as_ref(), &request, RetryPolicy::NONE, &emit)
                .await
                .map_err(runtime_error)?;
        if is_malformed_function_call(chat.finish_reason.as_deref()) {
            // Never a second retry: the session settles with the error instead.
            return Err(malformed_function_call_error());
        }
        no_tools_request = Some(request);
    }
    if chat_is_empty(&chat) {
        // One more try: empty completions are often transient.
        let active_request = no_tools_request.as_ref().unwrap_or(&chat_request);
        chat = llm_client::complete_with_deltas(
            provider.as_ref(),
            active_request,
            RetryPolicy::NONE,
            &emit,
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
    }
    Ok(chat)
}

pub(super) async fn run_completion(
    request: &CompletionRequest,
    config: &ExecutorConfig,
    sink: &dyn ResponseSink,
) -> Result<Message, Box<AgentRuntimeError>> {
    let (options, retry, default_max_tokens) = generation_settings().await;
    let max_tokens = request.max_tokens.or(default_max_tokens);
    let request_tools = request.available_tools.as_deref().unwrap_or(&[]);
    let primary = complete_once(
        &request.provider,
        &request.model,
        request,
        config,
        sink,
        options,
        retry,
        max_tokens,
        request_tools,
    )
    .await;
    let chat = match primary {
        Ok(chat) => chat,
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
                request_tools,
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
            })?
        }
    };
    Ok(assistant_message(request, chat))
}

pub(super) async fn run_compaction(
    request: &CompactRequest,
    model: &str,
    provider_id: &str,
    system_prompt: Option<&str>,
    config: &ExecutorConfig,
) -> Result<(String, CompactStreamMeta), Box<AgentRuntimeError>> {
    let (mut options, retry, default_max_tokens) = generation_settings().await;
    // Compaction summaries must not spend a thinking budget (#2760).
    options.thinking_effort = ThinkingEffort::Off;

    let mut active_messages = request.messages.clone();
    if should_strip_for_route(provider_id, model)
        && messages_have_multimodal_parts(&active_messages)
    {
        let (prepared, used_plugin) =
            prepare_messages_for_media_assist_retry(&active_messages, &request.session_id).await;
        active_messages = prepared;
        log::info!(
            "Rust LLM executor: media-assist pre-strip for remembered route {provider_id}:{model} during compaction (used_plugin={used_plugin})"
        );
    }

    let provider = provider_for(provider_id, config).await?;
    // No tools: the summary must be plain text.
    let build_chat_request = |messages: &[Message]| {
        convert::chat_request(
            model,
            system_prompt,
            messages,
            &[],
            default_max_tokens,
            options,
        )
    };
    let chat_request = build_chat_request(&active_messages);
    let chat = match llm_client::complete(provider.as_ref(), &chat_request, retry).await {
        Ok(chat) => chat,
        Err(error) if should_attempt_media_assist_fallback(&error, &active_messages) => {
            // Same recovery as completions (#3090): a history with rejected
            // media must not fail compaction on the Rust path.
            let (prepared, used_plugin) =
                prepare_messages_for_media_assist_retry(&active_messages, &request.session_id)
                    .await;
            active_messages = prepared;
            remember_strip_for_route(provider_id, model);
            log::warn!(
                "Rust LLM executor: media-assist fallback for {provider_id}:{model} during compaction (used_plugin={used_plugin}); retrying once"
            );
            llm_client::complete(
                provider.as_ref(),
                &build_chat_request(&active_messages),
                RetryPolicy::NONE,
            )
            .await
            .map_err(runtime_error)?
        }
        Err(error) => return Err(runtime_error(error)),
    };
    let usage = chat.usage.as_ref();
    let meta = CompactStreamMeta {
        finish_reason: chat.finish_reason.clone(),
        prompt_tokens: usage.map(|usage| usage.prompt_tokens),
        completion_tokens: usage.map(|usage| usage.completion_tokens),
        total_tokens: usage.map(|usage| usage.total_tokens),
    };
    Ok((chat.content, meta))
}
