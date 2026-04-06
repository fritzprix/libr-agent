use serde_json::{json, Value};

use crate::mcp::builtin::error_guidance::{
    guided_error, missing_agent_config_error, missing_agent_session_error, ErrorCategory,
    SuccessHint, ToolGroup,
};
use crate::mcp::builtin::session_api::utils::{
    build_agent_tool_data, check_session_next_actions, read_required_string,
};
use crate::mcp::types::MCPResult;

use super::super::AgentServer;
use super::caller_session_not_found_result;
use super::check_session::check_session;

async fn start_session_impl(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
    tool_name: &str,
    force_include_current_org: bool,
) -> Result<MCPResult, String> {
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;

    let include_current_org = force_include_current_org
        || args
            .get("includeCurrentOrg")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
    let requested_workspace_override = args
        .get("workspaceOverride")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let explicit_org = if include_current_org {
        let caller_session = match manager.get_session(caller_session_id).await? {
            Some(session) => session,
            None => return Ok(caller_session_not_found_result(caller_session_id)),
        };

        match (
            caller_session.org_id.clone(),
            caller_session.org_name.clone(),
            caller_session.org_root_session_id.clone(),
        ) {
            (Some(org_id), Some(org_name), Some(org_root_session_id)) => {
                Some((org_id, org_name, org_root_session_id))
            }
            _ => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Current session does not belong to an explicit org. Call createOrg first."
                        .to_string(),
                    ToolGroup::Agent,
                )
                .with_guidance(vec![
                    "Use createOrg(name=\"...\") from the root session first.".to_string(),
                    "Then call startSession(..., includeCurrentOrg=true) for org-visible member sessions."
                        .to_string(),
                ])
                .to_mcp_result())
            }
        }
    } else {
        None
    };

    let effective_workspace_path = if let Some(workspace_override) = requested_workspace_override {
        Some(workspace_override)
    } else if include_current_org {
        let (_, _, org_root_session_id) = explicit_org
            .as_ref()
            .ok_or_else(|| "Explicit org metadata missing after org validation".to_string())?;
        Some(
            crate::session::get_session_manager()?
                .get_session_workspace_dir_by_id(org_root_session_id)
                .to_string_lossy()
                .to_string(),
        )
    } else {
        None
    };

    let body: crate::agent::types::CreateSessionRequest = serde_json::from_value(json!({
        "parentSessionId": caller_session_id,
        "assistantId": read_required_string(&args, "agentId")?,
        "request": read_required_string(&args, "task")?,
        "workspacePath": effective_workspace_path.as_deref(),
        "maxDepth": args.get("maxDepth").and_then(|v| v.as_u64()),
        "maxFanout": args.get("maxFanout").and_then(|v| v.as_u64()),
        "orgId": explicit_org.as_ref().map(|(org_id, _, _)| org_id.as_str()),
        "orgName": explicit_org.as_ref().map(|(_, org_name, _)| org_name.as_str()),
        "orgRootSessionId": explicit_org
            .as_ref()
            .map(|(_, _, org_root_session_id)| org_root_session_id.as_str()),
    }))
    .map_err(|e| format!("Invalid arguments for {}: {}", tool_name, e))?;

    let wait_for_result = args
        .get("waitForResult")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let response = match crate::services::AgentService::spawn_agent(manager, body).await {
        Ok(res) => res,
        Err(err) if err.contains("Assistant not found:") => {
            let agent_id = args
                .get("agentId")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            return Ok(missing_agent_config_error(agent_id));
        }
        Err(err) => return Err(err),
    };

    let session_id = response.id;

    if wait_for_result {
        return check_session(
            server,
            json!({ "sessionId": session_id, "wait": true }),
            caller_session_id,
        )
        .await;
    }

    let alias_note = if tool_name == "spawnOrgAgent" {
        " spawnOrgAgent is a compatibility alias for startSession(includeCurrentOrg=true)."
    } else {
        ""
    };
    let workspace_note = if let Some(workspace_path) = effective_workspace_path.as_deref() {
        format!(" Shared workspace: {}.", workspace_path)
    } else {
        String::new()
    };
    let hint = if let Some(org_name) = response.org_name.clone() {
        SuccessHint::new(
            format!(
                "Session started successfully (ID: {}, org: {}).{}{}",
                session_id, org_name, alias_note, workspace_note
            ),
            vec![format!(
                "Use checkSession(\"{}\", wait=true) to wait for the answer.",
                session_id
            )],
        )
    } else {
        SuccessHint::new(
            format!(
                "Session started successfully (ID: {}).{}{}",
                session_id, alias_note, workspace_note
            ),
            vec![format!(
                "Use checkSession(\"{}\", wait=true) to wait for the answer.",
                session_id
            )],
        )
    };
    let message = hint.message.clone();
    let mut response_data = build_agent_tool_data(
        tool_name,
        "session",
        Some(&session_id),
        &message,
        "pending",
        check_session_next_actions(&session_id),
    );
    response_data.insert("sessionId".to_string(), Value::String(session_id.clone()));
    response_data.insert("status".to_string(), Value::String("started".to_string()));
    if let Some(workspace_path) = effective_workspace_path {
        response_data.insert("workspacePath".to_string(), Value::String(workspace_path));
    }

    Ok(hint.to_mcp_result_with_data(Some(Value::Object(response_data))))
}

