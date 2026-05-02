use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, SuccessHint, ToolGroup};
use crate::mcp::builtin::session_api::utils::{build_agent_tool_data, read_required_string};
use crate::mcp::types::MCPResult;
use crate::repositories::{session_repository::SessionRepository, SessionMetadata};

use super::super::AgentServer;
use super::{
    caller_session_not_found_result, invalid_explicit_org_result, missing_explicit_org_result,
    read_optional_string,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateOrgPreflight {
    ExistingOrg {
        org_id: String,
        org_name: String,
        root_session_id: String,
    },
    RequiresDedicatedWorkspace {
        effective_workspace: PathBuf,
        dedicated_workspace: PathBuf,
    },
    Proceed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TeamworkScaffoldStatus {
    pub workspace_path: String,
    pub missing_files: Vec<String>,
    pub manifest_present: bool,
    pub manifest_parse_error: Option<String>,
    pub execution_substrate_mode: Option<String>,
    pub org_lineage_intended: Option<bool>,
    pub recommended_skill: Option<String>,
}

impl TeamworkScaffoldStatus {
    pub fn is_ready_for_explicit_org(&self) -> bool {
        self.missing_files.is_empty()
            && self.manifest_parse_error.is_none()
            && self.execution_substrate_mode.as_deref() == Some("org")
            && self.org_lineage_intended == Some(true)
    }

    fn guidance_lines(&self) -> Vec<String> {
        let mut guidance = Vec::new();

        if !self.missing_files.is_empty() {
            guidance.push(format!(
                "Missing teamwork scaffold files: {}.",
                self.missing_files.join(", ")
            ));
        }

        if let Some(error) = self.manifest_parse_error.as_deref() {
            guidance.push(format!(
                ".libragent/teamwork.json exists but could not be parsed: {}.",
                error
            ));
        } else if !self.manifest_present {
            guidance.push(
                ".libragent/teamwork.json is missing, so this org has no machine-readable teamwork manifest yet."
                    .to_string(),
            );
        } else {
            if self.execution_substrate_mode.as_deref() != Some("org") {
                let mode = self
                    .execution_substrate_mode
                    .as_deref()
                    .unwrap_or("missing");
                guidance.push(format!(
                    ".libragent/teamwork.json should declare executionSubstrate.mode=\"org\" (current: {}).",
                    mode
                ));
            }

            if self.org_lineage_intended != Some(true) {
                let intended = self
                    .org_lineage_intended
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "missing".to_string());
                guidance.push(format!(
                    ".libragent/teamwork.json should declare executionSubstrate.orgLineage.intended=true (current: {}).",
                    intended
                ));
            }
        }

        guidance
    }
}

pub fn inspect_teamwork_scaffold(workspace_path: &Path) -> TeamworkScaffoldStatus {
    const REQUIRED_FILES: &[&str] = &[
        "agents.md",
        "MISSION.md",
        "ROLES.md",
        "coordination/KANBAN.md",
        "coordination/HANDOFF.md",
    ];

    let missing_files = REQUIRED_FILES
        .iter()
        .filter_map(|relative| {
            let candidate = workspace_path.join(relative);
            if candidate.exists() {
                None
            } else {
                Some((*relative).to_string())
            }
        })
        .collect::<Vec<_>>();

    let manifest_path = workspace_path.join(".libragent").join("teamwork.json");
    let manifest_present = manifest_path.exists();
    let mut manifest_parse_error = None;
    let mut execution_substrate_mode = None;
    let mut org_lineage_intended = None;

    if manifest_present {
        match std::fs::read_to_string(&manifest_path) {
            Ok(raw) => match serde_json::from_str::<Value>(&raw) {
                Ok(manifest) => {
                    execution_substrate_mode = manifest
                        .get("executionSubstrate")
                        .and_then(|value| value.get("mode"))
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    org_lineage_intended = manifest
                        .get("executionSubstrate")
                        .and_then(|value| value.get("orgLineage"))
                        .and_then(|value| value.get("intended"))
                        .and_then(Value::as_bool);
                }
                Err(error) => manifest_parse_error = Some(error.to_string()),
            },
            Err(error) => manifest_parse_error = Some(error.to_string()),
        }
    }

    let mut status = TeamworkScaffoldStatus {
        workspace_path: workspace_path.display().to_string(),
        missing_files,
        manifest_present,
        manifest_parse_error,
        execution_substrate_mode,
        org_lineage_intended,
        recommended_skill: None,
    };

    if !status.is_ready_for_explicit_org() {
        status.recommended_skill = Some("teamwork".to_string());
    }

    status
}

fn teamwork_scaffold_status_for_session(
    session_id: &str,
) -> Result<TeamworkScaffoldStatus, String> {
    let workspace_path: PathBuf =
        crate::session::get_session_manager()?.get_session_workspace_dir_by_id(session_id);
    Ok(inspect_teamwork_scaffold(&workspace_path))
}

fn create_org_next_actions(org_id: &str, include_builder_guidance: bool) -> Vec<Value> {
    let mut next_actions = vec![
        json!({
            "toolName": "startSession",
            "reason": "Create or add an explicit org member session. Org inheritance is automatic here.",
        }),
        json!({
            "toolName": "getOrg",
            "reason": "Inspect the explicit org summary.",
            "args": { "orgId": org_id }
        }),
    ];

    if include_builder_guidance {
        next_actions.push(json!({
            "actionType": "skill",
            "toolName": "teamwork",
            "reason": "Scaffold or repair the teamwork workspace constitution for this org."
        }));
    }

    next_actions
}

fn create_org_hint_lines(scaffold: &TeamworkScaffoldStatus) -> Vec<String> {
    let mut lines = Vec::new();

    if !scaffold.is_ready_for_explicit_org() {
        lines.extend(scaffold.guidance_lines());
        lines.push(
            "Use the teamwork skill next to scaffold or repair the teamwork workspace constitution in this workspace."
                .to_string(),
        );
    }

    lines
}

fn existing_org_hint_lines(
    scaffold: &TeamworkScaffoldStatus,
    workspace_status: &crate::session::TeamworkWorkspaceStatus,
) -> Vec<String> {
    let mut lines = create_org_hint_lines(scaffold);

    if !workspace_status.uses_dedicated_teamwork_workspace() {
        lines.push(format!(
            "Current effective workspace is still {}.",
            workspace_status.effective_workspace.display()
        ));
        lines.push(format!(
            "This org should be resumed from the dedicated teamwork workspace at {}.",
            workspace_status.dedicated_workspace.display()
        ));
        lines.push(
            "Call prepareTeamworkWorkspace() from the org root session to migrate the active workspace before continuing teamwork operations."
                .to_string(),
        );
    }

    lines
}

pub fn existing_explicit_org_identity(
    session: &SessionMetadata,
) -> Option<(String, String, String)> {
    match (
        session.org_id.clone(),
        session.org_name.clone(),
        session.org_root_session_id.clone(),
    ) {
        (Some(org_id), Some(org_name), Some(root_session_id)) => {
            Some((org_id, org_name, root_session_id))
        }
        _ => None,
    }
}

pub fn create_org_preflight(
    session: &SessionMetadata,
    workspace_status: &crate::session::TeamworkWorkspaceStatus,
) -> CreateOrgPreflight {
    if let Some((org_id, org_name, root_session_id)) = existing_explicit_org_identity(session) {
        return CreateOrgPreflight::ExistingOrg {
            org_id,
            org_name,
            root_session_id,
        };
    }

    if !workspace_status.uses_dedicated_teamwork_workspace() {
        return CreateOrgPreflight::RequiresDedicatedWorkspace {
            effective_workspace: workspace_status.effective_workspace.clone(),
            dedicated_workspace: workspace_status.dedicated_workspace.clone(),
        };
    }

    CreateOrgPreflight::Proceed
}

pub async fn create_org(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;
    let session = match manager.get_session(caller_session_id).await? {
        Some(session) => session,
        None => return Ok(caller_session_not_found_result(caller_session_id)),
    };

    if session.parent_session_id.is_some() {
        return Ok(guided_error(
            ErrorCategory::InvalidInput,
            "createOrg must be called from a top-level root session, not a delegated child"
                .to_string(),
            ToolGroup::Agent,
        )
        .with_guidance(vec![
            "Resume the top-level/root session first.".to_string(),
            "Then call createOrg(name=\"...\") from that root session.".to_string(),
        ])
        .to_mcp_result());
    }

    let workspace_status = crate::session::teamwork_workspace_status(
        crate::session::get_session_manager()?,
        caller_session_id,
    );
    match create_org_preflight(&session, &workspace_status) {
        CreateOrgPreflight::ExistingOrg {
            org_id: existing_org_id,
            org_name: existing_org_name,
            root_session_id: existing_root_id,
        } => {
            let scaffold = teamwork_scaffold_status_for_session(caller_session_id)?;
            let message = format!(
                "Current session already owns explicit org '{}' (ID: {}, root session: {}).",
                existing_org_name, existing_org_id, existing_root_id
            );
            let mut response_data = build_agent_tool_data(
                "createOrg",
                "org",
                Some(&existing_org_id),
                &message,
                "success",
                create_org_next_actions(&existing_org_id, !scaffold.is_ready_for_explicit_org()),
            );
            response_data.insert("orgId".to_string(), Value::String(existing_org_id));
            response_data.insert("orgName".to_string(), Value::String(existing_org_name));
            response_data.insert(
                "orgRootSessionId".to_string(),
                Value::String(existing_root_id),
            );
            response_data.insert(
                "teamworkScaffold".to_string(),
                serde_json::to_value(&scaffold).unwrap_or(Value::Null),
            );
            response_data.insert(
                "workspaceMismatch".to_string(),
                Value::Bool(!workspace_status.uses_dedicated_teamwork_workspace()),
            );
            response_data.insert(
                "effectiveWorkspace".to_string(),
                Value::String(workspace_status.effective_workspace.display().to_string()),
            );
            response_data.insert(
                "dedicatedTeamworkWorkspace".to_string(),
                Value::String(workspace_status.dedicated_workspace.display().to_string()),
            );
            return Ok(SuccessHint::new(
                message,
                existing_org_hint_lines(&scaffold, &workspace_status),
            )
            .to_mcp_result_with_data(Some(Value::Object(response_data))));
        }
        CreateOrgPreflight::RequiresDedicatedWorkspace {
            effective_workspace,
            dedicated_workspace,
        } => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "createOrg requires the governing/root session to already be using its dedicated teamwork workspace."
                    .to_string(),
                ToolGroup::Agent,
            )
            .with_guidance(vec![
                format!("Current effective workspace: {}", effective_workspace.display()),
                format!(
                    "Dedicated teamwork workspace required: {}",
                    dedicated_workspace.display()
                ),
                "Call prepareTeamworkWorkspace() from this root session first.".to_string(),
                "Then create or repair the teamwork scaffold in that workspace and retry createOrg(name=\"...\")."
                    .to_string(),
            ])
            .to_mcp_result());
        }
        CreateOrgPreflight::Proceed => {}
    }

    let org_name = read_required_string(&args, "name")?;
    let org_id = format!("org-{}", uuid::Uuid::new_v4().simple());
    let org_root_session_id = session.id.clone();
    let session_repo = crate::state::get_session_repository();
    session_repo
        .update_org_identity(
            caller_session_id,
            Some(org_id.clone()),
            Some(org_name.clone()),
            Some(org_root_session_id.clone()),
        )
        .await
        .map_err(|error| format!("Failed to persist org identity: {}", error))?;

    let scaffold = teamwork_scaffold_status_for_session(caller_session_id)?;

    let message = format!(
        "Explicit org created.\n\nOrg: {} (ID: {})\nRoot session: {}\n\nChild sessions started from this org root now join Org view automatically. Use includeCurrentOrg=false only when you intentionally want a one-off child to stay out of Org view.",
        org_name, org_id, org_root_session_id
    );
    let mut response_data = build_agent_tool_data(
        "createOrg",
        "org",
        Some(&org_id),
        &message,
        "success",
        create_org_next_actions(&org_id, !scaffold.is_ready_for_explicit_org()),
    );
    response_data.insert("orgId".to_string(), Value::String(org_id));
    response_data.insert("orgName".to_string(), Value::String(org_name));
    response_data.insert(
        "orgRootSessionId".to_string(),
        Value::String(org_root_session_id),
    );
    response_data.insert(
        "teamworkScaffold".to_string(),
        serde_json::to_value(&scaffold).unwrap_or(Value::Null),
    );

    Ok(SuccessHint::new(message, create_org_hint_lines(&scaffold))
        .to_mcp_result_with_data(Some(Value::Object(response_data))))
}

