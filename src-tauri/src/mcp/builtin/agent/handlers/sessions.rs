use serde_json::{json, Value};
use std::time::Duration;

use super::super::utils::{
    build_agent_tool_data, check_session_next_actions, insert_agent_session_id_fields,
    read_required_string,
};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_agent_config_error, missing_agent_session_error, ErrorCategory,
    SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::models::chat::MessageSource;
use crate::repositories::{AssistantRepository, SessionStatus};

use super::super::AgentServer;
use super::check_session::check_session;
use super::delegation::load_accessible_delegated_session;
use super::enrichment::{
    apply_check_session_enrichment, display_sanitize_workspace_path, format_workspace_status_note,
    resolve_check_session_enrichment, WorkspaceRelation,
};
use super::shared::caller_session_not_found_result;

/// Resolve display path + SHARED/ISOLATED for a newly spawned child session.
fn resolve_spawned_workspace_signal(
    caller_session_id: &str,
    child_session_id: &str,
    intended_workspace: Option<String>,
) -> (Option<String>, Option<WorkspaceRelation>) {
    let Ok(session_manager) = crate::session::get_session_manager() else {
        let display = intended_workspace
            .as_deref()
            .and_then(display_sanitize_workspace_path);
        return (display, None);
    };

    let child_raw = intended_workspace.unwrap_or_else(|| {
        session_manager
            .get_session_workspace_dir_by_id(child_session_id)
            .to_string_lossy()
            .into_owned()
    });
    let caller_raw = session_manager
        .get_session_workspace_dir_by_id(caller_session_id)
        .to_string_lossy()
        .into_owned();

    let relation = if crate::mcp::builtin::utils::workspace_paths_equivalent(
        std::path::Path::new(&child_raw),
        std::path::Path::new(&caller_raw),
    ) {
        WorkspaceRelation::Shared
    } else {
        WorkspaceRelation::Isolated
    };
    // Strip newlines before display — workspaceOverride is user-controlled and this
    // path lands in the plain-text startSession note (not only the Metadata fence).
    let display = display_sanitize_workspace_path(&child_raw);
    (display, Some(relation))
}

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

    let explicit_org = caller_explicit_org.clone();
    let assistant_id = read_required_string(&args, "agentId")?;
    let task = read_required_string(&args, "task")?;
    let requested_workspace_override = args
        .get("workspaceOverride")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let workspace_override_set = requested_workspace_override.is_some();

    let effective_workspace_path = if let Some(workspace_override) = requested_workspace_override {
        Some(workspace_override)
    } else if explicit_org.is_some() {
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
        "assistantId": assistant_id.clone(),
        "request": task.clone(),
        "workspacePath": effective_workspace_path.as_deref(),
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
                    "agent__startSession: failed to resolve assistantName for {}: {}",
                    assistant_id,
                    error
                );
            }
        }
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
    let session_ref = read_required_string(&args, "sessionId")?;
    let message_text = read_required_string(&args, "message")?;
    let reset = args.get("reset").and_then(|v| v.as_bool()).unwrap_or(false);
    let (wait_for_response, timeout_seconds) = match parse_message_to_session_wait_config(&args) {
        Ok(config) => config,
        Err(result) => return Ok(result),
    };
    let target_session = match load_accessible_delegated_session(
        manager,
        caller_session_id,
        &session_ref,
        "messageToSession",
    )
    .await
    {
        Ok(session) => session,
        Err(result) => return Ok(result),
    };
    let session_id = target_session.id.clone();
    let display_id = crate::utils::session_id::display_session_id(&session_id);
    let instruction_text = message_text.clone();
    let response = match crate::services::AgentService::send_message_to_session(
        manager,
        &session_id,
        message_text,
        Some(MessageSource::AgentTool),
        reset,
    )
    .await
    {
        Ok(response) => response,
        Err(err) if err.contains("Session not found:") => {
            return Ok(missing_agent_session_error(&session_ref));
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
        format!("Message {} for session {}.", response.status, display_id),
        vec![format!(
            "Use agent__checkSession(\"{}\", wait=true) to see the response.",
            display_id
        )],
    );
    let message = hint.message.clone();
    let mut response_data = build_agent_tool_data(
        "messageToSession",
        "session",
        Some(&display_id),
        &message,
        "pending",
        check_session_next_actions(&display_id),
    );
    insert_agent_session_id_fields(&mut response_data, &session_id);
    response_data.insert("messageId".to_string(), Value::String(response.message_id));
    response_data.insert("status".to_string(), Value::String(response.status));
    // Persist message body for human card (collapsed preview).
    response_data.insert("instruction".to_string(), Value::String(instruction_text));

    // Prefer assistant display name over opaque session id in the parent chat card.
    {
        if let Some(assistant_id) = target_session.assistant_id.as_deref() {
            response_data.insert(
                "assistantId".to_string(),
                Value::String(assistant_id.to_string()),
            );
            match crate::state::get_assistant_repository()
                .get_assistant(assistant_id)
                .await
            {
                Ok(Some(assistant)) => {
                    response_data
                        .insert("assistantName".to_string(), Value::String(assistant.name));
                }
                Ok(None) => {}
                Err(error) => {
                    log::warn!(
                        "agent__messageToSession: failed to resolve assistantName for {}: {}",
                        assistant_id,
                        error
                    );
                }
            }
        }
    }

    // Non-blocking path: attach child workspace so the parent can locate files
    // without waiting for the next checkSession.
    let enrichment = resolve_check_session_enrichment(&target_session, caller_session_id).await;
    apply_check_session_enrichment(&mut response_data, &enrichment);

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
    let session_ref = read_required_string(&args, "sessionId")?;

    if crate::utils::session_id::session_id_matches_ref(caller_session_id, &session_ref) {
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
        &session_ref,
        "stopSession",
    )
    .await
    {
        Ok(session) => session,
        Err(result) => return Ok(result),
    };
    let session_id = target_session.id.clone();
    let display_id = crate::utils::session_id::display_session_id(&session_id);

    if target_session.status != SessionStatus::Busy
        && target_session.status != SessionStatus::Queued
    {
        let current_status = target_session.status.as_str().to_string();
        let message = format!(
            "Session {} was already {}. No action taken.",
            display_id, current_status
        );
        let mut response_data = build_agent_tool_data(
            "stopSession",
            "session",
            Some(&display_id),
            &message,
            "noop",
            vec![],
        );
        insert_agent_session_id_fields(&mut response_data, &session_id);
        response_data.insert("stopped".to_string(), Value::Bool(false));
        response_data.insert("status".to_string(), Value::String(current_status));

        return Ok(SuccessHint::new(message, vec![])
            .to_mcp_result_with_data(Some(Value::Object(response_data))));
    }

    if let Err(error) = manager.terminate_session(session_id.clone()).await {
        if error.contains("not found") {
            return Ok(missing_agent_session_error(&session_ref));
        }
        return Err(error);
    }

    crate::services::agent_service::remove_lineage(&session_id).await;

    let hint = SuccessHint::new(format!("Session {} stopped.", display_id), vec![]);
    let message = hint.message.clone();
    let mut response_data = build_agent_tool_data(
        "stopSession",
        "session",
        Some(&display_id),
        &message,
        "success",
        vec![],
    );
    insert_agent_session_id_fields(&mut response_data, &session_id);
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
    let session_ref = read_required_string(&args, "sessionId")?;
    let timeout_seconds = args
        .get("timeout")
        .and_then(|value| value.as_u64())
        .map(|value| value.clamp(5, 300))
        .unwrap_or(60);

    if crate::utils::session_id::session_id_matches_ref(caller_session_id, &session_ref) {
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
        &session_ref,
        "compactSessionContext",
    )
    .await
    {
        Ok(session) => session,
        Err(result) => return Ok(result),
    };
    let session_id = target_session.id.clone();
    let display_id = crate::utils::session_id::display_session_id(&session_id);

    if target_session.status == SessionStatus::Busy
        || target_session.status == SessionStatus::Queued
    {
        return Ok(guided_error(
            ErrorCategory::InvalidState,
            format!(
                "Session {} is busy or queued. compactSessionContext only supports idle, paused, or error sessions.",
                display_id
            ),
            ToolGroup::Agent,
        )
        .with_guidance(vec![
            format!(
                "Wait for session {} to stop running before compacting it manually",
                display_id
            ),
            format!(
                "Use agent__checkSession(\"{}\", wait=true) if you need to block for the current run",
                display_id
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
                display_id, record.to_id
            )
        } else {
            format!(
                "No compaction was needed for session {}. There is not enough uncompacted history yet.",
                display_id
            )
        };
        let mut response_data = build_agent_tool_data(
            "compactSessionContext",
            "session",
            Some(&display_id),
            &message,
            "noop",
            check_session_next_actions(&display_id),
        );
        insert_agent_session_id_fields(&mut response_data, &session_id);
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
                display_id, timeout_seconds
            ),
            ToolGroup::Agent,
        )
        .with_guidance(vec![
            format!(
                "Retry compactSessionContext(sessionId=\"{}\") after the frontend finishes the compaction request",
                display_id
            ),
            format!(
                "Use agent__checkSession(\"{}\", wait=false) to inspect whether the delegated session is still active",
                display_id
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
                display_id
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
        display_id, state_label, compact_record.to_id, compact_record.summary
    );
    let mut response_data = build_agent_tool_data(
        "compactSessionContext",
        "session",
        Some(&display_id),
        &message,
        status,
        check_session_next_actions(&display_id),
    );
    insert_agent_session_id_fields(&mut response_data, &session_id);
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
    let session_ref = read_required_string(&args, "sessionId")?;

    // 1. Prevent self-deletion (matching stopSession pattern)
    if crate::utils::session_id::session_id_matches_ref(caller_session_id, &session_ref) {
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
    let target_session = match load_accessible_delegated_session(
        manager,
        caller_session_id,
        &session_ref,
        "deleteSession",
    )
    .await
    {
        Ok(session) => session,
        Err(result) => return Ok(result),
    };
    let session_id = target_session.id.clone();
    let display_id = crate::utils::session_id::display_session_id(&session_id);

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
            .map(|id| format!("  - {}", crate::utils::session_id::display_session_id(id)))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "Session {} deleted.\nCascade removed {} descendant session(s):\n{}",
            display_id, cascade_count, descendant_list
        )
    } else {
        format!("Session {} deleted.", display_id)
    };

    let hint = SuccessHint::new(message.clone(), vec![]);
    let mut response_data = build_agent_tool_data(
        "deleteSession",
        "session",
        Some(&display_id),
        &message,
        "success",
        vec![],
    );
    insert_agent_session_id_fields(&mut response_data, &session_id);
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
                .map(|id| Value::String(crate::utils::session_id::display_session_id(id)))
                .collect(),
        ),
    );

    Ok(hint.to_mcp_result_with_data(Some(Value::Object(response_data))))
}
