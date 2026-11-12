//! LLM execution in Rust for OpenAI-compatible providers (#2704).
//!
//! By default the WebView runs every LLM call: Rust emits
//! `llm:completion-request` / `llm:compact-request` and the frontend answers.
//! With this executor enabled, requests for an OpenAI-compatible provider are
//! answered here instead, so a session can run with no WebView and no GUI
//! window. Every other provider still goes to the frontend.
//!
//! Enable with `LIBRAGENT_LLM_EXECUTOR=rust`. Existing GUI sessions are not
//! affected unless that variable is set.
//!
//! Covered: the built-in `openai` provider and custom OpenAI-compatible
//! providers (`custom:<id>`, for example vLLM or LM Studio), streaming text,
//! reasoning, tool calls, images and audio in the request, and compaction.
//! Not covered here (the frontend has them): mid-stream loop and
//! reasoning-budget detection, retry with a fallback model, prompt-cache keys,
//! and thinking-effort mapping.

mod client;
mod convert;
mod stream;

use crate::agent::llm::types::{
    AgentRuntimeError, AgentRuntimeErrorType, CompactRequest, CompletionRequest,
};
use crate::agent::session_manager::CompactStreamMeta;
use crate::agent::AgentSessionManager;
use crate::mcp::types::MCPContent;
use crate::models::chat::Message;
use crate::repositories::settings_repository::SettingsRepository;
use client::Endpoint;
use dashmap::DashMap;
use serde_json::Value;
use std::sync::LazyLock;
use tauri::{AppHandle, Manager};

/// Process-wide switch. Value `rust` turns the executor on.
pub const EXECUTOR_ENV: &str = "LIBRAGENT_LLM_EXECUTOR";
const OPENAI_PROVIDER: &str = "openai";
const CUSTOM_PROVIDER_PREFIX: &str = "custom:";
const OPENAI_DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

/// In-flight calls by session, so a cancel can stop the HTTP request.
static IN_FLIGHT: LazyLock<DashMap<String, tokio::task::AbortHandle>> = LazyLock::new(DashMap::new);

fn env_selects_rust(value: Option<&str>) -> bool {
    value.is_some_and(|value| value.trim().eq_ignore_ascii_case("rust"))
}

/// True when this process should answer LLM requests in Rust.
pub fn is_enabled() -> bool {
    env_selects_rust(std::env::var(EXECUTOR_ENV).ok().as_deref())
}

fn is_openai_compatible(provider: &str) -> bool {
    provider == OPENAI_PROVIDER
        || provider
            .strip_prefix(CUSTOM_PROVIDER_PREFIX)
            .is_some_and(|id| !id.is_empty())
}

/// Picks the endpoint for `provider` out of the `serviceConfigs` and
/// `customProviders` settings values (the same data the frontend reads).
fn resolve_endpoint(
    provider: &str,
    service_configs: Option<&Value>,
    custom_providers: Option<&Value>,
) -> Result<Endpoint, String> {
    let non_empty = |value: Option<&Value>| {
        value
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    };

    if let Some(id) = provider.strip_prefix(CUSTOM_PROVIDER_PREFIX) {
        let entry = custom_providers
            .and_then(Value::as_array)
            .and_then(|providers| {
                providers
                    .iter()
                    .find(|entry| entry.get("id").and_then(Value::as_str) == Some(id))
            })
            .ok_or_else(|| format!("Custom provider '{provider}' is not in Settings"))?;
        let base_url = non_empty(entry.get("baseUrl"))
            .ok_or_else(|| format!("Custom provider '{provider}' has no base URL"))?;
        return Ok(Endpoint {
            base_url,
            api_key: non_empty(entry.get("apiKey")),
        });
    }

    let config = service_configs.and_then(|configs| configs.get(provider));
    Ok(Endpoint {
        base_url: non_empty(config.and_then(|config| config.get("baseUrl")))
            .unwrap_or_else(|| OPENAI_DEFAULT_BASE_URL.to_string()),
        api_key: non_empty(config.and_then(|config| config.get("apiKey"))),
    })
}

async fn read_setting(key: &str) -> Option<Value> {
    let repo = crate::state::try_get_settings_repository()?;
    let row = repo.get(key).await.ok()??;
    serde_json::from_str(&row.value).ok()
}

#[allow(clippy::result_large_err)]
async fn endpoint_for(provider: &str) -> Result<Endpoint, AgentRuntimeError> {
    let service_configs = read_setting("serviceConfigs").await;
    let custom_providers = read_setting("customProviders").await;
    resolve_endpoint(
        provider,
        service_configs.as_ref(),
        custom_providers.as_ref(),
    )
    .map_err(|message| {
        AgentRuntimeError::new(AgentRuntimeErrorType::ValidationError, message)
            .with_code("RUST_EXECUTOR_NO_ENDPOINT")
    })
}

