//! The provider trait and the OpenAI-compatible providers.
//!
//! The trait's default methods are the OpenAI-compatible behaviour. A provider
//! that speaks the same API overrides only what differs, as the frontend's
//! `FireworksService` and `OpenRouterService` extend `OpenAIService`.

use crate::anthropic::Anthropic;
use crate::error::LlmError;
use crate::gemini::Gemini;
use crate::openai;
use crate::stream::{OpenAiStream, StreamParser};
use crate::types::{ChatRequest, ThinkingEffort};
use serde_json::{json, Value};

/// Where and how to reach the provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// Base URL up to and including the API version, for example
    /// `https://api.openai.com/v1` or `http://127.0.0.1:8000/v1`.
    pub base_url: String,
    /// Sent as a bearer token when present. Local servers often need none.
    pub api_key: Option<String>,
}

/// One LLM provider.
pub trait ChatProvider: Send + Sync {
    fn endpoint(&self) -> &Endpoint;

    /// Where a chat call for `model` (as [`Self::model_id`] gives it) is posted.
    fn request_url(&self, _model: &str) -> String {
        format!(
            "{}/chat/completions",
            self.endpoint().base_url.trim_end_matches('/')
        )
    }

    /// The header that carries `api_key`.
    fn auth_header(&self, api_key: &str) -> (&'static str, String) {
        ("authorization", format!("Bearer {api_key}"))
    }

    /// Headers sent with every call, besides auth and content type.
    fn extra_headers(&self) -> &[(&'static str, &'static str)] {
        &[]
    }

    /// The model id as this provider expects it.
    fn model_id(&self, model: &str) -> String {
        model.to_string()
    }

    /// Adds the provider's reasoning parameters for `effort` to `body`.
    /// `auto` is sent as `medium`, as in the GUI.
    fn apply_thinking_effort(&self, body: &mut Value, effort: ThinkingEffort) {
        let level = match effort {
            ThinkingEffort::Off => return,
            ThinkingEffort::Low => "low",
            ThinkingEffort::Medium | ThinkingEffort::Auto => "medium",
            ThinkingEffort::High => "high",
        };
        body["reasoning_effort"] = json!(level);
    }

    /// A prompt-cache key for `body`, when the provider supports one.
    fn prompt_cache_key(&self, _request: &ChatRequest, _body: &Value) -> Option<String> {
        None
    }

    /// The request body for `request`.
    fn build_body(&self, request: &ChatRequest) -> Value {
        let mut body = openai::request_body(&self.model_id(&request.model), request);
        self.apply_thinking_effort(&mut body, request.thinking_effort);
        if let Some(key) = self.prompt_cache_key(request, &body) {
            body["prompt_cache_key"] = json!(key);
        }
        body
    }

    /// A fresh parser for one response stream.
    fn stream_parser(&self) -> Box<dyn StreamParser> {
        Box::<OpenAiStream>::default()
    }

    /// The error for an HTTP response that was not a success.
    fn http_error(&self, status: u16, body: &str) -> LlmError {
        LlmError::from_http_status(status, body)
    }
}

/// The built-in `openai` provider: OpenAI or a server at its Base URL setting.
#[derive(Debug, Clone)]
pub struct OpenAi {
    pub endpoint: Endpoint,
}

const OPENAI_HOST: &str = "api.openai.com";

impl ChatProvider for OpenAi {
    fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    /// Only the official endpoint gets a key; other servers may reject the
    /// field. Same key shape as the GUI's automatic key.
    fn prompt_cache_key(&self, request: &ChatRequest, body: &Value) -> Option<String> {
        let official = reqwest::Url::parse(&self.endpoint.base_url)
            .is_ok_and(|url| url.host_str() == Some(OPENAI_HOST));
        let system_prompt = request.system_prompt.as_deref().unwrap_or_default();
        if !official || (system_prompt.is_empty() && request.tools.is_empty()) {
            return None;
        }
        let tools = body.get("tools").cloned().unwrap_or_else(|| json!([]));
        Some(format!(
            "chat:{}:{}:{}",
            body["model"].as_str().unwrap_or_default(),
            openai::hash_key_part(system_prompt),
            openai::hash_key_part(&openai::stable_stringify(&tools)),
        ))
    }
}

