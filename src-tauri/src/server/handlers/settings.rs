use crate::repositories::settings_repository::SettingsRepository;
use crate::state::try_get_settings_repository;
use serde::Serialize;
use warp::{http::StatusCode, Rejection, Reply};

use super::types::ErrorResponse;

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

#[cfg(test)]
mod tests {
    use super::{harbor_model_name, preferred_model_from_value};
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
}
