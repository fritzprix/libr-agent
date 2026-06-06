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

pub fn parse_message_to_session_wait_config(
    args: &Value,
) -> Result<(bool, Option<u64>), MCPResult> {
    let wait_for_response = args
        .get("waitForResponse")
        .and_then(|value| value.as_bool())
        .unwrap_or(true);

    if !wait_for_response {
        return Ok((false, None));
    }

    let timeout_seconds = match args.get("timeout") {
        None => 3600,
        Some(value) => match value.as_u64() {
            Some(timeout) if (1..=3600).contains(&timeout) => timeout,
            _ => {
                return Err(guided_error(
                    ErrorCategory::InvalidInput,
                    "timeout must be an integer between 1 and 3600 seconds".to_string(),
                    ToolGroup::Agent,
                )
                .with_guidance(vec![
                    "Omit timeout to use the default 3600-second wait window".to_string(),
                    "Use a value between 1 and 3600 when waitForResponse=true".to_string(),
                ])
                .to_mcp_result());
            }
        },
    };

    Ok((true, Some(timeout_seconds)))
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
    let (wait_for_response, timeout_seconds) = match parse_message_to_session_wait_config(&args) {
        Ok(config) => config,
        Err(result) => return Ok(result),
    };
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

    if wait_for_response {
        return check_session(
            server,
            json!({
                "sessionId": session_id,
                "wait": true,
                "timeout": timeout_seconds.expect("wait timeout should exist when waiting")
            }),
            caller_session_id,
        )
        .await;
    }

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

    let target_session = match load_accessible_delegated_session(
        manager,
        caller_session_id,
        &session_id,
        "stopSession",
    )
    .await
    {
        Ok(session) => session,
        Err(result) => return Ok(result),
    };

    if target_session.status != SessionStatus::Busy {
        let current_status = target_session.status.as_str().to_string();
        let message = format!(
            "Session {} was already {}. No action taken.",
            session_id, current_status
        );
        let mut response_data = build_agent_tool_data(
            "stopSession",
            "session",
            Some(&session_id),
            &message,
            "noop",
            vec![],
        );
        response_data.insert("sessionId".to_string(), Value::String(session_id));
        response_data.insert("stopped".to_string(), Value::Bool(false));
        response_data.insert("status".to_string(), Value::String(current_status));

        return Ok(SuccessHint::new(message, vec![])
            .to_mcp_result_with_data(Some(Value::Object(response_data))));
    }

    if let Err(error) = manager.terminate_session(session_id.clone()).await {
        if error.contains("not found") {
            return Ok(missing_agent_session_error(&session_id));
        }
        return Err(error);
    }

    crate::services::agent_service::remove_lineage(&session_id).await;

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
                "No new compaction was needed for session {}. Existing compact summary is already current through {}.",
                session_id, record.to_id
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
        previous.to_id == compact_record.to_id && previous.summary == compact_record.summary
    });
    let status = if unchanged { "noop" } else { "success" };
    let state_label = if unchanged {
        "already current"
    } else {
        "compacted"
    };
    let message = format!(
        "Session {} {}.\n\nLatest compacted message: {}\n\nCompact summary:\n{}",
        session_id, state_label, compact_record.to_id, compact_record.summary
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
    response_data.insert("toId".to_string(), Value::String(compact_record.to_id));
    if let Some(condensed_count) = compact_record.condensed_count {
        response_data.insert(
            "condensedCount".to_string(),
            Value::Number(condensed_count.into()),
        );
    }
    response_data.insert("summary".to_string(), Value::String(compact_record.summary));
    response_data.insert(
        "timeoutSeconds".to_string(),
        Value::Number(timeout_seconds.into()),
    );

    Ok(SuccessHint::new(message, vec![])
        .to_mcp_result_with_data(Some(Value::Object(response_data))))
}

pub async fn delete_session(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;
    let session_id = read_required_string(&args, "sessionId")?;

    // 1. Prevent self-deletion (matching stopSession pattern)
    if caller_session_id == session_id {
        return Ok(self_target_session_action_result(
            "deleteSession",
            "Self-deletion is not allowed via deleteSession.",
            vec![
                "If the current session should be removed, use the normal session deletion controls in the UI instead."
                    .to_string(),
            ],
        ));
    }

    // 2. Perform lineage permissions check (reuse load_accessible_delegated_session)
    let _target_session = match load_accessible_delegated_session(
        manager,
        caller_session_id,
        &session_id,
        "deleteSession",
    )
    .await
    {
        Ok(session) => session,
        Err(result) => return Ok(result),
    };

    // 3. Execute cascade deletion
    let deleted_ids = manager.delete_session(session_id.clone()).await?;

    // 4. Clean up lineage metadata (sync with Tauri command behavior to prevent memory leaks)
    for deleted_id in &deleted_ids {
        crate::services::agent_service::remove_lineage(deleted_id).await;
    }

    // 5. Compose the response message
    // Provide a list of deleted descendant IDs so that the AI agent can parse and comprehend it
    let cascade_count = deleted_ids.len() - 1; // Exclude self
    let message = if cascade_count > 0 {
        let descendant_list = deleted_ids
            .get(1..)
            .unwrap_or(&[])
            .iter()
            .map(|id| format!("  - {}", id))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "Session {} deleted.\nCascade removed {} descendant session(s):\n{}",
            session_id, cascade_count, descendant_list
        )
    } else {
        format!("Session {} deleted.", session_id)
    };

    let hint = SuccessHint::new(message.clone(), vec![]);
    let mut response_data = build_agent_tool_data(
        "deleteSession",
        "session",
        Some(&session_id),
        &message,
        "success",
        vec![],
    );
    response_data.insert("sessionId".to_string(), Value::String(session_id));
    response_data.insert("deleted".to_string(), Value::Bool(true));
    response_data.insert(
        "descendantCount".to_string(),
        Value::Number((cascade_count as u64).into()),
    );
    response_data.insert(
        "deletedIds".to_string(),
        Value::Array(
            deleted_ids
                .iter()
                .map(|id| Value::String(id.clone()))
                .collect(),
        ),
    );

    Ok(hint.to_mcp_result_with_data(Some(Value::Object(response_data))))
}