/// startSession handler (from spawnAgent)
pub async fn start_session(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    start_session_impl(server, args, caller_session_id, "startSession", false).await
}

pub async fn spawn_org_agent(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    start_session_impl(server, args, caller_session_id, "spawnOrgAgent", true).await
}

/// messageToSession handler (from messageAgent)
pub async fn message_to_session(
    server: &AgentServer,
    args: Value,
    _caller_session_id: &str,
) -> Result<MCPResult, String> {
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;
    let session_id = read_required_string(&args, "sessionId")?;
    let message_text = read_required_string(&args, "message")?;
    let response = match crate::services::AgentService::send_message_to_session(
        manager,
        &session_id,
        message_text,
        Some("agent_tool".to_string()),
    )
    .await
    {
        Ok(response) => response,
        Err(err) if err.contains("Session not found:") => {
            return Ok(missing_agent_session_error(&session_id));
        }
        Err(err) => return Err(err),
    };

    let hint = SuccessHint::new(
        format!("Message {} for session {}.", response.status, session_id),
        vec![format!(
            "Use checkSession(\"{}\", wait=true) to see the response.",
            session_id
        )],
    );
    let message = hint.message.clone();
    let mut response_data = build_agent_tool_data(
        "messageToSession",
        "session",
        Some(&session_id),
        &message,
        "pending",
        check_session_next_actions(&session_id),
    );
    response_data.insert("sessionId".to_string(), Value::String(session_id));
    response_data.insert("messageId".to_string(), Value::String(response.message_id));
    response_data.insert("status".to_string(), Value::String(response.status));

    Ok(hint.to_mcp_result_with_data(Some(Value::Object(response_data))))
}

/// stopSession handler (from terminateAgent)
pub async fn stop_session(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;
    let session_id = read_required_string(&args, "sessionId")?;

    if caller_session_id == session_id {
        return Ok(guided_error(
            ErrorCategory::InvalidState,
            "Self-termination is not allowed via stopSession.",
            ToolGroup::Agent,
        )
        .with_guidance(vec![
            "Use stopSession only for child or delegated sessions".to_string(),
            "If the current workflow should stop, use the normal session cancellation controls instead"
                .to_string(),
        ])
        .to_mcp_result());
    }

    if let Err(error) = manager.terminate_session(session_id.clone()).await {
        if error.contains("not found") {
            return Ok(missing_agent_session_error(&session_id));
        }
        return Err(error);
    }

    crate::services::agent_service::lineage_store()
        .write()
        .await
        .remove(&session_id);

    let hint = SuccessHint::new(format!("Session {} stopped.", session_id), vec![]);
    let message = hint.message.clone();
    let mut response_data = build_agent_tool_data(
        "stopSession",
        "session",
        Some(&session_id),
        &message,
        "success",
        vec![],
    );
    response_data.insert("sessionId".to_string(), Value::String(session_id));
    response_data.insert("stopped".to_string(), Value::Bool(true));
    response_data.insert(
        "status".to_string(),
        Value::String("terminated".to_string()),
    );

    Ok(hint.to_mcp_result_with_data(Some(Value::Object(response_data))))
}