/// The assistant message the agent loop expects back for `request`.
fn assistant_message(request: &CompletionRequest, chat: stream::CompletedChat) -> Message {
    let now = chrono::Utc::now().timestamp_millis();
    let prompt_tokens = chat
        .usage
        .as_ref()
        .and_then(|usage| usage.get("promptTokens"))
        .and_then(Value::as_i64);
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
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage: chat.usage,
        prompt_tokens,
        created_at: now,
        updated_at: now,
        source: None,
        error: None,
        metadata: None,
    }
}

#[allow(clippy::result_large_err)]
async fn run_completion(request: &CompletionRequest) -> Result<Message, AgentRuntimeError> {
    let endpoint = endpoint_for(&request.provider).await?;
    let body = convert::chat_request_body(
        &request.model,
        request.system_prompt.as_deref(),
        &request.messages,
        request.available_tools.as_deref().unwrap_or(&[]),
        request.max_tokens,
    );
    let chat = client::complete(&endpoint, &body).await?;
    Ok(assistant_message(request, chat))
}

fn manager_for_task(app_handle: &AppHandle) -> Option<AgentSessionManager> {
    app_handle
        .try_state::<AgentSessionManager>()
        .map(|manager| manager.inner().clone_for_task())
}

/// Stop this session's in-flight call, if any. Called for every
/// `llm:completion-cancel`, so a workflow cancel or a recovery retry does not
/// leave an HTTP request running.
pub fn cancel(session_id: &str) {
    if let Some((_, handle)) = IN_FLIGHT.remove(session_id) {
        handle.abort();
        log::info!("Rust LLM executor: cancelled the call for session {session_id}");
    }
}

fn track(session_id: &str, handle: tokio::task::AbortHandle) {
    if let Some(previous) = IN_FLIGHT.insert(session_id.to_string(), handle) {
        previous.abort();
    }
}

/// Answer `request` in Rust when the executor is enabled and the provider is
/// OpenAI-compatible. Returns `false` when the frontend must handle it.
pub fn try_dispatch_completion(app_handle: &AppHandle, request: &CompletionRequest) -> bool {
    if !is_enabled() || !is_openai_compatible(&request.provider) {
        return false;
    }
    let Some(manager) = manager_for_task(app_handle) else {
        return false;
    };

    let request = request.clone();
    let session_id = request.session_id.clone();
    let task = tokio::spawn(async move {
        let session_id = request.session_id.clone();
        let outcome = run_completion(&request).await;
        IN_FLIGHT.remove(&session_id);
        let delivered = match outcome {
            Ok(message) => {
                manager
                    .handle_llm_response(session_id.clone(), message)
                    .await
            }
            Err(error) => {
                log::warn!(
                    "Rust LLM executor: completion failed for session {session_id}: {}",
                    error.display_message
                );
                manager.handle_llm_error(session_id.clone(), error).await
            }
        };
        if let Err(error) = delivered {
            log::error!(
                "Rust LLM executor: could not deliver the result for session {session_id}: {error}"
            );
        }
    });
    track(&session_id, task.abort_handle());
    log::info!("Rust LLM executor: running completion for session {session_id}");
    true
}

#[allow(clippy::result_large_err)]
async fn run_compaction(
    request: &CompactRequest,
    model: &str,
    provider: &str,
    system_prompt: Option<&str>,
) -> Result<(String, CompactStreamMeta), AgentRuntimeError> {
    let endpoint = endpoint_for(provider).await?;
    // No tools: the summary must be plain text.
    let body = convert::chat_request_body(model, system_prompt, &request.messages, &[], None);
    let chat = client::complete(&endpoint, &body).await?;
    let token = |key: &str| {
        chat.usage
            .as_ref()
            .and_then(|usage| usage.get(key))
            .and_then(Value::as_u64)
    };
    let meta = CompactStreamMeta {
        finish_reason: chat.finish_reason.clone(),
        prompt_tokens: token("promptTokens"),
        completion_tokens: token("completionTokens"),
        total_tokens: token("totalTokens"),
    };
    Ok((chat.content, meta))
}

