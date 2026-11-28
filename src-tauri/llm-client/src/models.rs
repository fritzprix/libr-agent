//! Provider model-list calls (#3091).
//!
//! The GUI used to list models through the TypeScript provider services.
//! `llm-client` makes the same list calls here, so the GUI can ask the native
//! client instead and the Tauri command layer can expose one command. Each
//! call returns [`ListedModel`] entries carrying whatever the provider's list
//! API reports; the GUI still merges them with the static LLM config, as the
//! services did. Providers without a list API ([`has_model_list_api`] says
//! which) keep the static list.

use crate::error::{ErrorKind, LlmError};
use crate::provider::ChatProvider;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const ANTHROPIC_PAGE_LIMIT: &str = "1000";
const GEMINI_PAGE_SIZE: &str = "100";

/// One model as a provider's list API reports it.
///
/// The field names are `camelCase` so the shape matches the GUI's
/// `ModelInfo`. Fields the provider does not report stay `None`; the GUI
/// fills those in from the static LLM config.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListedModel {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub support_reasoning: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub support_tools: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub support_streaming: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<ModelCost>,
}

/// Cost per million tokens, in USD, as a provider reports it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ModelCost {
    pub input: f64,
    pub output: f64,
}

/// True for the providers [`list_models`] can call. Providers without a list
/// API (Groq, Cerebras) keep the static list from the LLM config. Fireworks
/// is listed through its OpenAI-compatible `/models` endpoint, as the GUI's
/// `FireworksService` did by inheriting `OpenAIService`.
pub fn has_model_list_api(provider: &str) -> bool {
    matches!(
        provider,
        "openai" | "anthropic" | "gemini" | "ollama" | "openrouter" | "fireworks"
    ) || provider
        .strip_prefix("custom:")
        .is_some_and(|id| !id.is_empty())
}

/// Lists the models `provider` reports, giving up after `timeout`.
///
/// `provider_kind` is the settings provider id (`openai`, `custom:<id>`, …)
/// and decides which list API is called; `provider` carries the endpoint,
/// the key, and the error classification. The request goes through the same
/// system-proxy resolution as a chat call.
pub async fn list_models(
    provider_kind: &str,
    provider: &dyn ChatProvider,
    timeout: Duration,
) -> Result<Vec<ListedModel>, LlmError> {
    let client = list_http_client(timeout)?;
    let base = provider.endpoint().base_url.trim_end_matches('/');
    let models = match provider_kind {
        "ollama" => {
            let body = get_json(&client, provider, &format!("{base}/api/tags"), &[], false).await?;
            parse_ollama_models(&body)
        }
        "openrouter" => {
            // Public metadata endpoint, no key, as the GUI's metadata service.
            let body = get_json(&client, provider, &format!("{base}/models"), &[], false).await?;
            parse_openrouter_models(&body)
        }
        "anthropic" => list_anthropic(&client, provider, base).await?,
        "gemini" => list_gemini(&client, provider, base).await?,
        "openai" | "fireworks" => list_openai_compatible(&client, provider, base).await?,
        kind if kind.starts_with("custom:") => {
            list_openai_compatible(&client, provider, base).await?
        }
        kind => {
            return Err(LlmError::new(
                ErrorKind::Service,
                "RUST_EXECUTOR_UNSUPPORTED",
                format!("Provider '{kind}' has no model list API"),
            ))
        }
    };
    Ok(models)
}

/// The HTTP client for one list call: the same system-proxy configuration as
/// the chat client ([`crate::client`]), with a per-call total timeout.
fn list_http_client(timeout: Duration) -> Result<reqwest::Client, LlmError> {
    let builder = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(timeout);
    let builder = http_proxy::configure_client_builder(builder)
        .map_err(|e| network_error(format!("Failed to configure the HTTP client: {e}")))?;
    builder
        .build()
        .map_err(|e| network_error(format!("Failed to build the HTTP client: {e}")))
}

