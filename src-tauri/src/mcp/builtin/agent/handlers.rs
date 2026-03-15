use reqwest::Method;
use serde_json::{json, Value};

use crate::mcp::builtin::error_guidance::SuccessHint;
use crate::mcp::builtin::session_api::client::call_json;
use crate::mcp::builtin::session_api::formatting::{
    extract_session_status, latest_assistant_message_text,
};
use crate::mcp::builtin::session_api::utils::{
    count_session_turns, handle_wait_timeout_result, read_required_string,
    wait_until_session_terminal,
};
use crate::mcp::types::MCPResult;
use crate::repositories::AssistantRepository;

use super::AgentServer;

/// Unified create_agent handler (from createAssistant)
pub async fn create_agent(server: &AgentServer, args: Value) -> Result<MCPResult, String> {
    let res = crate::mcp::builtin::assistant::operations::create_assistant(
        &crate::mcp::builtin::assistant::AssistantServer::new(server.get_db().clone().into())
            .await?,
        args,
    )
    .await;

    if let Ok(ref mcp_res) = res {
        if mcp_res.is_error != Some(true) {
            server.invalidate_cache().await;
        }
    }
    res
}

/// Unified update_agent handler (from updateAssistant)
pub async fn update_agent(
    server: &AgentServer,
    args: Value,
    caller_session_id: Option<String>,
) -> Result<MCPResult, String> {
    let res = crate::mcp::builtin::assistant::operations::update_assistant(
        &crate::mcp::builtin::assistant::AssistantServer::new(server.get_db().clone().into())
            .await?,
        args,
        caller_session_id,
    )
    .await;

    if let Ok(ref mcp_res) = res {
        if mcp_res.is_error != Some(true) {
            server.invalidate_cache().await;
        }
    }
    res
}

/// Unified list handler: lists configs or sub-sessions
pub async fn list_agents_or_sessions(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let list_type = args
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("configs");

    match list_type {
        "configs" => {
            let repo = crate::repositories::SqliteAssistantRepository::new(server.get_db().clone());
            let mut agents = repo.list_assistants().await.map_err(|e| e.to_string())?;

            // Filter by query if provided
            if let Some(query) = args.get("query").and_then(|v| v.as_str()) {
                let q = query.to_lowercase();
                agents.retain(|a| {
                    a.name.to_lowercase().contains(&q) || a.config.to_lowercase().contains(&q)
                });
            }

            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
            let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

            let total = agents.len();
            let paged_agents: Vec<_> = agents.into_iter().skip(offset).take(limit).collect();

            let mut results = Vec::new();
            let mut text_summary = format!("Found {} agent configurations.\n\n", total);

            for agent in paged_agents {
                let config: Value = serde_json::from_str(&agent.config).unwrap_or_default();
                let desc = config
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("No description");
                let tools = config
                    .get("allowedBuiltInServiceAliases")
                    .cloned()
                    .unwrap_or(json!([]));

                text_summary.push_str(&format!(
                    "- **{}** (ID: `{}`)\n  Description: {}\n  Capabilities: {:?}\n\n",
                    agent.name, agent.id, desc, tools
                ));

                results.push(json!({
                    "id": agent.id,
                    "name": agent.name,
                    "description": desc,
                    "capabilities": tools
                }));
            }

            let hint = SuccessHint::new(
                text_summary,
                vec!["Use startSession(agentId=\"...\") to delegate work".to_string()],
            );
            Ok(hint.to_mcp_result_with_data(Some(json!({ "agents": results, "total": total }))))
        }
        "sessions" => {
            // Logic from getChildAgents
            let data: Value = call_json(
                Method::GET,
                &format!("/api/sessions/{}/children", caller_session_id),
                None,
                None,
            )
            .await?;

            let child_ids = data
                .get("children")
                .and_then(|v: &Value| v.as_array())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|value: Value| value.as_str().map(str::to_string))
                .collect::<Vec<_>>();

            let mut results = Vec::new();
            for child_id in &child_ids {
                if let Ok(child_data) = call_json(
                    Method::GET,
                    &format!("/api/sessions/{}", child_id),
                    None,
                    None,
                )
                .await
                {
                    let status = extract_session_status(&child_data);
                    let name = child_data
                        .get("name")
                        .and_then(|v: &Value| v.as_str())
                        .unwrap_or("Unnamed");
                    results.push(json!({
                        "id": child_id,
                        "name": name,
                        "status": status
                    }));
                }
            }

            let mut message = format!("Found {} sub-agent sessions.", results.len());
            if !results.is_empty() {
                message.push_str("\n\nActive roster:\n");
                for r in &results {
                    message.push_str(&format!(
                        "- {} (ID: {}) status={}\n",
                        r["name"], r["id"], r["status"]
                    ));
                }
            }

            let hint = SuccessHint::new(
                message,
                vec!["Use checkSession(sessionId) to get results".to_string()],
            );
            Ok(hint.to_mcp_result_with_data(Some(json!({ "sessions": results }))))
        }
        _ => Err(format!(
            "Invalid list type: {}. Use 'configs' or 'sessions'.",
            list_type
        )),
    }
}

