use crate::agent::llm::types::{AgentRuntimeError, AgentRuntimeErrorType};
use llm_client::Endpoint;
use std::sync::RwLock;
use tauri::{AppHandle, Manager};

/// Process-wide override. Value `webview` forces every call onto the frontend;
/// unset or `rust` keeps the default per-provider Rust routing (#2760).
pub const EXECUTOR_ENV: &str = "LIBRAGENT_LLM_EXECUTOR";

/// True unless the env forces the legacy all-WebView path.
pub(super) fn env_enables_rust(value: Option<&str>) -> bool {
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
pub(super) enum Route {
    /// This executor.
    Rust,
    /// The WebView, as before.
    Frontend,
    /// Nobody: the executor cannot take it and there is no WebView.
    Unavailable,
}

pub(super) fn route(enabled: bool, has_webview: bool, provider: &str) -> Route {
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

pub(super) async fn route_for(app_handle: &AppHandle, provider: &str) -> (Route, ExecutorConfig) {
    let config = current_config();
    let route = route(config.enabled, has_webview(app_handle, &config), provider);
    (route, config)
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
    AgentRuntimeError::new(
        AgentRuntimeErrorType::ValidationError,
        format!(
            "Provider '{provider}' needs the app window to run, and none is open. \
             Without a window only OpenAI-compatible providers, Anthropic, and Gemini can run."
        ),
    )
    .with_code("RUST_EXECUTOR_NO_WEBVIEW")
}
