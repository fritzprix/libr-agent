//! LLM provider commands that go through `llm-client` (#3091).

use crate::agent::llm::rust_executor::settings::provider_from_settings;
use llm_client::ListedModel;
use std::time::Duration;

/// Lists the models `provider_id` reports, calling the provider's own model
/// list API from Rust (OpenAI-compatible (including Fireworks), Anthropic,
/// Gemini, Ollama, and OpenRouter; custom providers use the OpenAI-compatible
/// call).
///
/// Providers without a list API (Groq, Cerebras) fail here, and the GUI falls
/// back to the static LLM config, as before. The endpoint, API key, and Base
/// URL come from the same `serviceConfigs` / `customProviders` settings the
/// chat path uses (`provider_from_settings`).
#[tauri::command]
pub async fn list_llm_models(
    provider_id: String,
    timeout_ms: u64,
) -> Result<Vec<ListedModel>, String> {
    if !llm_client::has_model_list_api(&provider_id) {
        return Err(format!(
            "Provider '{provider_id}' has no model list API; using the static model list"
        ));
    }
    let provider = provider_from_settings(&provider_id).await?;
    let timeout = Duration::from_millis(timeout_ms.max(1));
    llm_client::list_models(&provider_id, provider.as_ref(), timeout)
        .await
        .map_err(|error| error.message)
}
