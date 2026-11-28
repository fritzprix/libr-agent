use super::config::ExecutorConfig;
use super::convert::{self, GenerationOptions};
use crate::agent::llm::types::{AgentRuntimeError, AgentRuntimeErrorType};
use crate::models::advanced_settings::AdvancedSettings;
use crate::repositories::settings_repository::SettingsRepository;
use llm_client::{ChatProvider, CompletedChat, ErrorKind, LlmError, RetryPolicy};
use serde_json::Value;
use std::time::Duration;

async fn read_setting(key: &str) -> Option<Value> {
    let repo = crate::state::try_get_settings_repository()?;
    let row = repo.get(key).await.ok()??;
    serde_json::from_str(&row.value).ok()
}

/// The provider for `provider` from the `serviceConfigs` and `customProviders`
/// settings values (the same data the frontend reads), resolving the stored
/// endpoint, API key, and Base URL. Shared with the model-list command so the
/// settings read and provider resolution live in one place.
pub(crate) async fn provider_from_settings(
    provider: &str,
) -> Result<Box<dyn ChatProvider>, String> {
    let service_configs = read_setting("serviceConfigs").await;
    let custom_providers = read_setting("customProviders").await;
    llm_client::resolve_provider(
        provider,
        service_configs.as_ref(),
        custom_providers.as_ref(),
    )
}

/// The provider for `provider`, from the `serviceConfigs` and `customProviders`
/// settings values (the same data the frontend reads).
pub(super) async fn provider_for(
    provider: &str,
    config: &ExecutorConfig,
) -> Result<Box<dyn ChatProvider>, Box<AgentRuntimeError>> {
    let resolved = match &config.endpoint_override {
        Some(endpoint) => llm_client::provider_with_endpoint(provider, endpoint.clone())
            .ok_or_else(|| format!("Provider '{provider}' is not supported")),
        None => provider_from_settings(provider).await,
    };
    resolved.map_err(|message| {
        Box::new(
            AgentRuntimeError::new(AgentRuntimeErrorType::ValidationError, message)
                .with_code("RUST_EXECUTOR_NO_ENDPOINT"),
        )
    })
}

/// The temperature, thinking effort, retry policy, and default max tokens the
/// GUI would use.
pub(super) async fn generation_settings() -> (GenerationOptions, RetryPolicy, Option<u32>) {
    let advanced = AdvancedSettings::load_global().await;
    let temperature = convert::temperature_override(
        read_setting("temperatureOverrideEnabled").await.as_ref(),
        read_setting("temperature").await.as_ref(),
    );
    let options = GenerationOptions {
        temperature,
        thinking_effort: advanced.thinking_effort,
    };
    let retry = RetryPolicy {
        max_retries: advanced.max_retries,
        base_delay: Duration::from_millis(advanced.retry_delay),
    };
    let default_max_tokens = u32::try_from(advanced.default_max_output_tokens)
        .ok()
        .filter(|tokens| *tokens > 0);
    (options, retry, default_max_tokens)
}

pub(super) fn chat_is_empty(chat: &CompletedChat) -> bool {
    chat.content.trim().is_empty() && chat.tool_calls.is_empty() && chat.thinking.trim().is_empty()
}

#[derive(Debug, Clone)]
pub(super) struct FallbackModel {
    pub(super) provider: String,
    pub(super) model: String,
}

/// Context key `runtime_error` writes the provider HTTP status under, and
/// `fallback_notice_event` reads it back from (#2961).
pub(super) const HTTP_STATUS_CONTEXT_KEY: &str = "httpStatus";

pub(super) async fn read_fallback_model() -> Option<FallbackModel> {
    let value = read_setting("fallbackModel").await?;
    let provider = value.get("provider")?.as_str()?.trim();
    let model = value.get("model")?.as_str()?.trim();
    if provider.is_empty() || model.is_empty() {
        return None;
    }
    Some(FallbackModel {
        provider: provider.to_string(),
        model: model.to_string(),
    })
}

pub(super) fn runtime_error(error: LlmError) -> Box<AgentRuntimeError> {
    let error_type = match error.kind {
        ErrorKind::Network => AgentRuntimeErrorType::NetworkError,
        ErrorKind::Authentication => AgentRuntimeErrorType::AuthenticationError,
        ErrorKind::RateLimit => AgentRuntimeErrorType::RateLimitError,
        ErrorKind::Service => AgentRuntimeErrorType::AiServiceError,
    };
    let http_status = error.status;
    let mut error = AgentRuntimeError::new(error_type, error.message).with_code(error.code);
    if let Some(status) = http_status {
        // The fallback notice (#2961) reads the HTTP status back from here.
        let mut context = serde_json::Map::new();
        context.insert(
            HTTP_STATUS_CONTEXT_KEY.to_string(),
            serde_json::json!(status),
        );
        error = error.with_context(context);
    }
    Box::new(error)
}