/// A custom OpenAI-compatible provider from Settings (`custom:<id>`), for
/// example vLLM or LM Studio. Uses every default.
#[derive(Debug, Clone)]
pub struct CustomOpenAi {
    pub endpoint: Endpoint,
}

impl ChatProvider for CustomOpenAi {
    fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }
}

/// Fireworks AI: model ids live under `accounts/fireworks/models/`.
#[derive(Debug, Clone)]
pub struct Fireworks {
    pub endpoint: Endpoint,
}

const FIREWORKS_MODEL_PREFIX: &str = "accounts/fireworks/models/";

impl ChatProvider for Fireworks {
    fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    fn model_id(&self, model: &str) -> String {
        if model.starts_with(FIREWORKS_MODEL_PREFIX) {
            model.to_string()
        } else {
            format!("{FIREWORKS_MODEL_PREFIX}{model}")
        }
    }
}

/// OpenRouter: sends the attribution headers OpenRouter recommends.
#[derive(Debug, Clone)]
pub struct OpenRouter {
    pub endpoint: Endpoint,
}

impl ChatProvider for OpenRouter {
    fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    fn extra_headers(&self) -> &[(&'static str, &'static str)] {
        &[
            ("HTTP-Referer", "https://github.com/fritzprix/libr-agent"),
            ("X-Title", "LibrAgent"),
        ]
    }
}

const OPENAI_PROVIDER: &str = "openai";
const FIREWORKS_PROVIDER: &str = "fireworks";
const OPENROUTER_PROVIDER: &str = "openrouter";
const ANTHROPIC_PROVIDER: &str = "anthropic";
const GEMINI_PROVIDER: &str = "gemini";
const CUSTOM_PROVIDER_PREFIX: &str = "custom:";
const OPENAI_DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const FIREWORKS_BASE_URL: &str = "https://api.fireworks.ai/inference/v1";
const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";
const ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com/v1";
const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

/// True for the providers that speak the OpenAI chat-completions API.
pub fn is_openai_compatible(provider: &str) -> bool {
    matches!(
        provider,
        OPENAI_PROVIDER | FIREWORKS_PROVIDER | OPENROUTER_PROVIDER
    ) || provider
        .strip_prefix(CUSTOM_PROVIDER_PREFIX)
        .is_some_and(|id| !id.is_empty())
}

/// True for the providers this crate implements.
pub fn is_supported(provider: &str) -> bool {
    is_openai_compatible(provider) || matches!(provider, ANTHROPIC_PROVIDER | GEMINI_PROVIDER)
}

/// The provider `provider` talking to `endpoint`, or `None` for a provider
/// this crate does not implement.
pub fn provider_with_endpoint(provider: &str, endpoint: Endpoint) -> Option<Box<dyn ChatProvider>> {
    if !is_supported(provider) {
        return None;
    }
    Some(match provider {
        OPENAI_PROVIDER => Box::new(OpenAi { endpoint }),
        FIREWORKS_PROVIDER => Box::new(Fireworks { endpoint }),
        OPENROUTER_PROVIDER => Box::new(OpenRouter { endpoint }),
        ANTHROPIC_PROVIDER => Box::new(Anthropic { endpoint }),
        GEMINI_PROVIDER => Box::new(Gemini { endpoint }),
        _ => Box::new(CustomOpenAi { endpoint }),
    })
}

