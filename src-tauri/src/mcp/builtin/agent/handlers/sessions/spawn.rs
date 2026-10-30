use serde_json::{json, Value};

use super::super::super::utils::{
    build_agent_tool_data, check_session_next_actions, insert_agent_session_id_fields,
    read_complexity, read_required_string, resolve_model_for_complexity,
};
use crate::mcp::builtin::error_guidance::{
    missing_agent_config_error, session_id_passed_as_agent_config_error, SuccessHint,
};
use crate::mcp::types::MCPResult;
use crate::repositories::{AssistantRepository, SessionRepository};

use super::super::super::AgentServer;
use super::super::check_session::check_session;
use super::super::enrichment::format_workspace_status_note;
use super::super::shared::caller_session_not_found_result;
use super::workspace::{
    org_child_inherited_docker_workspace, resolve_spawn_workspace_binding,
    resolve_spawned_workspace_signal,
};

async fn spawn_session_impl(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
    tool_name: &str,
) -> Result<MCPResult, String> {
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;

    let caller_session = match manager.get_session(caller_session_id).await? {
        Some(session) => session,
        None => return Ok(caller_session_not_found_result(caller_session_id)),
    };
    let caller_explicit_org = match (
        caller_session.org_id.clone(),
        caller_session.org_name.clone(),
        caller_session.org_root_session_id.clone(),
    ) {
        (Some(org_id), Some(org_name), Some(org_root_session_id)) => {
            Some((org_id, org_name, org_root_session_id))
        }
        _ => None,
    };

    let explicit_org = caller_explicit_org.clone();
    let assistant_id = match read_required_string(&args, "configId") {
        Ok(value) => value,
        Err(result) => return Ok(result),
    };
    let task = match read_required_string(&args, "task") {
        Ok(value) => value,
        Err(result) => return Ok(result),
    };
    let complexity = match read_complexity(&args) {
        Ok(level) => level,
        Err(result) => return Ok(result),
    };
    let resolved_model = resolve_model_for_complexity(complexity).await?;
    let requested_workspace_override = args
        .get("workspaceOverride")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let workspace_override_set = requested_workspace_override.is_some();

    let session_manager = crate::session::get_session_manager()?;
    let caller_workspace = session_manager.get_session_workspace_dir_by_id(caller_session_id);
    let org_root_workspace = explicit_org.as_ref().map(|(_, _, org_root_session_id)| {
        session_manager.get_session_workspace_dir_by_id(org_root_session_id)
    });
    let effective_workspace_path = resolve_spawn_workspace_binding(
        requested_workspace_override.as_deref(),
        &caller_workspace,
        org_root_workspace.as_deref(),
    )
    .map(|path| path.to_string_lossy().into_owned());

    // When complexity mapping has no override, omit model/provider so the
    // existing parent-inheritance → preferredModel chain stays zero-breaking.
    let mut body_json = json!({
        "parentSessionId": caller_session_id,
        "assistantId": assistant_id.clone(),
        "request": task.clone(),
        "workspacePath": effective_workspace_path.as_deref(),
        "orgId": explicit_org.as_ref().map(|(org_id, _, _)| org_id.as_str()),
        "orgName": explicit_org.as_ref().map(|(_, org_name, _)| org_name.as_str()),
        "orgRootSessionId": explicit_org
            .as_ref()
            .map(|(_, _, org_root_session_id)| org_root_session_id.as_str()),
    });
    if let Some(resolved) = resolved_model.as_ref() {
        body_json["model"] = Value::String(resolved.model.clone());
        body_json["provider"] = Value::String(resolved.provider.clone());
    }

    // Org children share the org-root host workspace; also inherit Docker attach
    // so Harbor attach trials write into the same container FS as the root.
    if let Some((_, _, org_root_session_id)) = explicit_org.as_ref() {
        let org_root_session = if caller_session.id == *org_root_session_id {
            Some(caller_session.clone())
        } else {
            manager.get_session(org_root_session_id).await?
        };
        if let Some((mode, docker_config)) = org_root_session
            .as_ref()
            .and_then(org_child_inherited_docker_workspace)
        {
            body_json["workspaceIsolation"] = serde_json::to_value(mode)
                .map_err(|e| format!("Failed to serialize inherited workspaceIsolation: {e}"))?;
            body_json["dockerConfig"] = serde_json::to_value(docker_config)
                .map_err(|e| format!("Failed to serialize inherited dockerConfig: {e}"))?;
        }
    }

    let body: crate::agent::types::CreateSessionRequest = serde_json::from_value(body_json)
        .map_err(|e| format!("Invalid arguments for {}: {}", tool_name, e))?;

    let wait_for_result = args
        .get("waitForResult")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let response = match crate::services::AgentService::spawn_agent(manager, body).await {
        Ok(res) => res,
        Err(err) if err.contains("Assistant not found:") => {
            let requested_id = &assistant_id;

            let is_session_id = caller_session_id == requested_id.as_str()
                || matches!(
                    crate::state::get_session_repository()
                        .get_session(requested_id)
                        .await,
                    Ok(Some(_))
                );

            if is_session_id {
                return Ok(session_id_passed_as_agent_config_error(requested_id));
            }

            return Ok(missing_agent_config_error(requested_id));
        }
        Err(err) => return Err(err),
    };

    let session_id = response.id;
    let display_id = crate::utils::session_id::display_session_id(&session_id);

    if wait_for_result {
        let mut check_args = json!({ "sessionId": session_id, "wait": true });
        if let Some(timeout) = args.get("timeout") {
            check_args["timeout"] = timeout.clone();
        }
        return check_session(server, check_args, caller_session_id).await;
    }

    let (workspace_display, workspace_relation) =
        resolve_spawned_workspace_signal(caller_session_id, &session_id, effective_workspace_path);
    let workspace_note = match (workspace_display.as_deref(), workspace_relation) {
        (Some(path), Some(relation)) => format_workspace_status_note(path, relation),
        _ => String::new(),
    };
    let hint = if let Some(org_name) = response.org_name.clone() {
        SuccessHint::new(
            format!(
                "Session started successfully (ID: {}, org: {}).{}",
                display_id, org_name, workspace_note
            ),
            vec![format!(
                "Use agent__checkSession(\"{}\", wait=true) to wait for the answer.",
                display_id
            )],
        )
    } else {
        SuccessHint::new(
            format!(
                "Session started successfully (ID: {}).{}",
                display_id, workspace_note
            ),
            vec![format!(
                "Use agent__checkSession(\"{}\", wait=true) to wait for the answer.",
                display_id
            )],
        )
    };
    let message = hint.message.clone();
    let mut response_data = build_agent_tool_data(
        tool_name,
        "session",
        Some(&display_id),
        &message,
        "pending",
        check_session_next_actions(&session_id),
    );
    insert_agent_session_id_fields(&mut response_data, &session_id);
    response_data.insert("status".to_string(), Value::String("started".to_string()));
    response_data.insert(
        "assistantId".to_string(),
        Value::String(assistant_id.clone()),
    );
    response_data.insert("task".to_string(), Value::String(task.clone()));
    response_data.insert(
        "complexity".to_string(),
        Value::String(complexity.to_string()),
    );
    if let Some(resolved) = resolved_model.as_ref() {
        response_data.insert("model".to_string(), Value::String(resolved.model.clone()));
        response_data.insert(
            "provider".to_string(),
            Value::String(resolved.provider.clone()),
        );
    }
    response_data.insert(
        "workspaceOverride".to_string(),
        Value::Bool(workspace_override_set),
    );
    if let Some(workspace_path) = workspace_display {
        response_data.insert("workspacePath".to_string(), Value::String(workspace_path));
    }
    if let Some(relation) = workspace_relation {
        response_data.insert(
            "workspaceRelation".to_string(),
            Value::String(relation.as_str().to_string()),
        );
    }

    // Human card identity: prefer assistant display name over session id.
    {
        match crate::state::get_assistant_repository()
            .get_assistant(&assistant_id)
            .await
        {
            Ok(Some(assistant)) => {
                response_data.insert("assistantName".to_string(), Value::String(assistant.name));
            }
            Ok(None) => {}
            Err(error) => {
                log::warn!(
                    "agent__spawnSession: failed to resolve assistantName for {}: {}",
                    assistant_id,
                    error
                );
            }
        }
    }

    Ok(hint.to_mcp_result_with_data(Some(Value::Object(response_data))))
}

/// spawnSession handler.
pub async fn spawn_session(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    spawn_session_impl(server, args, caller_session_id, "spawnSession").await
}
