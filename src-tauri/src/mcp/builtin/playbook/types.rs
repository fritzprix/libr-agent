use crate::entity::playbook;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TargetSessionMode {
    #[serde(rename = "self")]
    Self_,
    Pin,
}

/// Minimal Start launch target (`pin` + sessionId). No multi-slot orchestration.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TargetSessionConfig {
    pub mode: TargetSessionMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

impl TargetSessionConfig {
    pub fn pinned_session_id(&self) -> Option<&str> {
        if self.mode != TargetSessionMode::Pin {
            return None;
        }
        self.session_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlaybookAction {
    #[serde(rename = "toolName")]
    pub tool_name: String,
    pub purpose: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlaybookStep {
    #[serde(rename = "stepId")]
    pub step_id: Option<String>,
    pub description: String,
    pub action: PlaybookAction,
    #[serde(rename = "requiredData")]
    pub required_data: Option<Vec<String>>,
    #[serde(rename = "outputVariable")]
    pub output_variable: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SuccessCriteria {
    pub description: String,
    #[serde(rename = "requiredArtifacts")]
    pub required_artifacts: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Playbook {
    pub id: String,
    pub assistant_id: String,
    pub goal: String,
    #[serde(rename = "initialCommand")]
    pub initial_command: Option<String>,
    pub workflow: Vec<PlaybookStep>,
    #[serde(rename = "successCriteria")]
    pub success_criteria: Option<SuccessCriteria>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(rename = "isBookmarked")]
    pub is_bookmarked: bool,
    #[serde(
        rename = "defaultTargetSession",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub default_target_session: Option<TargetSessionConfig>,
}

/// Parse workflow JSON: preferred steps array; legacy envelope still readable.
pub fn parse_workflow_payload(raw: &str) -> (Vec<PlaybookStep>, Option<TargetSessionConfig>) {
    let value: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("Failed to parse playbook workflow JSON: {e}");
            return (Vec::new(), None);
        }
    };

    match value {
        Value::Array(_) => {
            let steps = deserialize_steps(value, "steps-array");
            (steps, None)
        }
        Value::Object(map) => {
            let steps = match map.get("steps") {
                Some(v) => deserialize_steps(v.clone(), "envelope.steps"),
                None => {
                    log::warn!("Playbook workflow envelope is missing 'steps'; treating as empty");
                    Vec::new()
                }
            };
            let default_target = map
                .get("defaultTargetSession")
                .and_then(parse_stored_target_session);
            (steps, default_target)
        }
        other => {
            log::warn!(
                "Unexpected playbook workflow JSON type: {}; treating as empty",
                match other {
                    Value::Null => "null",
                    Value::Bool(_) => "bool",
                    Value::Number(_) => "number",
                    Value::String(_) => "string",
                    Value::Array(_) | Value::Object(_) => unreachable!(),
                }
            );
            (Vec::new(), None)
        }
    }
}

fn deserialize_steps(value: Value, context: &str) -> Vec<PlaybookStep> {
    match serde_json::from_value::<Vec<PlaybookStep>>(value) {
        Ok(steps) => steps,
        Err(e) => {
            log::warn!("Failed to deserialize playbook workflow steps ({context}): {e}");
            Vec::new()
        }
    }
}

/// Validate a stored launch target; drop invalid pin configs with a warning.
fn parse_stored_target_session(value: &Value) -> Option<TargetSessionConfig> {
    if value.is_null() {
        return None;
    }
    match serde_json::from_value::<TargetSessionConfig>(value.clone()) {
        Ok(cfg) => {
            if cfg.mode == TargetSessionMode::Pin && cfg.pinned_session_id().is_none() {
                log::warn!(
                    "Ignoring defaultTargetSession pin without a non-empty sessionId"
                );
                None
            } else {
                Some(cfg)
            }
        }
        Err(e) => {
            log::warn!("Failed to parse defaultTargetSession: {e}");
            None
        }
    }
}

/// Serialize workflow steps only (pin lives in `default_target_session` column).
pub fn serialize_workflow_steps(steps: &[PlaybookStep]) -> Result<String, serde_json::Error> {
    serde_json::to_string(steps)
}

/// Serialize pin config for the `default_target_session` column.
pub fn serialize_default_target_session_column(
    target: Option<&TargetSessionConfig>,
) -> Result<Option<String>, serde_json::Error> {
    match target {
        None => Ok(None),
        Some(cfg) => {
            if cfg.mode == TargetSessionMode::Pin && cfg.pinned_session_id().is_none() {
                return Ok(None);
            }
            Ok(Some(serde_json::to_string(cfg)?))
        }
    }
}

fn parse_default_target_session_column(raw: Option<&str>) -> Option<TargetSessionConfig> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }
    let value: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("Failed to parse default_target_session column: {e}");
            return None;
        }
    };
    parse_stored_target_session(&value)
}

