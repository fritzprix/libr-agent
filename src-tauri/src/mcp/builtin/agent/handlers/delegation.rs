use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

use super::super::utils::{build_agent_tool_data, insert_agent_session_id_fields};
use super::super::AgentServer;
use super::shared::caller_session_not_found_result;
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, SuccessHint, ToolGroup};
use crate::mcp::types::MCPResult;
use crate::repositories::SessionMetadata;

pub fn is_delegated_descendant_session(
    sessions: &HashMap<String, SessionMetadata>,
    caller_session_id: &str,
    target_session_id: &str,
) -> bool {
    if caller_session_id == target_session_id {
        return false;
    }

    let mut next_parent = sessions
        .get(target_session_id)
        .and_then(|session| session.parent_session_id.clone());
    let mut seen = HashSet::new();

    while let Some(parent_id) = next_parent {
        if !seen.insert(parent_id.clone()) {
            return false;
        }
        if parent_id == caller_session_id {
            return true;
        }
        next_parent = sessions
            .get(&parent_id)
            .and_then(|session| session.parent_session_id.clone());
    }

    false
}

pub async fn load_accessible_delegated_session(
    manager: &crate::agent::AgentSessionManager,
    caller_session_id: &str,
    target_session_id: &str,
    tool_name: &str,
) -> Result<SessionMetadata, MCPResult> {
    let caller_exists = match manager.get_session(caller_session_id).await {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(error) => {
            return Err(guided_error(
                ErrorCategory::InternalError,
                format!(
                    "Failed to load caller session metadata for {}: {}",
                    tool_name, error
                ),
                ToolGroup::Agent,
            )
            .with_guidance(vec![
                "Retry the operation once session metadata is available again".to_string(),
                "If the issue persists, inspect the session repository health".to_string(),
            ])
            .to_mcp_result())
        }
    };
    if !caller_exists {
        return Err(caller_session_not_found_result(caller_session_id));
    }

    let resolved_target_id = match resolve_delegated_session_ref(
        manager,
        caller_session_id,
        target_session_id,
        tool_name,
    )
    .await
    {
        Ok(id) => id,
        Err(result) => return Err(result),
    };

    let Some(target_session) = (match manager.get_session(&resolved_target_id).await {
        Ok(session) => session,
        Err(error) => {
            return Err(guided_error(
                ErrorCategory::InternalError,
                format!(
                    "Failed to load session metadata for {}: {}",
                    tool_name, error
                ),
                ToolGroup::Agent,
            )
            .with_guidance(vec![
                "Retry the operation once session metadata is available again".to_string(),
                "If the issue persists, inspect the session repository health".to_string(),
            ])
            .to_mcp_result())
        }
    }) else {
        return Err(
            crate::mcp::builtin::error_guidance::missing_agent_session_error(target_session_id),
        );
    };

    let mut next_parent = target_session.parent_session_id.clone();
    let mut seen = HashSet::new();
    while let Some(parent_id) = next_parent {
        if !seen.insert(parent_id.clone()) {
            break;
        }
        if parent_id == caller_session_id {
            return Ok(target_session);
        }
        next_parent = match manager.get_session(&parent_id).await {
            Ok(Some(session)) => session.parent_session_id,
            Ok(None) => None,
            Err(error) => {
                return Err(guided_error(
                    ErrorCategory::InternalError,
                    format!(
                        "Failed to load delegated session lineage for {}: {}",
                        tool_name, error
                    ),
                    ToolGroup::Agent,
                )
                .with_guidance(vec![
                    "Retry the operation once session metadata is available again".to_string(),
                    "If the issue persists, inspect the session repository health".to_string(),
                ])
                .to_mcp_result())
            }
        };
    }

    Err(guided_error(
        ErrorCategory::PermissionDenied,
        format!(
            "Session '{}' is not a delegated descendant of the current session '{}'.",
            target_session_id, caller_session_id
        ),
        ToolGroup::Agent,
    )
    .with_guidance(vec![
        format!(
            "Use {} only with delegated child/descendant sessions started from the current session",
            tool_name
        ),
        "Use agent__listAgents(type=\"sessions\") to inspect the delegated sessions you can control directly"
            .to_string(),
        "Start a new delegated session with agent__startSession(...) if you need fresh child work"
            .to_string(),
    ])
    .to_mcp_result())
}