/// Builds `provider` from the `serviceConfigs` and `customProviders` settings
/// values, the same data the frontend reads.
///
/// Fireworks, OpenRouter, Anthropic, and Gemini have fixed URLs; only OpenAI
/// and custom providers take a Base URL, as in the Settings screen.
pub fn resolve_provider(
    provider: &str,
    service_configs: Option<&Value>,
    custom_providers: Option<&Value>,
) -> Result<Box<dyn ChatProvider>, String> {
    let non_empty = |value: Option<&Value>| {
        value
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    };

    let endpoint = if let Some(id) = provider.strip_prefix(CUSTOM_PROVIDER_PREFIX) {
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
        Endpoint {
            base_url,
            api_key: non_empty(entry.get("apiKey")),
        }
    } else {
        let config = service_configs.and_then(|configs| configs.get(provider));
        let base_url = match provider {
            FIREWORKS_PROVIDER => FIREWORKS_BASE_URL.to_string(),
            OPENROUTER_PROVIDER => OPENROUTER_BASE_URL.to_string(),
            ANTHROPIC_PROVIDER => ANTHROPIC_BASE_URL.to_string(),
            GEMINI_PROVIDER => GEMINI_BASE_URL.to_string(),
            _ => non_empty(config.and_then(|config| config.get("baseUrl")))
                .unwrap_or_else(|| OPENAI_DEFAULT_BASE_URL.to_string()),
        };
        Endpoint {
            base_url,
            api_key: non_empty(config.and_then(|config| config.get("apiKey"))),
        }
    };

    provider_with_endpoint(provider, endpoint)
        .ok_or_else(|| format!("Provider '{provider}' is not supported"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ChatMessage, ContentPart, Role, ToolSpec};

    fn endpoint(base_url: &str) -> Endpoint {
        Endpoint {
            base_url: base_url.to_string(),
            api_key: None,
        }
    }

    fn request() -> ChatRequest {
        ChatRequest {
            model: "gpt-4.1".to_string(),
            system_prompt: Some("You are an agent.".to_string()),
            messages: vec![ChatMessage {
                role: Role::User,
                content: vec![ContentPart::Text("hi".to_string())],
                tool_calls: Vec::new(),
                tool_call_id: None,
                thinking: None,
                thinking_signature: None,
            }],
            tools: vec![ToolSpec {
                name: "t".to_string(),
                description: "d".to_string(),
                parameters: json!({ "type": "object" }),
            }],
            ..ChatRequest::default()
        }
    }

    #[test]
    fn openai_compatible_providers_are_recognised() {
        for provider in ["openai", "fireworks", "openrouter", "custom:abc123"] {
            assert!(is_openai_compatible(provider), "{provider}");
        }
        for provider in ["anthropic", "gemini"] {
            assert!(!is_openai_compatible(provider), "{provider}");
            assert!(is_supported(provider), "{provider}");
        }
        for provider in ["custom:", "groq", "cerebras", "ollama", ""] {
            assert!(!is_supported(provider), "{provider}");
        }
        assert!(provider_with_endpoint("groq", endpoint("http://x")).is_none());
    }

    #[test]
    fn anthropic_and_gemini_use_their_fixed_urls_and_configured_keys() {
        let configs = json!({
            "anthropic": { "apiKey": "sk-ant-key", "baseUrl": "http://ignored" },
            "gemini": { "apiKey": "gm-key" },
        });
        let anthropic = resolve_provider("anthropic", Some(&configs), None).unwrap();
        assert_eq!(
            anthropic.request_url("claude-sonnet-4-5"),
            "https://api.anthropic.com/v1/messages"
        );
        assert_eq!(anthropic.endpoint().api_key.as_deref(), Some("sk-ant-key"));
        assert_eq!(
            anthropic.auth_header("sk-ant-key"),
            ("x-api-key", "sk-ant-key".to_string())
        );

        let gemini = resolve_provider("gemini", Some(&configs), None).unwrap();
        assert_eq!(
            gemini.request_url("gemini-2.5-flash"),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:streamGenerateContent?alt=sse"
        );
        assert_eq!(
            gemini.auth_header("gm-key"),
            ("x-goog-api-key", "gm-key".to_string())
        );
    }

    #[test]
    fn openai_uses_its_service_config_and_falls_back_to_the_public_api() {
        let configs = json!({
            "openai": { "apiKey": "dummy-token-abcd", "baseUrl": "http://127.0.0.1:8000/v1" },
        });
        let provider = resolve_provider("openai", Some(&configs), None).unwrap();
        assert_eq!(
            provider.endpoint(),
            &Endpoint {
                base_url: "http://127.0.0.1:8000/v1".to_string(),
                api_key: Some("dummy-token-abcd".to_string()),
            }
        );
        let provider = resolve_provider("openai", None, None).unwrap();
        assert_eq!(provider.endpoint(), &endpoint(OPENAI_DEFAULT_BASE_URL));
    }

    #[test]
    fn fireworks_and_openrouter_keep_their_fixed_urls() {
        let configs = json!({
            "fireworks": { "apiKey": "fw-key", "baseUrl": "http://ignored" },
            "openrouter": { "apiKey": "sk-or-key" },
        });
        let fireworks = resolve_provider("fireworks", Some(&configs), None).unwrap();
        assert_eq!(fireworks.endpoint().base_url, FIREWORKS_BASE_URL);
        assert_eq!(fireworks.endpoint().api_key.as_deref(), Some("fw-key"));
        let openrouter = resolve_provider("openrouter", Some(&configs), None).unwrap();
        assert_eq!(
            openrouter.request_url("m"),
            "https://openrouter.ai/api/v1/chat/completions"
        );
    }

    #[test]
    fn a_custom_provider_needs_its_settings_entry_and_a_base_url() {
        let providers = json!([
            { "id": "lmstudio", "name": "LM Studio", "baseUrl": "http://localhost:1234/v1/" },
            { "id": "broken", "name": "No URL", "baseUrl": "  " },
        ]);

        let provider = resolve_provider("custom:lmstudio", None, Some(&providers)).unwrap();
        assert_eq!(
            provider.request_url("m"),
            "http://localhost:1234/v1/chat/completions"
        );
        assert_eq!(
            provider.auth_header("k"),
            ("authorization", "Bearer k".to_string())
        );
        let error = |id: &str| resolve_provider(id, None, Some(&providers)).err().unwrap();
        assert!(error("custom:broken").contains("no base URL"));
        assert!(error("custom:missing").contains("not in Settings"));
        assert!(error("groq").contains("not supported"));
    }

    #[test]
    fn thinking_effort_maps_to_reasoning_effort_and_auto_is_medium() {
        let provider = CustomOpenAi {
            endpoint: endpoint("http://localhost:1234/v1"),
        };
        let effort = |effort| {
            let body = provider.build_body(&ChatRequest {
                thinking_effort: effort,
                ..ChatRequest::default()
            });
            body.get("reasoning_effort").cloned()
        };
        assert_eq!(effort(ThinkingEffort::Off), None);
        assert_eq!(effort(ThinkingEffort::Low), Some(json!("low")));
        assert_eq!(effort(ThinkingEffort::Auto), Some(json!("medium")));
        assert_eq!(effort(ThinkingEffort::High), Some(json!("high")));
    }

    #[test]
    fn fireworks_prefixes_bare_model_ids_once() {
        let provider = Fireworks {
            endpoint: endpoint(FIREWORKS_BASE_URL),
        };
        assert_eq!(
            provider.model_id("llama-v3p1-70b-instruct"),
            "accounts/fireworks/models/llama-v3p1-70b-instruct"
        );
        assert_eq!(
            provider.model_id("accounts/fireworks/models/qwen3"),
            "accounts/fireworks/models/qwen3"
        );
        let body = provider.build_body(&ChatRequest {
            model: "qwen3".to_string(),
            ..ChatRequest::default()
        });
        assert_eq!(body["model"], "accounts/fireworks/models/qwen3");
    }

    #[test]
    fn only_the_official_openai_endpoint_gets_a_stable_prompt_cache_key() {
        let official = OpenAi {
            endpoint: endpoint(OPENAI_DEFAULT_BASE_URL),
        };
        let first = official.build_body(&request());
        let key = first["prompt_cache_key"].as_str().expect("a cache key");
        assert!(key.starts_with("chat:gpt-4.1:"), "{key}");
        // The history is not part of the key, so it survives new turns.
        let mut longer = request();
        longer.messages.push(longer.messages[0].clone());
        assert_eq!(official.build_body(&longer)["prompt_cache_key"], key);

        // Nothing to cache without a system prompt or tools.
        let bare = official.build_body(&ChatRequest::default());
        assert!(bare.get("prompt_cache_key").is_none());

        // A self-hosted server behind the `openai` provider may reject the field.
        let local = OpenAi {
            endpoint: endpoint("http://127.0.0.1:8000/v1"),
        };
        assert!(local
            .build_body(&request())
            .get("prompt_cache_key")
            .is_none());
    }

    #[test]
    fn openrouter_adds_its_attribution_headers() {
        let provider = OpenRouter {
            endpoint: endpoint(OPENROUTER_BASE_URL),
        };
        assert!(provider.extra_headers().contains(&("X-Title", "LibrAgent")));
        assert!(CustomOpenAi {
            endpoint: endpoint("http://x")
        }
        .extra_headers()
        .is_empty());
    }
}
