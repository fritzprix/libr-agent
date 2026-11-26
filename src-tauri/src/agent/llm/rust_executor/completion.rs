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
use crate::mcp::types::MCPContent;
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
) -> Result<CompletedChat, Box<AgentRuntimeError>> {
    let provider = provider_for(provider_id, config).await?;
    let chat_request = convert::chat_request(
        model,
        request.system_prompt.as_deref(),
        &request.messages,
        request.available_tools.as_deref().unwrap_or(&[]),
        max_tokens,
        options,
    );
    // Each delta goes out as it is read, so a listener shows the turn live.
    let chat = llm_client::complete_with_deltas(provider.as_ref(), &chat_request, retry, |delta| {
        sink.llm_stream_delta(stream_delta_event(request, delta));
    })
    .await
    .map_err(runtime_error)?;
    if chat_is_empty(&chat) {
        // One more try: empty completions are often transient.
        let chat = llm_client::complete_with_deltas(
            provider.as_ref(),
            &chat_request,
            RetryPolicy::NONE,
            |delta| sink.llm_stream_delta(stream_delta_event(request, delta)),
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

pub(super) async fn run_completion(
    request: &CompletionRequest,
    config: &ExecutorConfig,
    sink: &dyn ResponseSink,
) -> Result<Message, Box<AgentRuntimeError>> {
    let (options, retry, default_max_tokens) = generation_settings().await;
    let max_tokens = request.max_tokens.or(default_max_tokens);
    let primary = complete_once(
        &request.provider,
        &request.model,
        request,
        config,
        sink,
        options,
        retry,
        max_tokens,
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