/// startSession handler (from spawnAgent)
pub async fn start_session(
    server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let agent_id = read_required_string(&args, "agentId")?;
    let task = read_required_string(&args, "task")?;
    let wait_for_result = args
        .get("waitForResult")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let mut body = json!({
        "parentSessionId": caller_session_id,
        "assistantId": agent_id,
        "request": task,
    });

    if let Some(files) = args.get("contextFiles") {
        body["contextFiles"] = files.clone();
    }

    let data: Value = call_json(Method::POST, "/api/sessions", Some(body), None).await?;
    let session_id = data
        .get("id")
        .and_then(|v: &Value| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    if wait_for_result {
        // Reuse check_session logic
        return check_session(
            server,
            json!({ "sessionId": session_id, "wait": true }),
            caller_session_id,
        )
        .await;
    }

    let hint = SuccessHint::new(
        format!("Session started successfully (ID: {}).", session_id),
        vec![format!(
            "Use checkSession(\"{}\", wait=true) to wait for the answer.",
            session_id
        )],
    );

    Ok(hint.to_mcp_result_with_data(Some(json!({
        "sessionId": session_id,
        "status": "started"
    }))))
}

/// messageToSession handler (from messageAgent)
pub async fn message_to_session(
    _server: &AgentServer,
    args: Value,
    _caller_session_id: &str,
) -> Result<MCPResult, String> {
    let session_id = read_required_string(&args, "sessionId")?;
    let message = read_required_string(&args, "message")?;

    let data: Value = call_json(
        Method::POST,
        &format!("/api/sessions/{}/messages", session_id),
        Some(json!({ "content": message })),
        None,
    )
    .await?;

    let hint = SuccessHint::new(
        format!("Message sent to session {}.", session_id),
        vec![format!(
            "Use checkSession(\"{}\", wait=true) to see the response.",
            session_id
        )],
    );

    Ok(hint.to_mcp_result_with_data(Some(json!({
        "sessionId": session_id,
        "messageId": data.get("id")
    }))))
}

/// checkSession handler (from awaitAgent / getAgentStatus)
pub async fn check_session(
    _server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let session_id = read_required_string(&args, "sessionId")?;
    let wait = args.get("wait").and_then(|v| v.as_bool()).unwrap_or(false);
    let timeout = args.get("timeout").and_then(|v| v.as_u64()).unwrap_or(30);

    if wait {
        let wait_result = {
            let gate = crate::state::get_concurrency_gate();
            gate.suspend_agent().await?;
            let res =
                wait_until_session_terminal(&session_id, timeout, Some(caller_session_id)).await;
            gate.resume_agent().await?;
            res
        };

        let (session_data, _) =
            match handle_wait_timeout_result(wait_result, &session_id, timeout, false) {
                Ok(res) => res,
                Err(mcp_res) => return mcp_res,
            };
        let status = extract_session_status(&session_data);
        let turn_count = count_session_turns(&session_id).await;

        // Fetch latest messages to get the result
        let messages_data: Value = call_json(
            Method::GET,
            &format!("/api/sessions/{}/messages", session_id),
            None,
            Some(vec![("limit".to_string(), "5".to_string())]),
        )
        .await?;

        let messages = messages_data
            .get("messages")
            .and_then(|v: &Value| v.as_array())
            .cloned()
            .unwrap_or_default();
        let (_, assistant_text) = latest_assistant_message_text(&messages, None)
            .unwrap_or(("none".to_string(), "No final answer yet.".to_string()));

        let hint = SuccessHint::new(
            format!(
                "Session {} is terminal ({}).\n\nResult:\n{}",
                session_id, status, assistant_text
            ),
            vec![],
        );

        return Ok(hint.to_mcp_result_with_data(Some(json!({
            "sessionId": session_id,
            "status": status,
            "turnCount": turn_count,
            "result": assistant_text
        }))));
    }

    // Just check status
    let data: Value = call_json(
        Method::GET,
        &format!("/api/sessions/{}", session_id),
        None,
        None,
    )
    .await?;
    let status = extract_session_status(&data);
    let turn_count = count_session_turns(&session_id).await;

    let hint = SuccessHint::new(
        format!("Session {} is currently {}.", session_id, status),
        if status != "finished" && status != "error" {
            vec![format!(
                "Use checkSession(\"{}\", wait=true) to wait for completion.",
                session_id
            )]
        } else {
            vec![]
        },
    );

    Ok(hint.to_mcp_result_with_data(Some(json!({
        "sessionId": session_id,
        "status": status,
        "turnCount": turn_count
    }))))
}

/// stopSession handler (from terminateAgent)
pub async fn stop_session(
    _server: &AgentServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let session_id = read_required_string(&args, "sessionId")?;

    if caller_session_id == session_id {
        return Err("Self-termination is not allowed via stopSession.".to_string());
    }

    call_json(
        Method::POST,
        &format!("/api/sessions/{}/terminate", session_id),
        None,
        None,
    )
    .await?;

    let hint = SuccessHint::new(format!("Session {} stopped.", session_id), vec![]);

    Ok(hint.to_mcp_result_with_data(Some(json!({
        "sessionId": session_id,
        "stopped": true
    }))))
}
