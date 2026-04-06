use serde_json::Value;

use crate::mcp::builtin::error_guidance::missing_agent_session_error;
use crate::mcp::builtin::error_guidance::SuccessHint;
use crate::mcp::builtin::session_api::formatting::{extract_session_status, is_terminal_status};
use crate::mcp::builtin::session_api::utils::{
    build_agent_session_tool_data, check_session_next_actions, count_session_turns,
    handle_wait_timeout_result, read_required_string, wait_until_session_terminal,
};
use crate::mcp::types::MCPResult;

use super::super::AgentServer;
use super::{build_paused_check_session_result, build_terminal_check_session_result};

/// checkSession handler (from awaitAgent / getAgentStatus)
pub async fn check_session(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;
    let session_id = read_required_string(&args, "sessionId")?;
    let wait = args.get("wait").and_then(|v| v.as_bool()).unwrap_or(false);
    let timeout_secs = args.get("timeout").and_then(|v| v.as_u64()).unwrap_or(3600);

    let current_session_meta = match manager.get_session(&session_id).await? {
        Some(session) => session,
        None => return Ok(missing_agent_session_error(&session_id)),
    };
    let current_status = format!("{:?}", current_session_meta.status).to_lowercase();
    let current_turn_count = count_session_turns(&session_id).await;

    if current_status == "paused" {
        return build_paused_check_session_result(&session_id, current_turn_count).await;
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
            return build_paused_check_session_result(&session_id, turn_count).await;
        }
        return build_terminal_check_session_result(&session_id, &status, turn_count).await;
    }

    let status = current_status;
    let turn_count = current_turn_count;

    if is_terminal_status(&status) {
        return build_terminal_check_session_result(&session_id, &status, turn_count).await;
    }

    let next_steps = vec![format!(
        "Use checkSession(\"{}\", wait=true) to wait for completion.",
        session_id
    )];
    let message = format!(
        "Session {} is currently {} (Turns elapsed: {}).",
        session_id, status, turn_count
    );
    let hint = SuccessHint::new(message.clone(), next_steps);

    Ok(
        hint.to_mcp_result_with_data(Some(Value::Object(build_agent_session_tool_data(
            "checkSession",
            &session_id,
            &message,
            &status,
            "pending",
            turn_count,
            check_session_next_actions(&session_id),
        )))),
    )
}
