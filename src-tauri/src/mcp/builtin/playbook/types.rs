use crate::entity::playbook;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Mode of session targeting for playbook execution
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TargetSessionMode {
    #[serde(rename = "self")]
    Self_,
    Pin,
    Spawn,
}

impl Default for TargetSessionMode {
    fn default() -> Self {
        Self::Self_
    }
}

/// Session targeting configuration for an individual step or default playbook setting
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TargetSessionConfig {
    #[serde(default)]
    pub mode: TargetSessionMode,
    #[serde(rename = "sessionId", skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(rename = "configId", skip_serializing_if = "Option::is_none")]
    pub config_id: Option<String>,
    #[serde(rename = "sessionSlot", skip_serializing_if = "Option::is_none")]
    pub session_slot: Option<String>,
}

/// Named session slot definition for role-based multi-session routing
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SessionSlotConfig {
    #[serde(default)]
    pub mode: TargetSessionMode,
    #[serde(rename = "configId", skip_serializing_if = "Option::is_none")]
    pub config_id: Option<String>,
    #[serde(rename = "sessionId", skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(rename = "reuseAcrossSteps", skip_serializing_if = "Option::is_none")]
    pub reuse_across_steps: Option<bool>,
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
    #[serde(rename = "targetSession", skip_serializing_if = "Option::is_none")]
    pub target_session: Option<TargetSessionConfig>,
    #[serde(rename = "sessionSlot", skip_serializing_if = "Option::is_none")]
    pub session_slot: Option<String>,
    #[serde(rename = "promptTemplate", skip_serializing_if = "Option::is_none")]
    pub prompt_template: Option<String>,
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
    #[serde(
        rename = "defaultTargetSession",
        skip_serializing_if = "Option::is_none"
    )]
    pub default_target_session: Option<TargetSessionConfig>,
    #[serde(rename = "sessionSlots", skip_serializing_if = "Option::is_none")]
    pub session_slots: Option<HashMap<String, SessionSlotConfig>>,
    pub workflow: Vec<PlaybookStep>,
    #[serde(rename = "successCriteria")]
    pub success_criteria: Option<SuccessCriteria>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(rename = "isBookmarked")]
    pub is_bookmarked: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WorkflowStorage {
    Steps(Vec<PlaybookStep>),
    Structured {
        steps: Vec<PlaybookStep>,
        #[serde(rename = "defaultTargetSession")]
        default_target_session: Option<TargetSessionConfig>,
        #[serde(rename = "sessionSlots")]
        session_slots: Option<HashMap<String, SessionSlotConfig>>,
    },
}

impl Playbook {
    pub fn from_model(model: &playbook::Model) -> Self {
        let (mut workflow, default_target_session, session_slots) =
            match serde_json::from_str::<WorkflowStorage>(&model.workflow) {
                Ok(WorkflowStorage::Structured {
                    steps,
                    default_target_session,
                    session_slots,
                }) => (steps, default_target_session, session_slots),
                Ok(WorkflowStorage::Steps(steps)) => (steps, None, None),
                Err(e) => {
                    log::warn!(
                        "Failed to deserialize workflow for playbook '{}' (goal: '{}'): {}",
                        model.id,
                        model.goal,
                        e
                    );
                    (Vec::new(), None, None)
                }
            };

        // Normalize slot assignment: if session_slot was stored inside target_session, promote it
        for step in &mut workflow {
            if step.session_slot.is_none() {
                if let Some(ref target) = step.target_session {
                    if let Some(ref slot) = target.session_slot {
                        step.session_slot = Some(slot.clone());
                    }
                }
            }
        }

        Self {
            id: model.id.clone(),
            assistant_id: model.assistant_id.clone(),
            goal: model.goal.clone(),
            initial_command: model.initial_command.clone(),
            default_target_session,
            session_slots,
            workflow,
            success_criteria: model
                .success_criteria
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok()),
            created_at: model.created_at,
            updated_at: model.updated_at,
            is_bookmarked: model.is_bookmarked,
        }
    }
}