pub async fn get_org(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;
    let caller_session = match manager.get_session(caller_session_id).await? {
        Some(session) => session,
        None => return Ok(caller_session_not_found_result(caller_session_id)),
    };
    let requested_org_id = read_optional_string(&args, "orgId")?;
    let target_org_id = match requested_org_id.or_else(|| caller_session.org_id.clone()) {
        Some(org_id) => org_id,
        None => return Ok(missing_explicit_org_result()),
    };

    let sessions = manager.get_all_sessions().await?;
    let mut members: Vec<_> = sessions
        .into_iter()
        .filter(|session| session.org_id.as_deref() == Some(target_org_id.as_str()))
        .collect();

    if members.is_empty() {
        return Ok(guided_error(
            ErrorCategory::ResourceNotFound,
            format!("No sessions found for explicit org '{}'.", target_org_id),
            ToolGroup::Agent,
        )
        .to_mcp_result());
    }

    members.sort_by(|left, right| {
        let left_depth = left.depth.unwrap_or(0);
        let right_depth = right.depth.unwrap_or(0);
        left_depth
            .cmp(&right_depth)
            .then_with(|| left.created_at.cmp(&right.created_at))
    });

    let root_session = match members.iter().find(|session| {
        session
            .org_root_session_id
            .as_deref()
            .is_some_and(|root_id| root_id == session.id)
    }) {
        Some(root_session) => root_session,
        None => return Ok(invalid_explicit_org_result(&target_org_id)),
    };

    let org_name = root_session
        .org_name
        .clone()
        .unwrap_or_else(|| target_org_id.clone());
    let member_lines = members
        .iter()
        .map(|session| {
            format!(
                "- {} [{}] depth={} session={}",
                session.name.clone().unwrap_or_else(|| session.id.clone()),
                session.status.as_str(),
                session.depth.unwrap_or(0),
                session.id
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let busy_count = members
        .iter()
        .filter(|session| session.status == crate::repositories::SessionStatus::Busy)
        .count();
    let message = format!(
        "Explicit org summary\n\nOrg: {} (ID: {})\nRoot session: {}\nMembers: {} (busy: {})\n\n{}",
        org_name,
        target_org_id,
        root_session.id,
        members.len(),
        busy_count,
        member_lines
    );
    let mut response_data = build_agent_tool_data(
        "getOrg",
        "org",
        Some(&target_org_id),
        &message,
        "success",
        vec![json!({
            "toolName": "startSession",
            "reason": "Add another explicit org member under this org.",
        })],
    );
    response_data.insert("orgId".to_string(), Value::String(target_org_id));
    response_data.insert("orgName".to_string(), Value::String(org_name));
    response_data.insert(
        "orgRootSessionId".to_string(),
        Value::String(root_session.id.clone()),
    );
    response_data.insert("memberCount".to_string(), json!(members.len()));
    response_data.insert("busyCount".to_string(), json!(busy_count));

    Ok(SuccessHint::new(message, vec![])
        .to_mcp_result_with_data(Some(Value::Object(response_data))))
}
