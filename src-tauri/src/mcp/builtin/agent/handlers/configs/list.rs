use serde_json::{json, Value};

use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, SuccessHint, ToolGroup};
use crate::mcp::types::MCPResult;
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::AssistantRepository;
use sea_orm::DatabaseConnection;

use super::super::super::formatting::{
    build_server_name_lookup, format_capability_list, format_external_server_refs,
    resolve_external_server_labels,
};
use super::super::super::utils::build_agent_tool_data;
use super::super::super::AgentServer;
use super::response::extract_string_array;

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
        "configs" => list_agent_configs(server, &args).await,
        "sessions" => list_delegated_sessions(caller_session_id, &args).await,
        _ => Ok(guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "Invalid list type '{}'. Use 'configs' or 'sessions'.",
                list_type
            ),
            ToolGroup::Agent,
        )
        .with_guidance(vec![
            "Use agent__listAgents(type=\"configs\") to see agent configurations".to_string(),
            "Use agent__listAgents(type=\"sessions\") to inspect delegated sub-agent sessions"
                .to_string(),
        ])
        .to_mcp_result()),
    }
}

fn extract_pagination_args(args: &Value) -> (usize, usize) {
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|value| value.max(1) as usize)
        .unwrap_or(20);
    let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

    (limit, offset)
}

const SUMMARY_DESCRIPTION_MAX_CHARS: usize = 60;

fn build_pagination_note(
    offset: usize,
    page_len: usize,
    total: usize,
    limit: usize,
    narrow_with_query: bool,
) -> String {
    if page_len == 0 {
        return String::new();
    }

    let start = offset + 1;
    let end = offset + page_len;

    if end < total && narrow_with_query {
        format!(
            "*(Showing {} to {} of {} items. Narrow the list with query (preferred): agent__listAgents(query=\"<keyword>\"). Only if you must page, call again with offset: {})*\n",
            start,
            end,
            total,
            offset + limit
        )
    } else if end < total {
        format!(
            "*(Showing {} to {} of {} items. Call this tool again with offset: {} to see more)*\n",
            start,
            end,
            total,
            offset + limit
        )
    } else if offset > 0 {
        format!("*(Showing {} to {} of {} items)*\n", start, end, total)
    } else {
        String::new()
    }
}

async fn list_agent_configs(server: &AgentServer, args: &Value) -> Result<MCPResult, String> {
    list_agent_configs_from_db(server.get_db(), args).await
}

pub async fn list_agent_configs_for_test(
    db: &DatabaseConnection,
    args: &Value,
) -> Result<MCPResult, String> {
    list_agent_configs_from_db(db, args).await
}

