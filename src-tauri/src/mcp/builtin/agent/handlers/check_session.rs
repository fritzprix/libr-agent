use serde_json::Value;

use super::super::utils::{
    build_agent_session_tool_data, check_session_next_actions, count_session_turns,
    extract_session_status, handle_wait_timeout_result, is_terminal_status, read_required_string,
    wait_until_session_terminal,
};
use crate::agent::poll_tracker::{poll_tracker_key, PollTrackerVerdict};
use crate::mcp::builtin::error_guidance::SuccessHint;
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::types::MCPResult;

use super::super::AgentServer;
use super::check_session_results::{
    build_paused_check_session_result, build_terminal_check_session_result,
};
use super::delegation::load_accessible_delegated_session;
use super::enrichment::{
    append_check_session_context_to_message, apply_check_session_enrichment,
    resolve_check_session_enrichment,
};

/// checkSession handler (from awaitAgent / getAgentStatus)
pub async fn check_session(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;
    let session_ref = read_required_string(&args, "sessionId")?;
    let wait = args.get("wait").and_then(|v| v.as_bool()).unwrap_or(false);
    let timeout_secs = args.get("timeout").and_then(|v| v.as_u64()).unwrap_or(3600);

    let current_session_meta = match load_accessible_delegated_session(
        manager,
        caller_session_id,
        &session_ref,
        "checkSession",
    )
    .await
    {
        Ok(session) => session,
        Err(result) => return Ok(result),
    };
    let session_id = current_session_meta.id.clone();
    let storage_session_id =
        crate::utils::session_id::StorageSessionId::from_resolved(session_id.clone());
    let display_id = crate::utils::session_id::display_session_id(&session_id);
    let enrichment =
        resolve_check_session_enrichment(&current_session_meta, caller_session_id).await;
    let current_status = format!("{:?}", current_session_meta.status).to_lowercase();
    let current_turn_count = count_session_turns(&session_id).await;

    if current_status == "paused" {
        // Must pass storage session id — builders fetch messages by opaque key.
        // display_id is only for agent-facing copy inside those builders.
        return build_paused_check_session_result(
            &storage_session_id,
            current_turn_count,
            &enrichment,
        )
        .await;
    }

    if wait {
        let wait_result = {
            let gate = crate::state::get_concurrency_gate();
            let mut active_permit = Some(
                manager
                    .take_active_session_permit(caller_session_id)
                    .await
                    .ok_or_else(|| {
                        format!(
                            "Caller session {} is not holding an active concurrency permit",
                            caller_session_id
                        )
                    })?,
            );
            let suspended = match gate.suspend_agent(&mut active_permit).await {
                Ok(suspended) => suspended,
                Err(error) => {
                    if let Some(permit) = active_permit.take() {
                        manager
                            .restore_active_session_permit(caller_session_id, permit)
                            .await?;
                    }
                    return Err(error);
                }
            };
            let res = wait_until_session_terminal(
                manager,
                &session_id,
                timeout_secs,
                Some(caller_session_id),
            )
            .await;
            let resumed = suspended.resume().await?;
            manager
                .restore_active_session_permit(caller_session_id, resumed)
                .await?;
            res
        };

        let (session_data, _) = match handle_wait_timeout_result(
            wait_result,
            Some(manager),
            &session_id,
            timeout_secs,
            "checkSession",
            false,
        )
        .await
        {
            Ok(res) => res,
            Err(mcp_res) => return mcp_res,
        };

        let status = extract_session_status(&session_data);
        let turn_count = count_session_turns(&session_id).await;
        if status == "paused" {
            return build_paused_check_session_result(&storage_session_id, turn_count, &enrichment)
                .await;
        }
        return build_terminal_check_session_result(
            &storage_session_id,
            &status,
            turn_count,
            &enrichment,
        )
        .await;
    }

    let status = current_status;
    let turn_count = current_turn_count;
    let loop_fingerprint = format!("{status}:{turn_count}");

    if is_terminal_status(&status) {
        if let Some(sessions) = crate::state::try_get_active_sessions() {
            let active = sessions.read().await;
            if let Some(caller) = active.get(caller_session_id) {
                let key = poll_tracker_key("agent__checkSession", &display_id);
                caller.tool_poll_trackers.write().await.remove(&key);
            }
        }

        return build_terminal_check_session_result(
            &storage_session_id,
            &status,
            turn_count,
            &enrichment,
        )
        .await;
    }

    let threshold = crate::config::poll_threshold();
    let excessive = if let Some(sessions) = crate::state::try_get_active_sessions() {
        let active = sessions.read().await;
        if let Some(caller) = active.get(caller_session_id) {
            let key = poll_tracker_key("agent__checkSession", &display_id);
            let mut trackers = caller.tool_poll_trackers.write().await;
            let tracker = trackers.entry(key).or_default();
            tracker.observe(&loop_fingerprint, threshold) == PollTrackerVerdict::Excessive
        } else {
            false
        }
    } else {
        false
    };

    if excessive {
        return Ok(guided_error(
            ErrorCategory::InvalidState,
            "Excessive polling detected".to_string(),
            ToolGroup::Agent,
        )
        .guidance(vec![
            "Wait a few seconds before checking again".to_string(),
            format!(
                "Or use agent__checkSession(\"{}\", wait=true) to wait for completion",
                display_id
            ),
        ])
        .to_mcp_result());
    }

    let next_steps = vec![format!(
        "Use agent__checkSession(\"{}\", wait=true) to wait for completion.",
        display_id
    )];
    let message = append_check_session_context_to_message(
        &format!(
            "Session {} is currently {} (Turns elapsed: {}).",
            display_id, status, turn_count
        ),
        &enrichment,
    );
    let hint = SuccessHint::new(message.clone(), next_steps);
    let mut response_data = build_agent_session_tool_data(
        "checkSession",
        &display_id,
        &message,
        &status,
        "pending",
        turn_count,
        check_session_next_actions(&display_id),
    );
    apply_check_session_enrichment(&mut response_data, &enrichment);
    response_data.insert(
        "loopFingerprint".to_string(),
        Value::String(loop_fingerprint),
    );

    Ok(hint.to_mcp_result_with_data(Some(Value::Object(response_data))))
}