fn network_error(message: String) -> LlmError {
    LlmError::new(ErrorKind::Network, "RUST_EXECUTOR_NETWORK", message)
}

/// One `GET` that must answer with JSON, classified like a chat call.
async fn get_json(
    client: &reqwest::Client,
    provider: &dyn ChatProvider,
    url: &str,
    query: &[(&str, String)],
    with_auth: bool,
) -> Result<Value, LlmError> {
    if let Err(e) = http_proxy::ensure_proxy_resolved(url).await {
        log::warn!("system proxy pre-resolve failed for model list URL: {e}");
    }
    let mut request = client.get(url).header("accept", "application/json");
    for (name, value) in provider.extra_headers() {
        request = request.header(*name, *value);
    }
    if with_auth {
        if let Some(api_key) = provider
            .endpoint()
            .api_key
            .as_deref()
            .filter(|key| !key.is_empty())
        {
            let (name, value) = provider.auth_header(api_key);
            request = request.header(name, value);
        }
    }
    if !query.is_empty() {
        request = request.query(query);
    }
    let response = request.send().await.map_err(|e| {
        // `without_url`: a base URL can carry credentials in its query.
        // A timeout loses that detail in `without_url`, so say it here.
        let detail = if e.is_timeout() {
            "operation timed out".to_string()
        } else {
            e.without_url().to_string()
        };
        network_error(format!("Could not reach the model list endpoint: {detail}"))
    })?;
    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(provider.http_error(status.as_u16(), &text));
    }
    response.json().await.map_err(|e| {
        network_error(format!(
            "The model list response could not be read: {}",
            e.without_url()
        ))
    })
}

async fn list_openai_compatible(
    client: &reqwest::Client,
    provider: &dyn ChatProvider,
    base: &str,
) -> Result<Vec<ListedModel>, LlmError> {
    let body = get_json(client, provider, &format!("{base}/models"), &[], true).await?;
    Ok(parse_openai_models(&body))
}

async fn list_anthropic(
    client: &reqwest::Client,
    provider: &dyn ChatProvider,
    base: &str,
) -> Result<Vec<ListedModel>, LlmError> {
    let url = format!("{base}/models");
    let mut models = Vec::new();
    let mut after_id: Option<String> = None;
    loop {
        let mut query: Vec<(&str, String)> = vec![("limit", ANTHROPIC_PAGE_LIMIT.to_string())];
        if let Some(id) = &after_id {
            query.push(("after_id", id.clone()));
        }
        let body = get_json(client, provider, &url, &query, true).await?;
        let page = parse_anthropic_models(&body);
        let has_more = body
            .get("has_more")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let last_id = page.last().map(|model| model.id.clone());
        models.extend(page);
        // Stop when the server repeats the same cursor: a `has_more: true`
        // that never advances would otherwise loop after the caller gave up.
        match (has_more, last_id) {
            (true, Some(id)) if Some(&id) != after_id.as_ref() => after_id = Some(id),
            _ => break,
        }
    }
    Ok(models)
}

async fn list_gemini(
    client: &reqwest::Client,
    provider: &dyn ChatProvider,
    base: &str,
) -> Result<Vec<ListedModel>, LlmError> {
    let url = format!("{base}/models");
    let mut models = Vec::new();
    let mut page_token: Option<String> = None;
    loop {
        let mut query: Vec<(&str, String)> = vec![("page_size", GEMINI_PAGE_SIZE.to_string())];
        if let Some(token) = &page_token {
            query.push(("page_token", token.clone()));
        }
        let body = get_json(client, provider, &url, &query, true).await?;
        let page = parse_gemini_models(&body);
        let next = body
            .get("nextPageToken")
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty())
            .map(str::to_string);
        models.extend(page);
        let stop = match (&next, &page_token) {
            (Some(token), previous) => Some(token.as_str()) == previous.as_deref(),
            (None, _) => true,
        };
        page_token = next;
        if stop {
            break;
        }
    }
    Ok(models)
}

