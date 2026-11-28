//! LLM provider commands that go through `llm-client` (#3091).

use llm_client::ListedModel;
use serde_json::Value;
use std::time::Duration;

/// Reads a settings value, the same way the Rust executor does.
async fn read_setting(key: &str) -> Option<Value> {
    let repo = crate::state::try_get_settings_repository()?;
    let row = repo.get(key).await.ok()??;
    serde_json::from_str(&row.value).ok()
}

/// Lists the models `provider_id` reports, calling the provider's own model
/// list API from Rust (OpenAI-compatible, Anthropic, Gemini, Ollama, and
/// OpenRouter; custom providers use the OpenAI-compatible call).
///
/// Providers without a list API (Groq, Cerebras, Fireworks) fail here, and
/// the GUI falls back to the static LLM config, as before. The endpoint,
/// API key, and Base URL come from the same `serviceConfigs` /
/// `customProviders` settings the chat path uses.
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
    let service_configs = read_setting("serviceConfigs").await;
    let custom_providers = read_setting("customProviders").await;
    let provider = llm_client::resolve_provider(
        &provider_id,
        service_configs.as_ref(),
        custom_providers.as_ref(),
    )?;
    let timeout = Duration::from_millis(timeout_ms.max(1));
    llm_client::list_models(&provider_id, provider.as_ref(), timeout)
        .await
        .map_err(|error| error.message)
}
