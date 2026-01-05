use crate::entity::playbook;
use serde::{Deserialize, Serialize};
use sqlx::Row;

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
    pub session_id: String,
    pub goal: String,
    #[serde(rename = "initialCommand")]
    pub initial_command: Option<String>,
    pub workflow: Vec<PlaybookStep>,
    #[serde(rename = "successCriteria")]
    pub success_criteria: Option<SuccessCriteria>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Playbook {
    pub fn from_row(row: &sqlx::sqlite::SqliteRow) -> Self {
        let workflow_str: String = row.get("workflow");
        let success_criteria_str: Option<String> = row.get("success_criteria");

        Self {
            id: row.get("id"),
            session_id: row.get("session_id"),
            goal: row.get("goal"),
            initial_command: row.get("initial_command"),
            workflow: serde_json::from_str(&workflow_str).unwrap_or_default(),
            success_criteria: success_criteria_str.and_then(|s| serde_json::from_str(&s).ok()),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    pub fn from_model(model: &playbook::Model) -> Self {
        Self {
            id: model.id.clone(),
            session_id: model.session_id.clone(),
            goal: model.goal.clone(),
            initial_command: model.initial_command.clone(),
            workflow: serde_json::from_str(&model.workflow).unwrap_or_default(),
            success_criteria: model
                .success_criteria
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok()),
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}
