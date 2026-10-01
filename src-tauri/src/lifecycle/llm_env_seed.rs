//! Seed preferred LLM provider settings from environment variables.
//!
//! Used heavily by the **demo** profile so a clean DB boots with a working model.
//! Also applies on prod/dev when the vars are set (explicit opt-in via env).
//!
//! Variables (any one of the aliases):
//! - `LIBRAGENT_LLM_PROVIDER` / `LIBRAGENT_PROVIDER` — e.g. `openai`, `ollama`, `anthropic`
//! - `LIBRAGENT_LLM_MODEL` / `LIBRAGENT_MODEL`
//! - `LIBRAGENT_LLM_API_KEY` / `LIBRAGENT_API_KEY` / provider-native (`OPENAI_API_KEY`, …)
//! - `LIBRAGENT_LLM_BASE_URL` / `LIBRAGENT_BASE_URL` — optional OpenAI-compatible base URL

use crate::repositories::settings_repository::SettingsRepository;
use crate::state::try_get_settings_repository;
use serde_json::{json, Value};
use std::env;

fn first_env(keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Ok(value) = env::var(key) {
            let trimmed = value.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    None
}

fn api_key_for_provider(provider: &str) -> Option<String> {
    if let Some(key) = first_env(&["LIBRAGENT_LLM_API_KEY", "LIBRAGENT_API_KEY"]) {
        return Some(key);
    }
    match provider {
        "openai" => first_env(&["OPENAI_API_KEY"]),
        "anthropic" => first_env(&["ANTHROPIC_API_KEY"]),
        "gemini" => first_env(&["GEMINI_API_KEY", "GOOGLE_API_KEY"]),
        "groq" => first_env(&["GROQ_API_KEY"]),
        "openrouter" => first_env(&["OPENROUTER_API_KEY"]),
        "fireworks" => first_env(&["FIREWORKS_API_KEY"]),
        "cerebras" => first_env(&["CEREBRAS_API_KEY"]),
        "ollama" => None, // local; key optional
        _ => None,
    }
}

fn known_provider(provider: &str) -> bool {
    matches!(
        provider,
        "openai"
            | "anthropic"
            | "gemini"
            | "groq"
            | "openrouter"
            | "fireworks"
            | "cerebras"
            | "ollama"
    )
}

/// Apply env-based LLM settings into the settings repository when configured.
pub async fn seed_llm_settings_from_env() {
    let Some(provider) = first_env(&["LIBRAGENT_LLM_PROVIDER", "LIBRAGENT_PROVIDER"]) else {
        return;
    };
    let Some(model) = first_env(&["LIBRAGENT_LLM_MODEL", "LIBRAGENT_MODEL"]) else {
        log::warn!(
            "LIBRAGENT_LLM_PROVIDER set ({provider}) but LIBRAGENT_LLM_MODEL / LIBRAGENT_MODEL missing; skip LLM env seed"
        );
        return;
    };

    if !known_provider(&provider) {
        log::warn!(
            "Unknown LIBRAGENT_LLM_PROVIDER '{provider}'; expected a builtin id (openai, ollama, …); skip seed"
        );
        return;
    }

    let Some(repo) = try_get_settings_repository() else {
        log::warn!("Settings repository unavailable; skip LLM env seed");
        return;
    };

    let base_url = first_env(&["LIBRAGENT_LLM_BASE_URL", "LIBRAGENT_BASE_URL"]);
    let api_key = api_key_for_provider(&provider);

    if provider != "ollama" && api_key.is_none() && base_url.is_none() {
        log::warn!(
            "LLM env seed for '{provider}': no API key or base URL found; writing preferredModel only"
        );
    }

    // Merge into serviceConfigs
    let mut service_configs = match repo.get("serviceConfigs").await {
        Ok(Some(row)) => serde_json::from_str::<Value>(&row.value).unwrap_or_else(|_| json!({})),
        Ok(None) => json!({}),
        Err(e) => {
            log::error!("Failed to read serviceConfigs for LLM env seed: {e}");
            return;
        }
    };
    if !service_configs.is_object() {
        service_configs = json!({});
    }

    let mut provider_cfg = service_configs
        .get(&provider)
        .cloned()
        .unwrap_or_else(|| json!({}));
    if let Some(key) = api_key {
        provider_cfg["apiKey"] = json!(key);
    }
    if let Some(url) = base_url {
        provider_cfg["baseUrl"] = json!(url);
    }
    service_configs[&provider] = provider_cfg;

    if let Err(e) = repo.set("serviceConfigs", service_configs).await {
        log::error!("Failed to seed serviceConfigs from env: {e}");
        return;
    }

    let preferred = json!({ "provider": provider, "model": model });
    if let Err(e) = repo.set("preferredModel", preferred).await {
        log::error!("Failed to seed preferredModel from env: {e}");
        return;
    }

    log::info!("✅ Seeded LLM settings from env (provider={provider}, model={model})");
    println!("✅ LLM env seed: provider={provider} model={model}");
}
