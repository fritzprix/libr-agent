use serde_json::{json, Value};
use std::time::Duration;

use crate::mcp::builtin::error_guidance::{
    guided_error, missing_agent_config_error, missing_agent_session_error, ErrorCategory,
    SuccessHint, ToolGroup,
};
use crate::mcp::builtin::session_api::utils::{
    build_agent_tool_data, check_session_next_actions, read_required_string,
};
use crate::mcp::types::MCPResult;
use crate::models::chat::MessageSource;
use crate::repositories::SessionStatus;

use super::super::AgentServer;
use super::check_session::check_session;
use super::{caller_session_not_found_result, load_accessible_delegated_session};

fn self_target_session_action_result(
    tool_name: &str,
    message: &str,
    guidance: Vec<String>,
) -> MCPResult {
    guided_error(
        ErrorCategory::InvalidState,
        message.to_string(),
        ToolGroup::Agent,
    )
    .with_guidance(
        std::iter::once(format!(
            "Use {} only for child or delegated sessions",
            tool_name
        ))
        .chain(guidance)
        .collect(),
    )
    .to_mcp_result()
}

async fn start_session_impl(
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

    let include_current_org = args
        .get("includeCurrentOrg")
        .and_then(|v| v.as_bool())
        .unwrap_or(caller_explicit_org.is_some());
    let requested_workspace_override = args
        .get("workspaceOverride")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let explicit_org = if include_current_org {
        match caller_explicit_org {
            Some(org) => Some(org),
            None => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Current session does not belong to an explicit org. Call createOrg first."
                        .to_string(),
                    ToolGroup::Agent,
                )
                .with_guidance(vec![
                    "Use createOrg(name=\"...\") from the root session first.".to_string(),
                    "Then call startSession(...) to create org-visible member sessions."
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
        "model": args.get("model").and_then(|v| v.as_str()),
        "provider": args.get("provider").and_then(|v| v.as_str()),
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

    let workspace_note = if let Some(workspace_path) = effective_workspace_path.as_deref() {
        format!(" Shared workspace: {}.", workspace_path)
    } else {
        String::new()
    };
    let hint = if let Some(org_name) = response.org_name.clone() {
        SuccessHint::new(
            format!(
                "Session started successfully (ID: {}, org: {}).{}",
                session_id, org_name, workspace_note
            ),
            vec![format!(
                "Use checkSession(\"{}\", wait=true) to wait for the answer.",
                session_id
            )],
        )
    } else {
        SuccessHint::new(
            format!(
                "Session started successfully (ID: {}).{}",
                session_id, workspace_note
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
    start_session_impl(server, args, caller_session_id, "startSession").await
}

/// messageToSession handler (from messageAgent)
pub async fn message_to_session(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;
    let session_id = read_required_string(&args, "sessionId")?;
    let message_text = read_required_string(&args, "message")?;
    if let Err(result) = load_accessible_delegated_session(
        manager,
        caller_session_id,
        &session_id,
        "messageToSession",
    )
    .await
    {
        return Ok(result);
    }
    let response = match crate::services::AgentService::send_message_to_session(
        manager,
        &session_id,
        message_text,
        Some(MessageSource::AgentTool),
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
        return Ok(self_target_session_action_result(
            "stopSession",
            "Self-termination is not allowed via stopSession.",
            vec![
            "If the current workflow should stop, use the normal session cancellation controls instead"
                .to_string(),
            ],
        ));
    }

    if let Err(result) =
        load_accessible_delegated_session(manager, caller_session_id, &session_id, "stopSession")
            .await
    {
        return Ok(result);
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

pub async fn compact_session_context(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;
    let session_id = read_required_string(&args, "sessionId")?;
    let timeout_seconds = args
        .get("timeout")
        .and_then(|value| value.as_u64())
        .map(|value| value.clamp(5, 300))
        .unwrap_or(60);

    if caller_session_id == session_id {
        return Ok(self_target_session_action_result(
            "compactSessionContext",
                "Self-compaction is not allowed via compactSessionContext.",
                vec![
                    "Current-session compaction remains backend-managed to avoid recursive compaction loops"
                        .to_string(),
                "Use this tool only when you want to refresh another delegated session's compact summary"
                    .to_string(),
                ],
        ));
    }

    let target_session = match load_accessible_delegated_session(
        manager,
        caller_session_id,
        &session_id,
        "compactSessionContext",
    )
    .await
    {
        Ok(session) => session,
        Err(result) => return Ok(result),
    };

    if target_session.status == SessionStatus::Busy {
        return Ok(guided_error(
            ErrorCategory::InvalidState,
            format!(
                "Session {} is busy. compactSessionContext only supports idle, paused, or error sessions.",
                session_id
            ),
            ToolGroup::Agent,
        )
        .with_guidance(vec![
            format!(
                "Wait for session {} to stop running before compacting it manually",
                session_id
            ),
            format!(
                "Use checkSession(\"{}\", wait=true) if you need to block for the current run",
                session_id
            ),
        ])
        .to_mcp_result());
    }

    let previous_record = manager.get_compact_context(&session_id).await?;
    let is_active = {
        let active_sessions = manager.active_sessions_arc();
        let active = active_sessions.read().await;
        active.contains_key(&session_id)
    };

    if !is_active {
        log::info!(
            "Auto-resuming inactive session before compactSessionContext: {}",
            session_id
        );
        manager.resume_session(&session_id).await?;
        manager.init_session_with_messages(&session_id).await?;
    }

    let triggered = match manager.trigger_manual_compaction(&session_id).await {
        Ok(triggered) => triggered,
        Err(error) => return Err(error),
    };

    if !triggered {
        let message = if let Some(record) = previous_record {
            format!(
                "No new compaction was needed for session {}. Existing compact summary already covers {} -> {}.",
                session_id, record.from_id, record.to_id
            )
        } else {
            format!(
                "No compaction was needed for session {}. There is not enough uncompacted history yet.",
                session_id
            )
        };
        let mut response_data = build_agent_tool_data(
            "compactSessionContext",
            "session",
            Some(&session_id),
            &message,
            "noop",
            check_session_next_actions(&session_id),
        );
        response_data.insert("sessionId".to_string(), Value::String(session_id));
        response_data.insert("status".to_string(), Value::String("noop".to_string()));
        response_data.insert("compacted".to_string(), Value::Bool(false));

        return Ok(SuccessHint::new(message, vec![])
            .to_mcp_result_with_data(Some(Value::Object(response_data))));
    }

    if let Err(error) = manager
        .wait_for_compaction_to_settle(&session_id, Duration::from_secs(timeout_seconds))
        .await
    {
        return Ok(guided_error(
            ErrorCategory::Timeout,
            format!(
                "Compaction for session {} did not finish within {} seconds.",
                session_id, timeout_seconds
            ),
            ToolGroup::Agent,
        )
        .with_guidance(vec![
            format!(
                "Retry compactSessionContext(sessionId=\"{}\") after the frontend finishes the compaction request",
                session_id
            ),
            format!(
                "Use checkSession(\"{}\", wait=false) to inspect whether the delegated session is still active",
                session_id
            ),
            format!("Last wait error: {}", error),
        ])
        .to_mcp_result());
    }

    let compact_record = manager.get_compact_context(&session_id).await?;
    let Some(compact_record) = compact_record else {
        return Ok(guided_error(
            ErrorCategory::InternalError,
            format!(
                "Compaction for session {} settled but no compact summary record was saved.",
                session_id
            ),
            ToolGroup::Agent,
        )
        .with_guidance(vec![
            "Retry compactSessionContext once more".to_string(),
            "If the problem persists, inspect the target session logs for compact-request failures"
                .to_string(),
        ])
        .to_mcp_result());
    };

    let unchanged = previous_record.as_ref().is_some_and(|previous| {
        previous.from_id == compact_record.from_id
            && previous.to_id == compact_record.to_id
            && previous.summary == compact_record.summary
    });
    let status = if unchanged { "noop" } else { "success" };
    let state_label = if unchanged {
        "already current"
    } else {
        "compacted"
    };
    let message = format!(
        "Session {} {}.\n\nCompaction boundary: {} -> {}\n\nCompact summary:\n{}",
        session_id,
        state_label,
        compact_record.from_id,
        compact_record.to_id,
        compact_record.summary
    );
    let mut response_data = build_agent_tool_data(
        "compactSessionContext",
        "session",
        Some(&session_id),
        &message,
        status,
        check_session_next_actions(&session_id),
    );
    response_data.insert("sessionId".to_string(), Value::String(session_id));
    response_data.insert("status".to_string(), Value::String(status.to_string()));
    response_data.insert("compacted".to_string(), Value::Bool(!unchanged));
    response_data.insert("fromId".to_string(), Value::String(compact_record.from_id));
    response_data.insert("toId".to_string(), Value::String(compact_record.to_id));
    response_data.insert("summary".to_string(), Value::String(compact_record.summary));
    response_data.insert(
        "timeoutSeconds".to_string(),
        Value::Number(timeout_seconds.into()),
    );

    Ok(SuccessHint::new(message, vec![])
        .to_mcp_result_with_data(Some(Value::Object(response_data))))
}
