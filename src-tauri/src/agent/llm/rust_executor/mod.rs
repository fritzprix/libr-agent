//! LLM execution in Rust for the providers `llm-client` implements (#2704,
//! #2755, #2760).
//!
//! By default, a session whose provider (and Base URL) `llm-client` supports
//! is answered here. Providers the client does not support yet (mistral, for
//! example) still go to the WebView via `llm:completion-request` /
//! `llm:compact-request`. Set `LIBRAGENT_LLM_EXECUTOR=webview` to force every
//! call onto the frontend. [`configure`] replaces the variable, the endpoint
//! lookup, the WebView check and the delivery target for the life of its
//! guard, so a test can run a whole turn without touching the process
//! environment or needing a window.
//!
//! A request that Rust cannot answer still goes to the WebView when there is
//! one, and fails fast when there is none, because nothing would ever answer it.
//!
//! The HTTP calls go through the Tauri-free `llm-client` crate, which the GUI,
//! `libragent --tui`, and `liblet` share. This module decides who answers a
//! request, reads the provider and generation settings, and delivers results.
//!
//! Covered: the OpenAI-compatible providers (`openai`, `fireworks`,
//! `openrouter`, and `custom:<id>`, for example vLLM or LM Studio),
//! `anthropic`, and `gemini`; streaming text, reasoning and its signature,
//! tool calls, images and audio in the request, compaction, and the GUI's
//! temperature, thinking-effort, retry, and prompt-cache behaviour.
//!
//! While a call runs, each streamed piece goes out as an
//! [`AgentEvent::LlmStreamDelta`] (#2786), so the GUI shows the turn live.
//! Mid-stream loop and reasoning-budget detection is not done here: the GUI
//! feeds those events into its `StreamAccumulator`, which reports an issue
//! back and cancels the call. Not covered here (the frontend has them): retry
//! with a fallback model, and Gemini's retry without tools after a malformed
//! function call.

mod convert;

use crate::agent::events::{AgentEvent, AgentEventDispatcher};
use crate::agent::llm::types::{
    AgentRuntimeError, AgentRuntimeErrorType, CompactRequest, CompletionRequest,
};
use crate::agent::session_manager::CompactStreamMeta;
use crate::agent::tauri_events::TauriEventDispatcher;
use crate::agent::AgentSessionManager;
use crate::mcp::types::MCPContent;
use crate::models::advanced_settings::AdvancedSettings;
use crate::models::chat::Message;
use crate::repositories::settings_repository::SettingsRepository;
use convert::GenerationOptions;
use dashmap::DashMap;
pub use llm_client::Endpoint;
use llm_client::{
    ChatProvider, CompletedChat, ErrorKind, LlmError, RetryPolicy, StreamDelta, ThinkingEffort,
};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, RwLock};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::sync::oneshot;

/// Process-wide override. Value `webview` forces every call onto the frontend;
/// unset or `rust` keeps the default per-provider Rust routing (#2760).
pub const EXECUTOR_ENV: &str = "LIBRAGENT_LLM_EXECUTOR";

/// A call that is running for a session. `generation` tells a finished call
/// apart from the one that replaced it.
struct InFlight {
    generation: u64,
    handle: tokio::task::AbortHandle,
}

/// In-flight calls by session, so a cancel can stop the HTTP request.
static IN_FLIGHT: LazyLock<DashMap<String, InFlight>> = LazyLock::new(DashMap::new);
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

/// True unless the env forces the legacy all-WebView path.
fn env_enables_rust(value: Option<&str>) -> bool {
    !value.is_some_and(|value| value.trim().eq_ignore_ascii_case("webview"))
}

/// Settings of the executor. Production uses [`ExecutorConfig::from_env`].
#[derive(Clone)]
pub struct ExecutorConfig {
    /// Answer requests for supported providers in Rust (default on).
    pub enabled: bool,
    /// Use this endpoint instead of reading Settings. For a fixed server, such
    /// as a loopback mock in a test.
    pub endpoint_override: Option<Endpoint>,
    /// Treat the app as having no WebView, whatever its windows say.
    pub assume_no_webview: bool,
}

