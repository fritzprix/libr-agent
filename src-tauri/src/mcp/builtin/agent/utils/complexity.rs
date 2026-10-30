//! Complexity-based model routing for sub-agent delegation.
//!
//! Agents pass `complexity` (`low` | `normal` | `high`) on
//! `spawnSession` / `messageToSession`. Mapping comes from the
//! `complexityModelMapping` setting; unset levels inherit global
//! `preferredModel`.

use crate::mcp::builtin::error_guidance::{invalid_input_error, ToolGroup};
use crate::mcp::types::MCPResult;
use crate::repositories::settings_repository::SettingsRepository;
use serde_json::Value;

pub const COMPLEXITY_SETTING_KEY: &str = "complexityModelMapping";
pub const COMPLEXITY_LEVELS: &[&str] = &["low", "normal", "high"];

const COMPLEXITY_DESC: &str = concat!(
    "Task complexity level. Determines which pre-configured model/provider to use. ",
    "REQUIRED — choose the minimum sufficient complexity to minimize token cost. ",
    "Choose 'low' when possible; escalate only when necessary."
);

/// Description shared by spawnSession / messageToSession `complexity` schemas.
pub fn complexity_param_description() -> &'static str {
    COMPLEXITY_DESC
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedModelProvider {
    pub model: String,
    pub provider: String,
}

/// Parse and validate a required complexity level string.
pub fn parse_complexity(raw: &str) -> Result<&'static str, String> {
    match raw.trim() {
        "low" => Ok("low"),
        "normal" => Ok("normal"),
        "high" => Ok("high"),
        other => Err(format!(
            "Invalid complexity '{other}'. Expected one of: low, normal, high"
        )),
    }
}

/// Read and validate the required `complexity` argument of a delegation tool.
pub fn read_complexity(args: &Value) -> Result<&'static str, MCPResult> {
    let raw = super::read_required_string(args, "complexity")?;
    parse_complexity(&raw).map_err(|message| invalid_input_error(&message, ToolGroup::Agent))
}

/// Resolve an explicit model/provider override for a complexity level.
///
/// Returns `Ok(None)` when the level should keep the existing default chain
/// (parent inheritance → global preferredModel). That keeps unset mappings
/// zero-breaking with pre-complexity spawn/message behavior.
///
/// Mapping rules for `Ok(Some(_))`:
/// - level object must have non-empty `model` and `provider` strings
pub fn resolve_from_mapping(
    complexity: &str,
    mapping: Option<&Value>,
) -> Result<Option<ResolvedModelProvider>, String> {
    let level = parse_complexity(complexity)?;
    let Some(mapping) = mapping else {
        return Ok(None);
    };
    if !mapping.is_object() {
        return Ok(None);
    }
    let Some(entry) = mapping.get(level) else {
        return Ok(None);
    };
    if entry.is_null() {
        return Ok(None);
    }
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
        (Some(model), Some(provider)) => Ok(Some(ResolvedModelProvider {
            model: model.to_string(),
            provider: provider.to_string(),
        })),
        _ => Ok(None),
    }
}

/// Load optional complexityModelMapping JSON object from settings.
async fn load_complexity_mapping() -> Option<Value> {
    let repo = crate::state::try_get_settings_repository()?;
    match repo.get(COMPLEXITY_SETTING_KEY).await {
        Ok(Some(setting)) => match serde_json::from_str::<Value>(&setting.value) {
            Ok(val) if val.is_object() => Some(val),
            Ok(_) => {
                log::warn!(
                    "{COMPLEXITY_SETTING_KEY} is not a JSON object; falling back to preferredModel"
                );
                None
            }
            Err(e) => {
                log::warn!(
                    "Invalid {COMPLEXITY_SETTING_KEY} JSON ({e}); falling back to preferredModel"
                );
                None
            }
        },
        Ok(None) => None,
        Err(e) => {
            log::warn!("Failed to read {COMPLEXITY_SETTING_KEY}: {e}");
            None
        }
    }
}

/// Resolve an optional model/provider override for a complexity level.
///
/// `Ok(None)` means no mapping override — callers should leave the existing
/// parent-inheritance / preferredModel chain unchanged.
pub async fn resolve_model_for_complexity(
    complexity: &str,
) -> Result<Option<ResolvedModelProvider>, String> {
    let mapping = load_complexity_mapping().await;
    resolve_from_mapping(complexity, mapping.as_ref())
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use crate::mcp::types::MCPContent;
    use serde_json::json;

    #[test]
    fn parse_complexity_accepts_levels() {
        assert_eq!(parse_complexity("low").unwrap(), "low");
        assert_eq!(parse_complexity(" normal ").unwrap(), "normal");
        assert!(parse_complexity("medium").is_err());
    }

    fn error_text(result: &MCPResult) -> String {
        assert_eq!(result.is_error, Some(true));
        result
            .content
            .as_ref()
            .and_then(|c| c.first())
            .and_then(|item| match item {
                MCPContent::Text { text } => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    #[test]
    fn read_complexity_missing_returns_param_guidance() {
        let args = json!({ "configId": "cfg", "task": "do it" });
        let text = error_text(&read_complexity(&args).unwrap_err());
        assert!(text.contains("Missing required parameter: 'complexity'"));
        assert!(!text.contains("system error"));
    }

    #[test]
    fn read_complexity_invalid_returns_input_guidance() {
        let args = json!({ "complexity": "medium" });
        let text = error_text(&read_complexity(&args).unwrap_err());
        assert!(text.contains("Invalid complexity 'medium'"));
        assert!(!text.contains("system error"));
    }

    #[test]
    fn read_required_string_missing_names_param() {
        let text =
            error_text(&super::super::read_required_string(&json!({}), "sessionId").unwrap_err());
        assert!(text.contains("'sessionId'"));
        assert!(!text.contains("system error"));
    }

    #[test]
    fn read_complexity_accepts_valid_level() {
        assert_eq!(
            read_complexity(&json!({ "complexity": "high" })).unwrap(),
            "high"
        );
    }

    #[test]
    fn resolve_none_when_mapping_absent() {
        assert!(resolve_from_mapping("high", None).unwrap().is_none());
    }

    #[test]
    fn resolve_uses_level_override() {
        let mapping = json!({
            "high": { "model": "claude-opus", "provider": "anthropic" }
        });
        let resolved = resolve_from_mapping("high", Some(&mapping))
            .unwrap()
            .expect("override");
        assert_eq!(resolved.model, "claude-opus");
        assert_eq!(resolved.provider, "anthropic");
        assert!(resolve_from_mapping("low", Some(&mapping))
            .unwrap()
            .is_none());
    }

    #[test]
    fn resolve_null_or_partial_is_none() {
        let mapping = json!({
            "low": null,
            "normal": { "model": "only-model" },
            "high": { "model": "", "provider": "anthropic" }
        });
        assert!(resolve_from_mapping("low", Some(&mapping))
            .unwrap()
            .is_none());
        assert!(resolve_from_mapping("normal", Some(&mapping))
            .unwrap()
            .is_none());
        assert!(resolve_from_mapping("high", Some(&mapping))
            .unwrap()
            .is_none());
    }
}
