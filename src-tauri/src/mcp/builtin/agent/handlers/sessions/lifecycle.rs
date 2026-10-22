use serde_json::Value;

use super::super::super::utils::{
    build_agent_tool_data, insert_agent_session_id_fields, read_required_string,
};
use crate::mcp::builtin::error_guidance::SuccessHint;
use crate::mcp::types::MCPResult;
use crate::repositories::SessionStatus;

use super::super::super::AgentServer;
use super::super::delegation::{
    load_accessible_delegated_session, missing_accessible_session_error,
};
use super::self_target_session_action_result;

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

    if crate::utils::session_id::session_id_matches_legacy_ref(caller_session_id, &session_ref) {
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
            return Ok(missing_accessible_session_error(caller_session_id, &session_ref).await);
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
    if crate::utils::session_id::session_id_matches_legacy_ref(caller_session_id, &session_ref) {
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