/// Compaction counterpart of [`try_dispatch_completion`].
pub fn try_dispatch_compaction(app_handle: &AppHandle, request: &CompactRequest) -> bool {
    if !is_enabled() {
        return false;
    }
    // Without the parent request there is no model to call.
    let Some(parent) = request.parent_request.as_ref() else {
        return false;
    };
    if !is_openai_compatible(&parent.provider) {
        return false;
    }
    let Some(manager) = manager_for_task(app_handle) else {
        return false;
    };

    let request = request.clone();
    let model = parent.model.clone();
    let provider = parent.provider.clone();
    let system_prompt = parent.system_prompt.clone();
    let session_id = request.session_id.clone();
    let task = tokio::spawn(async move {
        let session_id = request.session_id.clone();
        let outcome = run_compaction(&request, &model, &provider, system_prompt.as_deref()).await;
        IN_FLIGHT.remove(&session_id);
        let delivered = match outcome {
            Ok((summary, meta)) => manager
                .handle_compact_response(
                    &session_id,
                    request.to_id.clone(),
                    request.compacted_delta_count,
                    summary,
                    Some(meta),
                )
                .await
                .map(|_| ()),
            Err(error) => {
                log::warn!(
                    "Rust LLM executor: compaction failed for session {session_id}: {}",
                    error.display_message
                );
                manager
                    .handle_compact_error(session_id.clone(), error, None)
                    .await
            }
        };
        if let Err(error) = delivered {
            log::error!(
                "Rust LLM executor: could not deliver the compaction result for session {session_id}: {error}"
            );
        }
    });
    track(&session_id, task.abort_handle());
    log::info!("Rust LLM executor: running compaction for session {session_id}");
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_the_value_rust_enables_the_executor() {
        assert!(env_selects_rust(Some("rust")));
        assert!(env_selects_rust(Some(" Rust ")));
        assert!(!env_selects_rust(Some("webview")));
        assert!(!env_selects_rust(Some("")));
        assert!(!env_selects_rust(None));
    }

    #[test]
    fn only_openai_and_custom_providers_are_handled_in_rust() {
        assert!(is_openai_compatible("openai"));
        assert!(is_openai_compatible("custom:abc123"));
        assert!(!is_openai_compatible("custom:"));
        assert!(!is_openai_compatible("anthropic"));
        assert!(!is_openai_compatible("gemini"));
    }

    #[test]
    fn openai_uses_its_service_config_and_falls_back_to_the_public_api() {
        let configs = json!({
            "openai": { "apiKey": "dummy-token-abcd", "baseUrl": "http://127.0.0.1:8000/v1" },
        });
        assert_eq!(
            resolve_endpoint("openai", Some(&configs), None).unwrap(),
            Endpoint {
                base_url: "http://127.0.0.1:8000/v1".to_string(),
                api_key: Some("dummy-token-abcd".to_string()),
            }
        );
        assert_eq!(
            resolve_endpoint("openai", None, None).unwrap(),
            Endpoint {
                base_url: OPENAI_DEFAULT_BASE_URL.to_string(),
                api_key: None,
            }
        );
    }

    #[test]
    fn a_custom_provider_needs_its_settings_entry_and_a_base_url() {
        let providers = json!([
            { "id": "lmstudio", "name": "LM Studio", "baseUrl": "http://localhost:1234/v1" },
            { "id": "broken", "name": "No URL", "baseUrl": "  " },
        ]);

        assert_eq!(
            resolve_endpoint("custom:lmstudio", None, Some(&providers)).unwrap(),
            Endpoint {
                base_url: "http://localhost:1234/v1".to_string(),
                api_key: None,
            }
        );
        assert!(resolve_endpoint("custom:broken", None, Some(&providers))
            .unwrap_err()
            .contains("no base URL"));
        assert!(resolve_endpoint("custom:missing", None, Some(&providers))
            .unwrap_err()
            .contains("not in Settings"));
    }

    #[test]
    fn the_assistant_message_answers_the_request_it_was_made_for() {
        let request = CompletionRequest {
            session_id: "s1".to_string(),
            response_message_id: "resp-1".to_string(),
            messages: Vec::new(),
            model: "m".to_string(),
            provider: "openai".to_string(),
            system_prompt: None,
            max_tokens: None,
            available_tools: None,
        };
        let chat = stream::CompletedChat {
            content: "done".to_string(),
            thinking: String::new(),
            tool_calls: Vec::new(),
            usage: Some(json!({ "promptTokens": 7, "completionTokens": 1, "totalTokens": 8 })),
            finish_reason: Some("stop".to_string()),
        };

        let message = assistant_message(&request, chat);

        // The agent loop matches the reply to the turn by this id.
        assert_eq!(message.id, "resp-1");
        assert_eq!(message.session_id, "s1");
        assert_eq!(message.role, "assistant");
        assert_eq!(convert::text_of(&message.content), "done");
        assert!(message.tool_calls.is_none());
        assert!(message.thinking.is_none());
        assert_eq!(message.prompt_tokens, Some(7));
    }
}
