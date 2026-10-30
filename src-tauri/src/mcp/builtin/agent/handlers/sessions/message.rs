use serde_json::{json, Value};

use super::super::super::utils::{
    build_agent_tool_data, check_session_next_actions, insert_agent_session_id_fields,
    read_complexity, read_required_string, resolve_model_for_complexity,
};
use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, SuccessHint, ToolGroup};
use crate::mcp::types::MCPResult;
use crate::models::chat::MessageSource;
use crate::repositories::AssistantRepository;

use super::super::super::AgentServer;
use super::super::check_session::check_session;
use super::super::delegation::{
    load_accessible_delegated_session, missing_accessible_session_error,
};
use super::super::enrichment::{apply_check_session_enrichment, resolve_check_session_enrichment};

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

/// messageToSession handler (from messageAgent)
pub async fn message_to_session(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let manager = server
        .get_manager()
        .ok_or("AgentSessionManager not available")?;
    let session_ref = match read_required_string(&args, "sessionId") {
        Ok(value) => value,
        Err(result) => return Ok(result),
    };
    let message_text = match read_required_string(&args, "message") {
        Ok(value) => value,
        Err(result) => return Ok(result),
    };
    let complexity = match read_complexity(&args) {
        Ok(level) => level,
        Err(result) => return Ok(result),
    };
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
    let target_model = resolve_model_for_complexity(complexity).await?;
    if let Some(resolved) = target_model.as_ref() {
        if target_session.model != resolved.model || target_session.provider != resolved.provider {
            log::info!(
                "agent__messageToSession: escalating session {} model {}/{} -> {}/{} (complexity={})",
                display_id,
                target_session.model,
                target_session.provider,
                resolved.model,
                resolved.provider,
                complexity
            );
            manager
                .update_session_config(
                    session_id.clone(),
                    Some(resolved.model.clone()),
                    Some(resolved.provider.clone()),
                    None,
                    None,
                )
                .await?;
        }
    }
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
            return Ok(missing_accessible_session_error(caller_session_id, &session_ref).await);
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
    response_data.insert(
        "complexity".to_string(),
        Value::String(complexity.to_string()),
    );
    if let Some(resolved) = target_model.as_ref() {
        response_data.insert("model".to_string(), Value::String(resolved.model.clone()));
        response_data.insert(
            "provider".to_string(),
            Value::String(resolved.provider.clone()),
        );
    }
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