async fn list_agent_configs_from_db(
    db: &DatabaseConnection,
    args: &Value,
) -> Result<MCPResult, String> {
    let repo = crate::repositories::SqliteAssistantRepository::new(db.clone());
    let mut agents = repo.list_assistants().await.map_err(|e| e.to_string())?;

    if let Some(query) = args.get("query").and_then(|v| v.as_str()) {
        let q = query.to_lowercase();
        agents.retain(|a| {
            a.id.to_lowercase().contains(&q)
                || a.name.to_lowercase().contains(&q)
                || a.config.to_lowercase().contains(&q)
        });
    }

    let (limit, offset) = extract_pagination_args(args);
    let verbose = args
        .get("verbose")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    let total = agents.len();
    let paged_agents: Vec<_> = agents.into_iter().skip(offset).take(limit).collect();
    let mcp_repo = crate::state::get_mcp_server_repository();
    let external_servers = mcp_repo.list().await.map_err(|e| e.to_string())?;
    let server_name_lookup = build_server_name_lookup(&external_servers);

    let mut results = Vec::new();
    let mut text_summary = format!("Found {} agent configurations.\n\n", total);
    if !paged_agents.is_empty() {
        if verbose {
            text_summary.push_str("| Name | ID | Capabilities | Servers | Description |\n");
            text_summary.push_str("|---|---|---|---|---|\n");
        } else {
            text_summary.push_str("| Name | ID | Description |\n");
            text_summary.push_str("|---|---|---|\n");
        }
    } else if total > 0 {
        text_summary.push_str(&format!(
            "No results for this page (offset {}, limit {}). Try a smaller offset.\n",
            offset, limit
        ));
    }

    for agent in paged_agents {
        let parsed_config = serde_json::from_str::<Value>(&agent.config).unwrap_or(Value::Null);
        let effective_builtin_capabilities =
            crate::agent::tools::runtime_allowed_builtin_service_aliases_from_value(&parsed_config);
        let external_mcp_servers = extract_string_array(&parsed_config, "mcpServerIds");

        let mut desc = parsed_config
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        if !verbose && desc.chars().count() > SUMMARY_DESCRIPTION_MAX_CHARS {
            desc = desc
                .chars()
                .take(SUMMARY_DESCRIPTION_MAX_CHARS - 3)
                .collect::<String>()
                + "...";
        }

        let name_clean = agent.name.replace('|', "\\|");
        let id_clean = agent.id.replace('|', "\\|");
        let cap_clean = format_capability_list(&effective_builtin_capabilities).replace('|', "\\|");
        let server_clean = format_external_server_refs(&external_mcp_servers, &server_name_lookup)
            .replace('|', "\\|");
        let desc_clean = desc.replace('|', "\\|").replace('\n', " ");

        if verbose {
            text_summary.push_str(&format!(
                "| {} | `{}` | {} | {} | {} |\n",
                name_clean, id_clean, cap_clean, server_clean, desc_clean
            ));

            let res_capabilities =
                resolve_external_server_labels(&external_mcp_servers, &server_name_lookup);

            results.push(json!({
                "id": agent.id,
                "name": agent.name,
                "description": parsed_config.get("description").and_then(Value::as_str),
                "systemPrompt": parsed_config.get("systemPrompt").and_then(Value::as_str),
                "builtinCapabilities": effective_builtin_capabilities,
                "externalMcpServers": res_capabilities,
            }));
        } else {
            text_summary.push_str(&format!(
                "| {} | `{}` | {} |\n",
                name_clean, id_clean, desc_clean
            ));
            results.push(json!({
                "id": agent.id,
                "name": agent.name,
                "description": desc,
            }));
        }
    }

    let pagination_note = build_pagination_note(offset, results.len(), total, limit, true);
    if !pagination_note.is_empty() {
        text_summary.push('\n');
        text_summary.push_str(&pagination_note);
    }

    if !verbose && !results.is_empty() {
        text_summary.push_str(
            "\n*(Summary view. For capabilities, servers and full descriptions of one config, call agent__listAgents(query=\"<id>\", verbose=true))*\n",
        );
    }

    let hint = SuccessHint::new(
        text_summary,
        vec![
            "Use agent__spawnSession(configId=...) to run a delegated task using one of these configurations"
                .to_string(),
        ],
    );

    let response_message = hint.message.clone();
    let mut response_data = build_agent_tool_data(
        "agent__listAgents",
        "agentConfigCollection",
        None,
        &response_message,
        "success",
        vec![json!({
            "toolName": "agent__spawnSession",
            "reason": "Spawn a new delegated agent session using one of the configurations.",
        })],
    );
    response_data.insert("type".to_string(), Value::String("configs".to_string()));
    response_data.insert("configs".to_string(), Value::Array(results));
    response_data.insert("total".to_string(), json!(total));

    Ok(hint.to_mcp_result_with_data(Some(Value::Object(response_data))))
}

pub async fn list_delegated_sessions_for_test(
    caller_session_id: &str,
    args: &Value,
) -> Result<MCPResult, String> {
    list_delegated_sessions(caller_session_id, args).await
}