fn entries<'a>(body: &'a Value, key: &str) -> Vec<&'a Value> {
    body.get(key)
        .and_then(Value::as_array)
        .map(|array| array.iter().collect())
        .unwrap_or_default()
}

fn text(entry: &Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn number(entry: &Value, key: &str) -> Option<u32> {
    entry
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
}

fn listed(
    id: String,
    name: String,
    description: String,
    context_window: Option<u32>,
) -> ListedModel {
    ListedModel {
        id,
        name,
        description,
        context_window,
        support_reasoning: None,
        support_tools: None,
        support_streaming: None,
        cost: None,
    }
}

/// OpenAI's `/v1/models` shape: a `data` array of `{id, ...}` entries, or a
/// bare array (some proxies). An entry's id may arrive as `id`, `model`, or
/// `name`, matching the GUI's `fetchOpenAIModels`.
fn parse_openai_models(body: &Value) -> Vec<ListedModel> {
    let from_data = entries(body, "data");
    let page = if !from_data.is_empty() {
        from_data
    } else if body.is_array() {
        body.as_array().unwrap().iter().collect()
    } else {
        Vec::new()
    };
    page.into_iter()
        .filter_map(|entry| {
            let id = match entry {
                Value::String(value) => value.trim().to_string(),
                Value::Object(_) => ["id", "model", "name"]
                    .iter()
                    .filter_map(|key| entry.get(key).and_then(Value::as_str))
                    .map(str::trim)
                    .find(|value| !value.is_empty())
                    .unwrap_or_default()
                    .to_string(),
                _ => String::new(),
            };
            if id.is_empty() {
                return None;
            }
            Some(listed(id.clone(), id, text(entry, "description"), None))
        })
        .collect()
}

/// Anthropic's `/v1/models` shape: `{data: [{id, display_name}], has_more}`.
fn parse_anthropic_models(body: &Value) -> Vec<ListedModel> {
    entries(body, "data")
        .into_iter()
        .filter_map(|entry| {
            let id = text(entry, "id");
            if id.is_empty() {
                return None;
            }
            let display_name = text(entry, "display_name");
            let name = if display_name.is_empty() {
                id.clone()
            } else {
                display_name
            };
            Some(listed(id, name, String::new(), None))
        })
        .collect()
}

/// Gemini's `/v1beta/models` shape: `{models: [{name: "models/<id>", ...}]}`.
/// Models that cannot generate are skipped, as the GUI does.
fn parse_gemini_models(body: &Value) -> Vec<ListedModel> {
    entries(body, "models")
        .into_iter()
        .filter_map(|entry| {
            let raw = text(entry, "name");
            let id = raw.strip_prefix("models/").unwrap_or(&raw).to_string();
            if id.is_empty() {
                return None;
            }
            // A response without the field is kept, as the GUI's `?? true`.
            if let Some(methods) = entry
                .get("supportedGenerationMethods")
                .and_then(Value::as_array)
            {
                if !methods
                    .iter()
                    .any(|method| method.as_str() == Some("generateContent"))
                {
                    return None;
                }
            }
            Some(listed(
                id,
                text(entry, "displayName"),
                text(entry, "description"),
                number(entry, "inputTokenLimit"),
            ))
        })
        .collect()
}

/// Ollama's native `/api/tags` shape: `{models: [{name, details: {family}}]}`.
fn parse_ollama_models(body: &Value) -> Vec<ListedModel> {
    entries(body, "models")
        .into_iter()
        .filter_map(|entry| {
            let id = text(entry, "name");
            if id.is_empty() {
                return None;
            }
            let family = entry
                .get("details")
                .map(|details| text(details, "family"))
                .unwrap_or_default();
            Some(listed(id.clone(), id, family, None))
        })
        .collect()
}

/// OpenRouter's public `/api/v1/models` shape, mapped as the GUI's
/// `OpenRouterService.listModels` did: capabilities come from
/// `supported_parameters`, costs from per-token `pricing`.
fn parse_openrouter_models(body: &Value) -> Vec<ListedModel> {
    entries(body, "data")
        .into_iter()
        .filter_map(|entry| {
            let id = text(entry, "id");
            if id.is_empty() {
                return None;
            }
            let name = {
                let raw = text(entry, "name");
                if raw.is_empty() {
                    id.clone()
                } else {
                    raw
                }
            };
            let description = {
                let raw = text(entry, "description");
                if raw.is_empty() {
                    name.clone()
                } else {
                    raw
                }
            };
            let parameters: Vec<&str> = entry
                .get("supported_parameters")
                .and_then(Value::as_array)
                .map(|array| array.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            let pricing = entry.get("pricing");
            let per_million = |key: &str| {
                pricing
                    .and_then(|pricing| pricing.get(key))
                    .and_then(Value::as_str)
                    .and_then(|value| value.parse::<f64>().ok())
                    .unwrap_or(0.0)
                    * 1_000_000.0
            };
            let internal_reasoning = pricing
                .map(|pricing| text(pricing, "internal_reasoning"))
                .unwrap_or_default();
            Some(ListedModel {
                id,
                name,
                description,
                context_window: number(entry, "context_length"),
                support_reasoning: Some(
                    parameters.contains(&"reasoning") || !internal_reasoning.is_empty(),
                ),
                support_tools: Some(parameters.contains(&"tools")),
                support_streaming: Some(
                    parameters.contains(&"stream") || parameters.contains(&"streaming"),
                ),
                cost: Some(ModelCost {
                    input: per_million("prompt"),
                    output: per_million("completion"),
                }),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{CustomOpenAi, Endpoint, Fireworks, Ollama, OpenAi};
    use crate::{Anthropic, Gemini};
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use warp::Filter;

    /// `authorization`, `x-api-key`, and `x-goog-api-key` as received.
    type AuthHeaders = (Option<String>, Option<String>, Option<String>);

    #[derive(Default)]
    struct Seen {
        calls: AtomicUsize,
        paths: Mutex<Vec<String>>,
        queries: Mutex<Vec<HashMap<String, String>>>,
        auth: Mutex<Vec<AuthHeaders>>,
    }

    /// Serves `GET` on a free loopback port; `reply` gets the 0-based call
    /// number.
    async fn serve_get(
        reply: impl Fn(usize) -> String + Clone + Send + Sync + 'static,
    ) -> (String, Arc<Seen>) {
        let seen = Arc::new(Seen::default());
        let record = seen.clone();
        let route = warp::get()
            .and(warp::path::tail())
            .and(warp::query::<HashMap<String, String>>())
            .and(warp::header::optional::<String>("authorization"))
            .and(warp::header::optional::<String>("x-api-key"))
            .and(warp::header::optional::<String>("x-goog-api-key"))
            .map(move |tail: warp::path::Tail, query, auth, api_key, goog| {
                record.paths.lock().unwrap().push(tail.as_str().to_string());
                record.queries.lock().unwrap().push(query);
                record.auth.lock().unwrap().push((auth, api_key, goog));
                let call = record.calls.fetch_add(1, Ordering::SeqCst);
                warp::reply::json(&serde_json::from_str::<Value>(&reply(call)).unwrap())
            });
        let (addr, server) = warp::serve(route).bind_ephemeral(([127, 0, 0, 1], 0));
        tokio::spawn(server);
        (format!("http://{addr}"), seen)
    }

    fn endpoint(base_url: &str, api_key: Option<&str>) -> Endpoint {
        Endpoint {
            base_url: base_url.to_string(),
            api_key: api_key.map(str::to_string),
        }
    }

    #[test]
    fn only_the_providers_with_a_list_api_are_offered() {
        for provider in [
            "openai",
            "anthropic",
            "gemini",
            "ollama",
            "openrouter",
            "fireworks",
            "custom:lmstudio",
        ] {
            assert!(has_model_list_api(provider), "{provider}");
        }
        // Groq and Cerebras keep the static list (#3091).
        for provider in ["groq", "cerebras", "custom:", "mistral", ""] {
            assert!(!has_model_list_api(provider), "{provider}");
        }
    }

    #[test]
    fn openai_entries_accept_id_model_and_name() {
        let models = parse_openai_models(&json!({
            "object": "list",
            "data": [
                { "id": "gpt-4o", "description": "GPT-4o", "created": 1 },
                { "model": "legacy-name" },
                { "name": "named-entry" },
                { "created": 2 },
            ]
        }));
        assert_eq!(models.len(), 3);
        assert_eq!(models[0].id, "gpt-4o");
        assert_eq!(models[0].description, "GPT-4o");
        assert_eq!(models[1].id, "legacy-name");
        assert_eq!(models[2].id, "named-entry");
    }

    #[test]
    fn openai_accepts_a_bare_array_response() {
        let models = parse_openai_models(&json!(["a-model", { "id": "b-model" }]));
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "a-model");
        assert_eq!(models[1].id, "b-model");
    }

    #[test]
    fn anthropic_entries_use_the_display_name() {
        let models = parse_anthropic_models(&json!({
            "data": [
                { "id": "claude-sonnet-4-5", "display_name": "Claude Sonnet 4.5" },
                { "id": "claude-opus-4-1" },
            ],
            "has_more": false
        }));
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].name, "Claude Sonnet 4.5");
        assert_eq!(models[1].name, "claude-opus-4-1");
    }

    #[test]
    fn gemini_strips_the_prefix_and_keeps_generating_models_only() {
        let models = parse_gemini_models(&json!({
            "models": [
                {
                    "name": "models/gemini-2.5-flash",
                    "displayName": "Gemini 2.5 Flash",
                    "description": "Fast model",
                    "inputTokenLimit": 1048576,
                    "supportedGenerationMethods": ["generateContent", "streamGenerateContent"]
                },
                {
                    "name": "models/text-embedding-004",
                    "supportedGenerationMethods": ["embedContent"]
                },
                { "name": "models/no-methods-shown" },
                { "name": "models/" }
            ],
            "nextPageToken": "p2"
        }));
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "gemini-2.5-flash");
        assert_eq!(models[0].name, "Gemini 2.5 Flash");
        assert_eq!(models[0].context_window, Some(1048576));
        assert_eq!(models[1].id, "no-methods-shown");
        assert_eq!(models[1].context_window, None);
    }

    #[test]
    fn ollama_entries_carry_the_model_family() {
        let models = parse_ollama_models(&json!({
            "models": [
                { "name": "llama3.1:8b", "details": { "family": "llama" } },
                { "name": "qwen2.5-coder:7b" }
            ]
        }));
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "llama3.1:8b");
        assert_eq!(models[0].description, "llama");
        assert_eq!(models[1].description, "");
    }

    #[test]
    fn openrouter_entries_map_parameters_pricing_and_context() {
        let models = parse_openrouter_models(&json!({
            "data": [
                {
                    "id": "anthropic/claude-sonnet-4.5",
                    "name": "Claude Sonnet 4.5",
                    "description": "Sonnet",
                    "context_length": 200000,
                    "supported_parameters": ["tools", "stream", "reasoning"],
                    "pricing": { "prompt": "0.000003", "completion": "0.000015" }
                },
                {
                    "id": "deepseek/deepseek-r1",
                    "supported_parameters": ["reasoning"],
                    "pricing": { "prompt": "0.0000004", "completion": "0.000002", "internal_reasoning": "0.000004" }
                }
            ]
        }));
        assert_eq!(models.len(), 2);
        let sonnet = &models[0];
        assert_eq!(sonnet.context_window, Some(200000));
        assert_eq!(sonnet.support_tools, Some(true));
        assert_eq!(sonnet.support_streaming, Some(true));
        assert_eq!(sonnet.support_reasoning, Some(true));
        assert!((sonnet.cost.unwrap().input - 3.0).abs() < f64::EPSILON);
        assert!((sonnet.cost.unwrap().output - 15.0).abs() < f64::EPSILON);
        let r1 = &models[1];
        assert_eq!(r1.support_tools, Some(false));
        assert_eq!(r1.support_streaming, Some(false));
        assert_eq!(r1.description, r1.name);
    }

    #[tokio::test]
    async fn an_openai_list_call_parses_the_page_and_sends_the_key() {
        let (base, seen) =
            serve_get(|_| json!({ "data": [{ "id": "gpt-4o" }, { "id": "o3-mini" }] }).to_string())
                .await;
        let provider = OpenAi {
            endpoint: endpoint(&format!("{base}/v1"), Some("dummy-token-abcd")),
        };

        let models = list_models("openai", &provider, Duration::from_secs(5))
            .await
            .unwrap();

        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "gpt-4o");
        assert_eq!(seen.paths.lock().unwrap()[0], "v1/models");
        assert_eq!(
            seen.auth.lock().unwrap()[0].0.as_deref(),
            Some("Bearer dummy-token-abcd")
        );
    }

    #[tokio::test]
    async fn a_custom_list_call_needs_no_key() {
        let (base, seen) =
            serve_get(|_| json!({ "data": [{ "id": "local-7b" }] }).to_string()).await;
        let provider = CustomOpenAi {
            endpoint: endpoint(&base, None),
        };

        let models = list_models("custom:local", &provider, Duration::from_secs(5))
            .await
            .unwrap();

        assert_eq!(models.len(), 1);
        assert_eq!(seen.auth.lock().unwrap()[0].0, None);
    }

    #[tokio::test]
    async fn a_fireworks_list_call_uses_the_openai_compatible_endpoint() {
        // `FireworksService` inherited `OpenAIService.listModels`, so
        // Fireworks had a live model list before #3091; the native command
        // must keep it through the same OpenAI-compatible call.
        let (base, seen) = serve_get(|_| {
            json!({ "data": [{ "id": "accounts/fireworks/models/llama-v3p1-70b-instruct" }] })
                .to_string()
        })
        .await;
        let provider = Fireworks {
            endpoint: endpoint(&base, Some("fw-key")),
        };

        let models = list_models("fireworks", &provider, Duration::from_secs(5))
            .await
            .unwrap();

        assert_eq!(models.len(), 1);
        assert_eq!(
            models[0].id,
            "accounts/fireworks/models/llama-v3p1-70b-instruct"
        );
        assert_eq!(seen.paths.lock().unwrap()[0], "models");
        assert_eq!(
            seen.auth.lock().unwrap()[0].0.as_deref(),
            Some("Bearer fw-key"),
            "Fireworks sends the bearer key, as the OpenAI-compatible services did"
        );
    }

    #[tokio::test]
    async fn anthropic_stops_when_the_server_repeats_the_same_cursor() {
        // `has_more: true` with a cursor that never advances must not loop
        // after the caller has already given up.
        let (base, seen) =
            serve_get(|_| json!({ "data": [{ "id": "claude-a" }], "has_more": true }).to_string())
                .await;
        let provider = Anthropic {
            endpoint: endpoint(&base, Some("sk-ant-key")),
        };

        let models = list_models("anthropic", &provider, Duration::from_secs(5))
            .await
            .unwrap();

        assert_eq!(seen.calls.load(Ordering::SeqCst), 2);
        assert_eq!(models.len(), 2);
    }

    /// A minimal HTTP proxy listener: records every absolute-form target it
    /// is asked for and answers `/models` with one listed model. Other
    /// targets get `502` so a stray request cannot hang the test.
    fn spawn_proxy_listener(recorded: Arc<Mutex<Vec<String>>>) -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let recorded = recorded.clone();
                std::thread::spawn(move || {
                    use std::io::{BufRead, BufReader, Write};
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let mut request_line = String::new();
                    if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
                        return;
                    }
                    // A client that honours the proxy sends
                    // `GET http://host/path HTTP/1.1`.
                    let target = request_line
                        .split_whitespace()
                        .nth(1)
                        .unwrap_or_default()
                        .to_string();
                    recorded.lock().unwrap().push(target.clone());
                    let response = if target.ends_with("/models") {
                        let body = r#"{"data":[{"id":"proxied-model"}]}"#;
                        format!(
                            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                            body.len()
                        )
                    } else {
                        "HTTP/1.1 502 Bad Gateway\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                            .to_string()
                    };
                    let _ = stream.write_all(response.as_bytes());
                    let _ = stream.flush();
                });
            }
        });
        port
    }

    #[tokio::test]
    async fn a_model_list_call_goes_through_the_system_proxy() {
        // Environment access is process-global, so keep this the only test
        // touching the proxy variables.
        static PROXY_ENV_LOCK: Mutex<()> = Mutex::new(());
        let _guard = PROXY_ENV_LOCK.lock().unwrap();

        let recorded = Arc::new(Mutex::new(Vec::new()));
        let proxy_port = spawn_proxy_listener(recorded.clone());
        let previous: Vec<(String, Option<String>)> =
            ["HTTP_PROXY", "http_proxy", "NO_PROXY", "no_proxy"]
                .iter()
                .map(|key| {
                    (
                        key.to_string(),
                        std::env::var_os(key).map(|v| v.to_string_lossy().into_owned()),
                    )
                })
                .collect();
        std::env::set_var("HTTP_PROXY", format!("http://127.0.0.1:{proxy_port}"));
        // Keep loopback direct so concurrent tests are not rerouted here.
        std::env::set_var("NO_PROXY", "127.0.0.1,localhost");

        // This origin host must not be reachable directly: only a client
        // built with the system-proxy configuration completes the call.
        let provider = CustomOpenAi {
            endpoint: endpoint("http://model-list-origin.invalid/v1", None),
        };
        let result = list_models("custom:proxy-test", &provider, Duration::from_secs(5)).await;

        for (key, value) in previous {
            match value {
                Some(v) => std::env::set_var(&key, v),
                None => std::env::remove_var(&key),
            }
        }

        let models = result.expect("the list call must succeed through the system proxy");
        assert_eq!(models[0].id, "proxied-model");
        assert!(
            recorded
                .lock()
                .unwrap()
                .iter()
                .any(|target| target == "http://model-list-origin.invalid/v1/models"),
            "the proxy never saw the model list request"
        );
    }

    #[tokio::test]
    async fn gemini_pages_until_the_last_page_and_sends_the_header_key() {
        let (base, seen) = serve_get(|call| match call {
            0 => json!({
                "models": [{ "name": "models/gemini-2.5-flash", "supportedGenerationMethods": ["generateContent"] }],
                "nextPageToken": "p2"
            })
            .to_string(),
            _ => json!({
                "models": [{ "name": "models/gemini-2.5-pro", "supportedGenerationMethods": ["generateContent"] }]
            })
            .to_string(),
        })
        .await;
        let provider = Gemini {
            endpoint: endpoint(&base, Some("gm-key")),
        };

        let models = list_models("gemini", &provider, Duration::from_secs(5))
            .await
            .unwrap();

        assert_eq!(models.len(), 2);
        assert_eq!(seen.calls.load(Ordering::SeqCst), 2);
        assert_eq!(seen.queries.lock().unwrap()[0]["page_size"], "100");
        assert!(!seen.queries.lock().unwrap()[0].contains_key("page_token"));
        assert_eq!(seen.queries.lock().unwrap()[1]["page_token"], "p2");
        assert_eq!(
            seen.auth.lock().unwrap()[0].2.as_deref(),
            Some("gm-key"),
            "the key goes in the x-goog-api-key header"
        );
    }

    #[tokio::test]
    async fn anthropic_pages_with_after_id_and_sends_the_api_key() {
        let (base, seen) = serve_get(|call| match call {
            0 => json!({
                "data": [{ "id": "claude-a", "display_name": "A" }],
                "has_more": true
            })
            .to_string(),
            _ => json!({ "data": [{ "id": "claude-b" }], "has_more": false }).to_string(),
        })
        .await;
        let provider = Anthropic {
            endpoint: endpoint(&base, Some("sk-ant-key")),
        };

        let models = list_models("anthropic", &provider, Duration::from_secs(5))
            .await
            .unwrap();

        assert_eq!(models.len(), 2);
        assert_eq!(models[1].name, "claude-b");
        assert_eq!(seen.queries.lock().unwrap()[1]["after_id"], "claude-a");
        assert_eq!(
            seen.auth.lock().unwrap()[0].1.as_deref(),
            Some("sk-ant-key"),
            "the key goes in the x-api-key header"
        );
    }

    #[tokio::test]
    async fn an_ollama_list_call_reads_the_tags_endpoint() {
        let (base, seen) = serve_get(|_| {
            json!({ "models": [{ "name": "qwen2.5:7b", "details": { "family": "qwen" } }] })
                .to_string()
        })
        .await;
        let provider = Ollama {
            endpoint: endpoint(&base, None),
        };

        let models = list_models("ollama", &provider, Duration::from_secs(5))
            .await
            .unwrap();

        assert_eq!(models[0].id, "qwen2.5:7b");
        assert_eq!(seen.paths.lock().unwrap()[0], "api/tags");
    }

    #[tokio::test]
    async fn an_http_error_keeps_the_provider_classification() {
        let (addr, server) = warp::serve(warp::get().map(|| {
            warp::http::Response::builder()
                .status(401)
                .body(r#"{"error":{"message":"Incorrect API key provided"}}"#)
                .unwrap()
        }))
        .bind_ephemeral(([127, 0, 0, 1], 0));
        tokio::spawn(server);
        let provider = CustomOpenAi {
            endpoint: endpoint(&format!("http://{addr}/v1"), Some("bad-key")),
        };

        let error = list_models("openai", &provider, Duration::from_secs(5))
            .await
            .unwrap_err();

        assert_eq!(error.kind, ErrorKind::Authentication);
        assert!(error.message.contains("HTTP 401"));
        assert!(error.message.contains("Incorrect API key"));
    }

    #[tokio::test]
    async fn a_server_that_never_answers_hits_the_timeout() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            // Accept the connection and stay silent until past the timeout.
            let _accepted = listener.accept();
            std::thread::sleep(Duration::from_secs(5));
        });
        let provider = CustomOpenAi {
            endpoint: endpoint(&format!("http://127.0.0.1:{port}/v1"), None),
        };

        let error = list_models("openai", &provider, Duration::from_millis(250))
            .await
            .unwrap_err();

        assert_eq!(error.kind, ErrorKind::Network);
        // Whatever the transport reports, it must be the timeout, not a
        // connection refused.
        let lower = error.message.to_lowercase();
        assert!(
            lower.contains("timeout") || lower.contains("timed out"),
            "message was: {}",
            error.message
        );
    }

    #[tokio::test]
    async fn a_provider_without_a_list_api_is_refused() {
        let provider = CustomOpenAi {
            endpoint: endpoint("http://127.0.0.1:1", None),
        };
        let error = list_models("groq", &provider, Duration::from_secs(5))
            .await
            .unwrap_err();
        assert!(error.message.contains("no model list API"));
    }
}