impl ExecutorConfig {
    pub fn from_env() -> Self {
        Self {
            enabled: env_enables_rust(std::env::var(EXECUTOR_ENV).ok().as_deref()),
            endpoint_override: None,
            assume_no_webview: false,
        }
    }
}

static CONFIG_OVERRIDE: RwLock<Option<ExecutorConfig>> = RwLock::new(None);

/// Keeps a [`configure`] setting in force; dropping it restores the default.
pub struct ConfigGuard(());

impl Drop for ConfigGuard {
    fn drop(&mut self) {
        *CONFIG_OVERRIDE
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    }
}

/// Replace the environment-derived config until the guard is dropped.
///
/// The override is process-wide: callers must serialize their use of it (for
/// example with a test-suite lock) so concurrent callers do not race.
#[must_use = "the setting ends when the guard is dropped"]
pub fn configure(config: ExecutorConfig) -> ConfigGuard {
    *CONFIG_OVERRIDE
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(config);
    ConfigGuard(())
}

fn current_config() -> ExecutorConfig {
    CONFIG_OVERRIDE
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
        .unwrap_or_else(ExecutorConfig::from_env)
}

/// True when this process should answer LLM requests in Rust.
pub fn is_enabled() -> bool {
    current_config().enabled
}

fn has_webview(app_handle: &AppHandle, config: &ExecutorConfig) -> bool {
    !config.assume_no_webview && !app_handle.webview_windows().is_empty()
}

/// True when something can answer an LLM request without a WebView.
pub fn runs_without_webview(app_handle: &AppHandle) -> bool {
    let config = current_config();
    config.enabled && !has_webview(app_handle, &config)
}

/// Who answers an LLM request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    /// This executor.
    Rust,
    /// The WebView, as before.
    Frontend,
    /// Nobody: the executor cannot take it and there is no WebView.
    Unavailable,
}

fn route(enabled: bool, has_webview: bool, provider: &str) -> Route {
    if !enabled {
        Route::Frontend
    } else if llm_client::is_supported(provider) {
        // llm-client honors the configured Base URL for every built-in
        // provider (#2797), so a custom URL no longer keeps a session on the
        // WebView (#2760 criterion).
        Route::Rust
    } else if has_webview {
        Route::Frontend
    } else {
        Route::Unavailable
    }
}

async fn route_for(app_handle: &AppHandle, provider: &str) -> (Route, ExecutorConfig) {
    let config = current_config();
    let route = route(config.enabled, has_webview(app_handle, &config), provider);
    (route, config)
}

fn no_session_manager() -> AgentRuntimeError {
    AgentRuntimeError::new(
        AgentRuntimeErrorType::ValidationError,
        "The Rust LLM executor took this request, but no session manager is available \
         to receive the result."
            .to_string(),
    )
    .with_code("RUST_EXECUTOR_NO_SESSION_MANAGER")
}

fn unavailable(provider: &str) -> AgentRuntimeError {
    AgentRuntimeError::new(
        AgentRuntimeErrorType::ValidationError,
        format!(
            "Provider '{provider}' needs the app window to run, and none is open. \
             Without a window only OpenAI-compatible providers, Anthropic, and Gemini can run."
        ),
    )
    .with_code("RUST_EXECUTOR_NO_WEBVIEW")
}

async fn read_setting(key: &str) -> Option<Value> {
    let repo = crate::state::try_get_settings_repository()?;
    let row = repo.get(key).await.ok()??;
    serde_json::from_str(&row.value).ok()
}

