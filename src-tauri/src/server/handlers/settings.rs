use crate::repositories::settings_repository::SettingsRepository;
use crate::state::try_get_settings_repository;
use serde::Serialize;
use serde_json::{json, Value};
use warp::{http::StatusCode, Rejection, Reply};

use super::types::ErrorResponse;

const COMPLEXITY_SETTING_KEY: &str = "complexityModelMapping";

/// Response for `GET /api/settings/preferredModel`.
///
/// Mirrors the global-settings fallback used when creating a session without an
/// explicit model/provider (`agent/lifecycle/creation.rs`).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferredModelResponse {
    pub model: String,
    pub provider: String,
    /// Harbor CLI form: `provider/model` (or bare model when provider is empty).
    pub harbor_model: String,
}

fn harbor_model_name(model: &str, provider: &str) -> String {
    let model = model.trim();
    let provider = provider.trim();
    if model.is_empty() {
        return String::new();
    }
    if provider.is_empty() || model.contains('/') {
        return model.to_string();
    }
    format!("{provider}/{model}")
}

fn preferred_model_from_value(value: &serde_json::Value) -> (String, String) {
    let model = value
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("gpt-4")
        .to_string();
    let provider = value
        .get("provider")
        .and_then(|v| v.as_str())
        .unwrap_or("openai")
        .to_string();
    (model, provider)
}

/// GET /api/settings/preferredModel
pub async fn get_preferred_model() -> Result<impl Reply, Rejection> {
    let Some(repo) = try_get_settings_repository() else {
        return Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: "Settings repository is not initialized".to_string(),
            }),
            StatusCode::SERVICE_UNAVAILABLE,
        ));
    };

    let (model, provider) = match repo.get("preferredModel").await {
        Ok(Some(setting)) => match serde_json::from_str::<serde_json::Value>(&setting.value) {
            Ok(val) => preferred_model_from_value(&val),
            Err(_) => ("gpt-4".to_string(), "openai".to_string()),
        },
        Ok(None) => ("gpt-4".to_string(), "openai".to_string()),
        Err(e) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: format!("Failed to read preferredModel: {e}"),
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };

    let harbor_model = harbor_model_name(&model, &provider);
    Ok(warp::reply::with_status(
        warp::reply::json(&PreferredModelResponse {
            model,
            provider,
            harbor_model,
        }),
        StatusCode::OK,
    ))
}

fn empty_complexity_mapping() -> Value {
    json!({
        "low": null,
        "normal": null,
        "high": null,
    })
}

fn normalize_complexity_mapping(value: &Value) -> Result<Value, String> {
    let obj = value
        .as_object()
        .ok_or_else(|| "complexityModelMapping must be a JSON object".to_string())?;
    let mut out = serde_json::Map::new();
    for level in ["low", "normal", "high"] {
        match obj.get(level) {
            None | Some(Value::Null) => {
                out.insert(level.to_string(), Value::Null);
            }
            Some(entry) => {
                let model = entry
                    .get("model")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                let provider = entry
                    .get("provider")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                match (model, provider) {
                    (Some(model), Some(provider)) => {
                        out.insert(
                            level.to_string(),
                            json!({ "model": model, "provider": provider }),
                        );
                    }
                    (None, None) if entry.as_object().is_some_and(|o| o.is_empty()) => {
                        out.insert(level.to_string(), Value::Null);
                    }
                    _ => {
                        return Err(format!(
                            "complexityModelMapping.{level} must be null or {{ model, provider }}"
                        ));
                    }
                }
            }
        }
    }
    Ok(Value::Object(out))
}

/// GET /api/settings/complexityModelMapping
pub async fn get_complexity_model_mapping() -> Result<impl Reply, Rejection> {
    let Some(repo) = try_get_settings_repository() else {
        return Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: "Settings repository is not initialized".to_string(),
            }),
            StatusCode::SERVICE_UNAVAILABLE,
        ));
    };

    let mapping = match repo.get(COMPLEXITY_SETTING_KEY).await {
        Ok(Some(setting)) => match serde_json::from_str::<Value>(&setting.value) {
            Ok(val) => match normalize_complexity_mapping(&val) {
                Ok(normalized) => normalized,
                Err(_) => empty_complexity_mapping(),
            },
            Err(_) => empty_complexity_mapping(),
        },
        Ok(None) => empty_complexity_mapping(),
        Err(e) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: format!("Failed to read {COMPLEXITY_SETTING_KEY}: {e}"),
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&mapping),
        StatusCode::OK,
    ))
}

/// PUT /api/settings/complexityModelMapping
pub async fn put_complexity_model_mapping(body: Value) -> Result<impl Reply, Rejection> {
    let Some(repo) = try_get_settings_repository() else {
        return Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: "Settings repository is not initialized".to_string(),
            }),
            StatusCode::SERVICE_UNAVAILABLE,
        ));
    };

    let normalized = match normalize_complexity_mapping(&body) {
        Ok(value) => value,
        Err(error) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse { error }),
                StatusCode::BAD_REQUEST,
            ));
        }
    };

    if let Err(e) = repo.set(COMPLEXITY_SETTING_KEY, normalized.clone()).await {
        return Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: format!("Failed to save {COMPLEXITY_SETTING_KEY}: {e}"),
            }),
            StatusCode::INTERNAL_SERVER_ERROR,
        ));
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&normalized),
        StatusCode::OK,
    ))
}

#[cfg(test)]
mod tests {
    use super::{harbor_model_name, normalize_complexity_mapping, preferred_model_from_value};
    use serde_json::json;

    #[test]
    fn harbor_model_name_joins_provider() {
        assert_eq!(harbor_model_name("gpt-5.4", "openai"), "openai/gpt-5.4");
        assert_eq!(
            harbor_model_name("openrouter/foo", "openrouter"),
            "openrouter/foo"
        );
        assert_eq!(harbor_model_name("local-model", ""), "local-model");
    }

    #[test]
    fn preferred_model_from_value_reads_fields() {
        let (model, provider) =
            preferred_model_from_value(&json!({"model": "Qwen3", "provider": "openai"}));
        assert_eq!(model, "Qwen3");
        assert_eq!(provider, "openai");
    }

    #[test]
    fn normalize_complexity_mapping_accepts_partial() {
        let normalized = normalize_complexity_mapping(&json!({
            "high": { "model": "claude-opus", "provider": "anthropic" }
        }))
        .unwrap();
        assert!(normalized["low"].is_null());
        assert!(normalized["normal"].is_null());
        assert_eq!(normalized["high"]["model"], "claude-opus");
    }
}