/// Resolve a full session id, bare short token, or optional `session-{short}` form.
///
/// Exact DB hits win. Otherwise alias resolution is scoped to delegated descendants of
/// `caller_session_id` so short refs stay unambiguous within the caller's tree.
async fn resolve_delegated_session_ref(
    manager: &crate::agent::AgentSessionManager,
    caller_session_id: &str,
    target_ref: &str,
    tool_name: &str,
) -> Result<String, MCPResult> {
    match manager.get_session(target_ref).await {
        Ok(Some(_)) => return Ok(target_ref.to_string()),
        Ok(None) => {}
        Err(error) => {
            return Err(guided_error(
                ErrorCategory::InternalError,
                format!(
                    "Failed to resolve session reference for {}: {}",
                    tool_name, error
                ),
                ToolGroup::Agent,
            )
            .with_guidance(vec![
                "Retry the operation once session metadata is available again".to_string(),
                "If the issue persists, inspect the session repository health".to_string(),
            ])
            .to_mcp_result())
        }
    }

    let all_sessions = match manager.get_all_sessions().await {
        Ok(sessions) => sessions,
        Err(error) => {
            return Err(guided_error(
                ErrorCategory::InternalError,
                format!(
                    "Failed to list sessions while resolving reference for {}: {}",
                    tool_name, error
                ),
                ToolGroup::Agent,
            )
            .with_guidance(vec![
                "Retry the operation once session metadata is available again".to_string(),
                "If the issue persists, inspect the session repository health".to_string(),
            ])
            .to_mcp_result())
        }
    };

    let sessions_by_id: HashMap<String, SessionMetadata> = all_sessions
        .iter()
        .map(|session| (session.id.clone(), session.clone()))
        .collect();

    let descendant_ids: Vec<&str> = all_sessions
        .iter()
        .filter(|session| {
            is_delegated_descendant_session(&sessions_by_id, caller_session_id, &session.id)
        })
        .map(|session| session.id.as_str())
        .collect();

    match crate::utils::session_id::resolve_session_id_among(
        descendant_ids.iter().copied(),
        target_ref,
    ) {
        crate::utils::session_id::SessionIdResolve::Unique(resolved) => Ok(resolved.to_string()),
        crate::utils::session_id::SessionIdResolve::Missing => {
            Err(crate::mcp::builtin::error_guidance::missing_agent_session_error(target_ref))
        }
        crate::utils::session_id::SessionIdResolve::Ambiguous(count) => {
            let caller_alias = crate::utils::session_id::display_session_id(caller_session_id);
            Err(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Session reference '{}' is ambiguous among {} delegated descendants of '{}'.",
                    target_ref, count, caller_alias
                ),
                ToolGroup::Agent,
            )
            .with_guidance(vec![
                "Multiple delegated sessions share this short alias; confirm the target by name via list(type=\"sessions\")".to_string(),
                format!(
                    "Retry {} with an exact storage id if available, or remove unused sibling sessions so aliases are unique",
                    tool_name
                ),
            ])
            .to_mcp_result())
        }
    }
}

pub async fn prepare_teamwork_workspace(
    server: &AgentServer,
    _args: Value,
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
            "agent__prepareTeamworkWorkspace must be called from a top-level governing/root session."
                .to_string(),
            ToolGroup::Agent,
        )
        .with_guidance(vec![
            "Resume the governing/root session first.".to_string(),
            "Then call agent__prepareTeamworkWorkspace() before creating teamwork scaffold artifacts."
                .to_string(),
        ])
        .to_mcp_result());
    }

    let artifact_path =
        crate::services::WorkspaceService::provision_teamwork_workspace(caller_session_id).await?;
    let message = format!(
        "Teamwork artifact directory is ready for session {}. Do not call agent__prepareTeamworkWorkspace again — the empty @teamwork/ root already exists.",
        caller_session_id
    );
    let hint = SuccessHint::new(
        message.clone(),
        vec![
            "Recommended follow-up: scaffold the full org teamwork set. Prefer the teamwork skill + scripts/init_task_force.py with --output set to this response's artifactPath field, or write under @teamwork/ (agents.md, MISSION.md, ROLES.md, coordination/*, and @teamwork/.libragent/teamwork.json with executionSubstrate.mode=\"org\" and orgLineage.intended=true)."
                .to_string(),
            "After that scaffold is complete, agent__createOrg(name=\"...\") from this root session, then agent__startSession for org members so they inherit the shared workspace. Spawning children before agent__createOrg leaves each spoke in an isolated workspace."
                .to_string(),
        ],
    );

    let mut response_data = build_agent_tool_data(
        "prepareTeamworkWorkspace",
        "workspace",
        Some(&crate::utils::session_id::display_session_id(
            caller_session_id,
        )),
        &message,
        "success",
        vec![
            json!({
                "actionType": "skill",
                "toolName": "teamwork",
                "reason": "Scaffold the full @teamwork/ artifact set (prefer init_task_force.py) before agent__createOrg."
            }),
            json!({
                "toolName": "agent__createOrg",
                "reason": "Create explicit org identity only after @teamwork/ scaffold + teamwork.json are ready."
            }),
        ],
    );
    insert_agent_session_id_fields(&mut response_data, caller_session_id);
    response_data.insert("artifactPath".to_string(), Value::String(artifact_path));
    response_data.insert("mode".to_string(), Value::String("teamwork".to_string()));

    Ok(hint.to_mcp_result_with_data(Some(Value::Object(response_data))))
}