async fn list_delegated_sessions(
    caller_session_id: &str,
    args: &Value,
) -> Result<MCPResult, String> {
    let session_repo = crate::state::get_session_repository();

    let child_ids = match session_repo.get_child_session_ids(caller_session_id).await {
        Ok(ids) => ids,
        Err(_) => {
            let store = crate::services::agent_service::lineage_store().read().await;
            store
                .iter()
                .filter_map(|(id, meta)| {
                    if meta.parent_session_id.as_deref() == Some(caller_session_id) {
                        Some(id.clone())
                    } else {
                        None
                    }
                })
                .collect()
        }
    };

    let (limit, offset) = extract_pagination_args(args);
    let total = child_ids.len();
    let paged_child_ids: Vec<_> = child_ids.into_iter().skip(offset).take(limit).collect();

    let mut paged_results = Vec::new();
    for child_id in paged_child_ids {
        if let Ok(Some(child_data)) = session_repo.get_session(&child_id).await {
            let status = format!("{:?}", child_data.status).to_lowercase();
            paged_results.push(json!({
                "id": crate::utils::session_id::display_session_id(&child_id),
                "name": child_data.name.unwrap_or_else(|| "Unnamed".to_string()),
                "status": status,
                "assistantId": child_data.assistant_id,
            }));
        }
    }

    let mut message = format!("Found {} sub-agent sessions.\n\n", total);
    if !paged_results.is_empty() {
        message.push_str("| Name | Session ID | Assistant ID | Status |\n");
        message.push_str("|---|---|---|---|\n");
        for result in &paged_results {
            let name_clean = result["name"]
                .as_str()
                .unwrap_or("")
                .replace('|', "\\|")
                .replace('\n', " ");
            let id_clean = result["id"]
                .as_str()
                .unwrap_or("")
                .replace('|', "\\|")
                .replace('\n', " ");
            let assistant_id_clean = result["assistantId"]
                .as_str()
                .unwrap_or("[unbound]")
                .replace('|', "\\|")
                .replace('\n', " ");
            let status_clean = result["status"]
                .as_str()
                .unwrap_or("")
                .replace('|', "\\|")
                .replace('\n', " ");
            message.push_str(&format!(
                "| {} | `{}` | `{}` | {} |\n",
                name_clean, id_clean, assistant_id_clean, status_clean
            ));
        }
    } else if total > 0 {
        message.push_str(&format!(
            "No results for this page (offset {}, limit {}). Try a smaller offset.\n",
            offset, limit
        ));
    }

    let pagination_note = build_pagination_note(offset, paged_results.len(), total, limit, false);
    if !pagination_note.is_empty() {
        message.push('\n');
        message.push_str(&pagination_note);
    }

    let (follow_ups, next_actions) = if total == 0 {
        (
            vec![
                "No delegated child sessions are available to inspect.".to_string(),
                "Continue your assigned deliverable — do not call agent__checkSession without a listed sessionId."
                    .to_string(),
                "Re-read @teamwork/coordination/HANDOFF.md if you need sibling or handoff status."
                    .to_string(),
            ],
            Vec::new(),
        )
    } else if let Some(first_id) = paged_results
        .first()
        .and_then(|row| row.get("id"))
        .and_then(|id| id.as_str())
    {
        (
            vec![format!(
                "Use agent__checkSession(\"{}\") to inspect a listed session.",
                first_id
            )],
            vec![json!({
                "toolName": "agent__checkSession",
                "reason": "Inspect one of the listed delegated sessions in more detail.",
                "args": {
                    "sessionId": first_id,
                    "wait": false
                }
            })],
        )
    } else {
        // Non-empty total but empty page (offset past end) — do not invent a checkSession target.
        (
            vec!["Adjust offset/limit to page into the session list.".to_string()],
            Vec::new(),
        )
    };

    let hint = SuccessHint::new(message, follow_ups);
    let response_message = hint.message.clone();
    let mut response_data = build_agent_tool_data(
        "agent__listAgents",
        "sessionCollection",
        None,
        &response_message,
        "success",
        next_actions,
    );
    response_data.insert("type".to_string(), Value::String("sessions".to_string()));
    response_data.insert("sessions".to_string(), Value::Array(paged_results));
    response_data.insert("total".to_string(), json!(total));
    Ok(hint.to_mcp_result_with_data(Some(Value::Object(response_data))))
}