/// The provider for `provider`, from the `serviceConfigs` and `customProviders`
/// settings values (the same data the frontend reads).
async fn provider_for(
    provider: &str,
    config: &ExecutorConfig,
) -> Result<Box<dyn ChatProvider>, Box<AgentRuntimeError>> {
    let resolved = match &config.endpoint_override {
        Some(endpoint) => llm_client::provider_with_endpoint(provider, endpoint.clone())
            .ok_or_else(|| format!("Provider '{provider}' is not supported")),
        None => {
            let service_configs = read_setting("serviceConfigs").await;
            let custom_providers = read_setting("customProviders").await;
            llm_client::resolve_provider(
                provider,
                service_configs.as_ref(),
                custom_providers.as_ref(),
            )
        }
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
async fn generation_settings() -> (GenerationOptions, RetryPolicy, Option<u32>) {
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

fn chat_is_empty(chat: &CompletedChat) -> bool {
    chat.content.trim().is_empty() && chat.tool_calls.is_empty() && chat.thinking.trim().is_empty()
}

#[derive(Debug, Clone)]
struct FallbackModel {
    provider: String,
    model: String,
}

async fn read_fallback_model() -> Option<FallbackModel> {
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

fn runtime_error(error: LlmError) -> Box<AgentRuntimeError> {
    let error_type = match error.kind {
        ErrorKind::Network => AgentRuntimeErrorType::NetworkError,
        ErrorKind::Authentication => AgentRuntimeErrorType::AuthenticationError,
        ErrorKind::RateLimit => AgentRuntimeErrorType::RateLimitError,
        ErrorKind::Service => AgentRuntimeErrorType::AiServiceError,
    };
    Box::new(AgentRuntimeError::new(error_type, error.message).with_code(error.code))
}

/// The assistant message the agent loop expects back for `request`.
fn assistant_message(request: &CompletionRequest, chat: CompletedChat) -> Message {
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

async fn run_completion(
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

/// The event that carries `delta` of the call answering `request`.
fn stream_delta_event(request: &CompletionRequest, delta: StreamDelta) -> AgentEvent {
    AgentEvent::LlmStreamDelta {
        session_id: request.session_id.clone(),
        response_message_id: request.response_message_id.clone(),
        provider: request.provider.clone(),
        max_tokens: request.max_tokens,
        delta,
    }
}

/// Where an executor result is delivered: the session manager in the app.
///
/// A trait so a test can deliver through the real session handlers without a
/// full Tauri runtime, which cannot be cloned for a mock window in tests.
#[async_trait::async_trait]
pub trait ResponseSink: Send + Sync {
    /// One live piece of the running call, as an [`AgentEvent::LlmStreamDelta`].
    ///
    /// Sync and best effort: a lost delta only makes the live view lag, because
    /// the final message still arrives through [`Self::llm_response`]. The
    /// default drops it, for a target that has no live view.
    fn llm_stream_delta(&self, _event: AgentEvent) {}

    async fn llm_response(&self, session_id: String, message: Message) -> Result<(), String>;
    async fn llm_error(&self, session_id: String, error: AgentRuntimeError) -> Result<(), String>;
    async fn compact_response(
        &self,
        session_id: &str,
        request: &CompactRequest,
        summary: String,
        meta: CompactStreamMeta,
    ) -> Result<(), String>;
    async fn compact_error(
        &self,
        session_id: String,
        error: AgentRuntimeError,
    ) -> Result<(), String>;
}

#[async_trait::async_trait]
impl ResponseSink for AgentSessionManager {
    fn llm_stream_delta(&self, event: AgentEvent) {
        let dispatcher = TauriEventDispatcher::from_handle(self.app_handle());
        if let Err(error) = dispatcher.emit_agent_event(event) {
            log::debug!("Rust LLM executor: could not emit a stream delta: {error}");
        }
    }

    async fn llm_response(&self, session_id: String, message: Message) -> Result<(), String> {
        self.handle_llm_response(session_id, message).await
    }

    async fn llm_error(&self, session_id: String, error: AgentRuntimeError) -> Result<(), String> {
        self.handle_llm_error(session_id, error).await
    }

    async fn compact_response(
        &self,
        session_id: &str,
        request: &CompactRequest,
        summary: String,
        meta: CompactStreamMeta,
    ) -> Result<(), String> {
        self.handle_compact_response(
            session_id,
            request.to_id.clone(),
            request.compacted_delta_count,
            summary,
            Some(meta),
        )
        .await
        .map(|_| ())
    }

    async fn compact_error(
        &self,
        session_id: String,
        error: AgentRuntimeError,
    ) -> Result<(), String> {
        self.handle_compact_error(session_id, error, None).await
    }
}

/// The session manager kept in the app's managed state, as a delivery target.
fn manager_sink(app_handle: &AppHandle) -> Option<Arc<dyn ResponseSink>> {
    app_handle
        .try_state::<AgentSessionManager>()
        .map(|manager| Arc::new(manager.inner().clone_for_task()) as Arc<dyn ResponseSink>)
}

/// Stop this session's in-flight call, if any. Called for every
/// `llm:completion-cancel`, so a workflow cancel or a recovery retry does not
/// leave an HTTP request running.
pub fn cancel(session_id: &str) {
    if let Some((_, call)) = IN_FLIGHT.remove(session_id) {
        call.handle.abort();
        log::info!("Rust LLM executor: cancelled the call for session {session_id}");
    }
}

/// Run `work` as the one in-flight call of `session_id`.
///
/// The call is registered before `work` starts, so it can neither finish ahead
/// of its own entry nor leave a stale one. A call this replaces is aborted: the
/// new request supersedes it and settles the session. Each call removes only
/// its own entry when it ends.
fn spawn_tracked<F>(session_id: &str, work: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
    let (registered, wait_registered) = oneshot::channel::<()>();
    let key = session_id.to_string();
    let task = tokio::spawn({
        let key = key.clone();
        async move {
            // Closed channel means the entry was dropped; run anyway.
            let _ = wait_registered.await;
            work.await;
            IN_FLIGHT.remove_if(&key, |_, call| call.generation == generation);
        }
    });
    let call = InFlight {
        generation,
        handle: task.abort_handle(),
    };
    if let Some(previous) = IN_FLIGHT.insert(key, call) {
        previous.handle.abort();
        log::warn!(
            "Rust LLM executor: a new call for session {session_id} replaced one still running"
        );
    }
    let _ = registered.send(());
}

/// Answer `request` in Rust when the executor is enabled and the provider is
/// one `llm-client` supports. `Ok(false)` means the frontend must handle it; an error
/// means nothing can.
pub async fn try_dispatch_completion(
    app_handle: &AppHandle,
    request: &CompletionRequest,
) -> Result<bool, Box<AgentRuntimeError>> {
    dispatch_completion(app_handle, request, || manager_sink(app_handle)).await
}

/// [`try_dispatch_completion`] with the session-manager lookup passed in, for
/// a caller (a test) that cannot reach app state through its handle.
pub async fn dispatch_completion(
    app_handle: &AppHandle,
    request: &CompletionRequest,
    manager: impl FnOnce() -> Option<Arc<dyn ResponseSink>>,
) -> Result<bool, Box<AgentRuntimeError>> {
    let (route, config) = route_for(app_handle, &request.provider).await;
    match route {
        Route::Frontend => return Ok(false),
        Route::Unavailable => return Err(Box::new(unavailable(&request.provider))),
        Route::Rust => {}
    }
    // The route said Rust can answer, so falling back to the WebView now would
    // silently lose the request when there is none: fail instead.
    let Some(sink) = manager() else {
        return Err(Box::new(no_session_manager()));
    };

    let request = request.clone();
    let session_id = request.session_id.clone();
    spawn_tracked(&session_id, async move {
        let session_id = request.session_id.clone();
        let delivered = match run_completion(&request, &config, sink.as_ref()).await {
            Ok(message) => sink.llm_response(session_id.clone(), message).await,
            Err(error) => {
                log::warn!(
                    "Rust LLM executor: completion failed for session {session_id}: {}",
                    error.display_message
                );
                sink.llm_error(session_id.clone(), *error).await
            }
        };
        if let Err(error) = delivered {
            log::error!(
                "Rust LLM executor: could not deliver the result for session {session_id}: {error}"
            );
        }
    });
    log::info!("Rust LLM executor: running completion for session {session_id}");
    Ok(true)
}

async fn run_compaction(
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

/// Compaction counterpart of [`try_dispatch_completion`].
pub async fn try_dispatch_compaction(
    app_handle: &AppHandle,
    request: &CompactRequest,
) -> Result<bool, Box<AgentRuntimeError>> {
    dispatch_compaction(app_handle, request, || manager_sink(app_handle)).await
}

/// [`try_dispatch_compaction`] with the session-manager lookup passed in.
pub async fn dispatch_compaction(
    app_handle: &AppHandle,
    request: &CompactRequest,
    manager: impl FnOnce() -> Option<Arc<dyn ResponseSink>>,
) -> Result<bool, Box<AgentRuntimeError>> {
    // Without the parent request there is no model to call, so no provider to
    // route by: the frontend gets it, as before.
    let provider = request
        .parent_request
        .as_ref()
        .map(|parent| parent.provider.as_str())
        .unwrap_or_default();
    let (route, config) = route_for(app_handle, provider).await;
    match route {
        Route::Frontend => return Ok(false),
        Route::Unavailable => return Err(Box::new(unavailable(provider))),
        Route::Rust => {}
    }
    let Some(parent) = request.parent_request.as_ref() else {
        return Ok(false);
    };
    // The route said Rust can answer, so falling back to the WebView now would
    // silently lose the request when there is none: fail instead.
    let Some(sink) = manager() else {
        return Err(Box::new(no_session_manager()));
    };

    let request = request.clone();
    let model = parent.model.clone();
    let provider = parent.provider.clone();
    let system_prompt = parent.system_prompt.clone();
    let session_id = request.session_id.clone();
    spawn_tracked(&session_id, async move {
        let session_id = request.session_id.clone();
        let outcome = run_compaction(
            &request,
            &model,
            &provider,
            system_prompt.as_deref(),
            &config,
        )
        .await;
        let delivered = match outcome {
            Ok((summary, meta)) => {
                sink.compact_response(&session_id, &request, summary, meta)
                    .await
            }
            Err(error) => {
                log::warn!(
                    "Rust LLM executor: compaction failed for session {session_id}: {}",
                    error.display_message
                );
                sink.compact_error(session_id.clone(), *error).await
            }
        };
        if let Err(error) = delivered {
            log::error!(
                "Rust LLM executor: could not deliver the compaction result for session {session_id}: {error}"
            );
        }
    });
    log::info!("Rust LLM executor: running compaction for session {session_id}");
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn webview_is_the_only_env_value_that_disables_rust() {
        assert!(env_enables_rust(None));
        assert!(env_enables_rust(Some("")));
        assert!(env_enables_rust(Some("rust")));
        assert!(env_enables_rust(Some(" Rust ")));
        assert!(!env_enables_rust(Some("webview")));
        assert!(!env_enables_rust(Some(" WebView ")));
    }

    #[test]
    fn client_errors_keep_their_kind_and_code() {
        let cases = [
            (ErrorKind::Network, AgentRuntimeErrorType::NetworkError),
            (
                ErrorKind::Authentication,
                AgentRuntimeErrorType::AuthenticationError,
            ),
            (ErrorKind::RateLimit, AgentRuntimeErrorType::RateLimitError),
            (ErrorKind::Service, AgentRuntimeErrorType::AiServiceError),
        ];
        for (kind, expected) in cases {
            let error = runtime_error(LlmError {
                kind,
                code: "RUST_EXECUTOR_HTTP",
                message: "boom".to_string(),
                status: None,
            });
            assert_eq!(
                std::mem::discriminant(&error.error_type),
                std::mem::discriminant(&expected)
            );
            let code = error
                .details
                .as_ref()
                .and_then(|details| details.error_code.as_deref());
            assert_eq!(code, Some("RUST_EXECUTOR_HTTP"));
        }
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
        let chat = CompletedChat {
            content: "done".to_string(),
            thinking: String::new(),
            thinking_signature: Some("sig".to_string()),
            tool_calls: Vec::new(),
            usage: Some(llm_client::Usage {
                prompt_tokens: 7,
                completion_tokens: 1,
                total_tokens: 8,
                cached_prompt_tokens: None,
                details: None,
            }),
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
        // Anthropic and Gemini need it back with the next request.
        assert_eq!(message.thinking_signature.as_deref(), Some("sig"));
        assert_eq!(message.prompt_tokens, Some(7));
        assert_eq!(
            message.usage,
            Some(json!({ "promptTokens": 7, "completionTokens": 1, "totalTokens": 8 }))
        );
    }

    #[test]
    fn a_stream_delta_event_names_its_turn_and_serializes_for_the_frontend() {
        let request = CompletionRequest {
            session_id: "s1".to_string(),
            response_message_id: "resp-1".to_string(),
            messages: Vec::new(),
            model: "m".to_string(),
            provider: "anthropic".to_string(),
            system_prompt: None,
            max_tokens: Some(4096),
            available_tools: None,
        };
        let delta = StreamDelta {
            content: Some("hi".to_string()),
            ..StreamDelta::default()
        };

        let event = serde_json::to_value(stream_delta_event(&request, delta)).unwrap();

        assert_eq!(
            event,
            json!({
                "type": "llmStreamDelta",
                "sessionId": "s1",
                "responseMessageId": "resp-1",
                "provider": "anthropic",
                "maxTokens": 4096,
                "delta": { "content": "hi" },
            })
        );
    }

    #[test]
    fn routing_keeps_the_frontend_unless_rust_can_answer() {
        // Off: everything goes to the WebView, with or without one.
        assert_eq!(route(false, true, "openai"), Route::Frontend);
        assert_eq!(route(false, false, "anthropic"), Route::Frontend);
        // On: supported providers are answered in Rust either way. A
        // configured Base URL is honored by llm-client for every built-in
        // provider (#2797), so it never changes the route; see
        // `every_built_in_provider_honors_its_configured_base_url` in
        // llm-client and `configured_anthropic_base_url_routes_to_rust` in
        // the integration tests.
        assert_eq!(route(true, true, "openai"), Route::Rust);
        assert_eq!(route(true, false, "custom:lmstudio"), Route::Rust);
        assert_eq!(route(true, false, "fireworks"), Route::Rust);
        assert_eq!(route(true, false, "openrouter"), Route::Rust);
        assert_eq!(route(true, false, "anthropic"), Route::Rust);
        assert_eq!(route(true, true, "gemini"), Route::Rust);
        assert_eq!(route(true, false, "groq"), Route::Rust);
        assert_eq!(route(true, false, "cerebras"), Route::Rust);
        assert_eq!(route(true, true, "ollama"), Route::Rust);
        // On: any other provider needs the WebView, and fails fast without it.
        assert_eq!(route(true, true, "mistral"), Route::Frontend);
        assert_eq!(route(true, false, "mistral"), Route::Unavailable);
        assert_eq!(route(true, false, ""), Route::Unavailable);
    }

    #[test]
    fn an_unavailable_route_says_which_provider_cannot_run() {
        let error = unavailable("mistral");
        let code = error
            .details
            .as_ref()
            .and_then(|details| details.error_code.as_deref());
        assert_eq!(code, Some("RUST_EXECUTOR_NO_WEBVIEW"));
        assert!(error.display_message.contains("mistral"));
    }

    #[test]
    fn a_missing_session_manager_names_the_missing_receiver() {
        let error = no_session_manager();
        let code = error
            .details
            .as_ref()
            .and_then(|details| details.error_code.as_deref());
        assert_eq!(code, Some("RUST_EXECUTOR_NO_SESSION_MANAGER"));
        assert!(error.display_message.contains("session manager"));
    }

    #[tokio::test]
    async fn a_call_that_ends_at_once_leaves_no_entry_behind() {
        let (done, finished) = oneshot::channel::<()>();
        spawn_tracked("early-finish", async move {
            let _ = done.send(());
        });
        finished.await.unwrap();
        // The entry is removed right after the work, on the task itself.
        for _ in 0..100 {
            if !IN_FLIGHT.contains_key("early-finish") {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("a finished call left its in-flight entry");
    }

    #[tokio::test]
    async fn a_replacing_call_aborts_the_old_one_and_keeps_its_own_entry() {
        let (old_started, old_running) = oneshot::channel::<()>();
        let (old_dropped, old_was_aborted) = oneshot::channel::<()>();
        spawn_tracked("replaced", async move {
            // Dropped when the task is aborted, which signals the test.
            let _signal = old_dropped;
            let _ = old_started.send(());
            std::future::pending::<()>().await;
        });
        old_running.await.unwrap();

        let (release, wait_release) = oneshot::channel::<()>();
        spawn_tracked("replaced", async move {
            let _ = wait_release.await;
        });

        // The old call was aborted: its sender is dropped without a send.
        assert!(old_was_aborted.await.is_err());
        // The old call's end did not remove the new call's entry.
        assert!(IN_FLIGHT.contains_key("replaced"));
        let _ = release.send(());
        cancel("replaced");
        assert!(!IN_FLIGHT.contains_key("replaced"));
    }
}