fn parse_target_session_config(value: &Value) -> Result<Option<TargetSessionConfig>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let cfg: TargetSessionConfig = serde_json::from_value(value.clone())
        .map_err(|e| format!("Invalid defaultTargetSession configuration: {}", e))?;
    // Pin without sessionId is allowed at parse time; callers fill from MCP session_id.
    Ok(Some(cfg))
}

impl Playbook {
    pub fn from_model(model: &playbook::Model) -> Self {
        let (workflow, envelope_target) = parse_workflow_payload(&model.workflow);
        let default_target_session =
            parse_default_target_session_column(model.default_target_session.as_deref())
                .or(envelope_target);
        Self {
            id: model.id.clone(),
            assistant_id: model.assistant_id.clone(),
            goal: model.goal.clone(),
            initial_command: model.initial_command.clone(),
            workflow,
            success_criteria: model
                .success_criteria
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok()),
            created_at: model.created_at,
            updated_at: model.updated_at,
            is_bookmarked: model.is_bookmarked,
            default_target_session,
        }
    }

    pub fn parse_default_target_session_arg(
        args: &Value,
    ) -> Result<Option<TargetSessionConfig>, String> {
        match args.get("defaultTargetSession") {
            Some(v) => parse_target_session_config(v),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_legacy_steps_array() {
        let raw = r#"[{"stepId":"s1","description":"d","action":{"toolName":"t","purpose":"p"},"requiredData":[],"outputVariable":"o"}]"#;
        let (steps, target) = parse_workflow_payload(raw);
        assert_eq!(steps.len(), 1);
        assert!(target.is_none());
    }

    #[test]
    fn parse_envelope_with_pin() {
        let raw = r#"{
            "steps":[{"stepId":"s1","description":"d","action":{"toolName":"t","purpose":"p"},"requiredData":[],"outputVariable":"o"}],
            "defaultTargetSession":{"mode":"pin","sessionId":"sess-1"}
        }"#;
        let (steps, target) = parse_workflow_payload(raw);
        assert_eq!(steps.len(), 1);
        let target = target.expect("pin present");
        assert_eq!(target.mode, TargetSessionMode::Pin);
        assert_eq!(target.pinned_session_id(), Some("sess-1"));
    }

    #[test]
    fn serialize_steps_only() {
        let steps = vec![PlaybookStep {
            step_id: Some("s1".into()),
            description: "d".into(),
            action: PlaybookAction {
                tool_name: "t".into(),
                purpose: "p".into(),
            },
            required_data: Some(vec![]),
            output_variable: "o".into(),
        }];
        let json = serialize_workflow_steps(&steps).unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();
        assert!(value.is_array());
    }

    #[test]
    fn parse_malformed_steps_returns_empty_with_no_panic() {
        let raw = r#"[{"description":"missing-required-fields"}]"#;
        let (steps, target) = parse_workflow_payload(raw);
        assert!(steps.is_empty());
        assert!(target.is_none());
    }

    #[test]
    fn parse_pin_without_session_id_is_dropped() {
        let raw = r#"{
            "steps":[{"stepId":"s1","description":"d","action":{"toolName":"t","purpose":"p"},"requiredData":[],"outputVariable":"o"}],
            "defaultTargetSession":{"mode":"pin"}
        }"#;
        let (steps, target) = parse_workflow_payload(raw);
        assert_eq!(steps.len(), 1);
        assert!(target.is_none());
    }

    #[test]
    fn parse_step_without_optional_fields() {
        let raw = r#"[{"description":"d","action":{"toolName":"t","purpose":"p"},"outputVariable":"o"}]"#;
        let (steps, target) = parse_workflow_payload(raw);
        assert_eq!(steps.len(), 1);
        assert!(steps[0].step_id.is_none());
        assert!(steps[0].required_data.is_none());
        assert!(target.is_none());
    }

    #[test]
    fn column_roundtrip_pin() {
        let pin = TargetSessionConfig {
            mode: TargetSessionMode::Pin,
            session_id: Some("sess-9".into()),
        };
        let col = serialize_default_target_session_column(Some(&pin))
            .unwrap()
            .expect("pin json");
        let parsed = parse_default_target_session_column(Some(&col)).expect("parse");
        assert_eq!(parsed.pinned_session_id(), Some("sess-9"));
    }

    #[test]
    fn from_model_prefers_column_over_envelope() {
        let model = playbook::Model {
            id: "pb1".into(),
            assistant_id: "a1".into(),
            goal: "g".into(),
            initial_command: None,
            workflow: r#"{"steps":[{"description":"d","action":{"toolName":"t","purpose":"p"},"outputVariable":"o"}],"defaultTargetSession":{"mode":"pin","sessionId":"envelope"}}"#.into(),
            success_criteria: None,
            default_target_session: Some(
                r#"{"mode":"pin","sessionId":"column"}"#.into(),
            ),
            created_at: 1,
            updated_at: 1,
            is_bookmarked: false,
        };
        let pb = Playbook::from_model(&model);
        assert_eq!(
            pb.default_target_session
                .as_ref()
                .and_then(|t| t.pinned_session_id()),
            Some("column")
        );
        assert_eq!(pb.workflow.len(), 1);
    }
}
