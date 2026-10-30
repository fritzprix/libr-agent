use serde_json::Value;
use std::time::Duration;

use super::super::super::utils::{
    build_agent_tool_data, check_session_next_actions, insert_agent_session_id_fields,
    read_required_string,
};
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, SuccessHint, ToolGroup};
use crate::mcp::types::MCPResult;
use crate::repositories::SessionStatus;

use super::super::super::AgentServer;
use super::super::delegation::load_accessible_delegated_session;
use super::self_target_session_action_result;

pub async fn compact_session_context(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let session_ref = match read_required_string(&args, "sessionId") {
        Ok(value) => value,
        Err(result) => return Ok(result),
    };
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;
    let timeout_seconds = args
        .get("timeout")
        .and_then(|value| value.as_u64())
        .map(|value| value.clamp(5, 300))
        .unwrap_or(60);

    if crate::utils::session_id::session_id_matches_legacy_ref(caller_session_id, &session_ref) {
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
