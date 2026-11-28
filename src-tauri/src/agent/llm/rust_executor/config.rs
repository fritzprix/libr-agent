use crate::agent::llm::types::{AgentRuntimeError, AgentRuntimeErrorType};
use llm_client::Endpoint;
use std::sync::RwLock;

/// Settings of the executor. Production uses [`ExecutorConfig::default`].
#[derive(Clone, Default)]
pub struct ExecutorConfig {
    /// Use this endpoint instead of reading Settings. For a fixed server, such
    /// as a loopback mock in a test.
    pub endpoint_override: Option<Endpoint>,
}

impl ExecutorConfig {
    pub fn new(endpoint_override: Option<Endpoint>) -> Self {
        Self { endpoint_override }
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

/// Replace the default config until the guard is dropped.
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
        .unwrap_or_default()
}

/// Who answers an LLM request (#3094). There is no WebView fallback: every
/// GUI provider is answered in Rust, and anything else fails with a clear
/// error instead of waiting for a frontend that would not answer it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Route {
    /// This executor.
    Rust,
    /// Nobody: the executor cannot answer this provider.
    Unavailable,
}

pub(super) fn route(provider: &str) -> Route {
    if llm_client::is_supported(provider) {
        // llm-client honors the configured Base URL for every built-in
        // provider (#2797), so a custom URL routes to Rust too.
        Route::Rust
    } else {
        Route::Unavailable
    }
}

pub(super) fn route_for(provider: &str) -> (Route, ExecutorConfig) {
    (route(provider), current_config())
}

pub(super) fn no_session_manager() -> AgentRuntimeError {
    AgentRuntimeError::new(
        AgentRuntimeErrorType::ValidationError,
        "The Rust LLM executor took this request, but no session manager is available \
         to receive the result."
            .to_string(),
    )
    .with_code("RUST_EXECUTOR_NO_SESSION_MANAGER")
}

pub(super) fn unavailable(provider: &str) -> AgentRuntimeError {
    let named = if provider.is_empty() {
        "(no provider)".to_string()
    } else {
        format!("'{provider}'")
    };
    AgentRuntimeError::new(
        AgentRuntimeErrorType::ValidationError,
        format!(
            "Provider {named} is not supported by the LLM executor. Supported providers: \
             OpenAI-compatible (openai, groq, cerebras, fireworks, openrouter, ollama, and \
             custom:<id> endpoints), Anthropic, and Gemini."
        ),
    )
    .with_code("RUST_EXECUTOR_UNSUPPORTED_PROVIDER")
}

pub(super) fn no_compaction_parent() -> AgentRuntimeError {
    AgentRuntimeError::new(
        AgentRuntimeErrorType::ValidationError,
        "Compaction request has no parent request, so there is no model to call.".to_string(),
    )
    .with_code("RUST_EXECUTOR_NO_COMPACTION_PARENT")
}
