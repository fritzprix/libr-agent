use crate::mcp::types::{MCPError, MCPResponse, MCPResponseResult, ServerCapabilities, ServerInfo};
use crate::state::get_mcp_service_proxy_manager;
use serde::Deserialize;
use warp::{http::StatusCode, Rejection};

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<serde_json::Value>,
    method: String,
    params: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,
    arguments: Option<serde_json::Value>,
}

fn error_response(id: Option<serde_json::Value>, code: i32, message: String) -> MCPResponse {
    MCPResponse {
        jsonrpc: "2.0".to_string(),
        id: id.and_then(|v| serde_json::from_value(v).ok()),
        result: None,
        error: Some(MCPError {
            code,
            message,
            data: None,
        }),
    }
}

fn ok_response(id: Option<serde_json::Value>, result: MCPResponseResult) -> MCPResponse {
    MCPResponse {
        jsonrpc: "2.0".to_string(),
        id: id.and_then(|v| serde_json::from_value(v).ok()),
        result: Some(result),
        error: None,
    }
}

fn handle_initialize(id: Option<serde_json::Value>) -> MCPResponse {
    ok_response(
        id,
        MCPResponseResult::Initialize {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ServerCapabilities {
                tools: Some(serde_json::json!({ "listChanged": false })),
                resources: None,
                prompts: None,
                experimental: None,
            },
            server_info: Some(ServerInfo {
                name: "libragent-builtin".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            }),
            instructions: None,
        },
    )
}

async fn handle_tools_list(session_id: &str, id: Option<serde_json::Value>) -> MCPResponse {
    let proxy_manager = get_mcp_service_proxy_manager();
    let proxy = match proxy_manager.get_proxy(session_id).await {
        Some(p) => p,
        None => {
            return error_response(id, -32602, format!("Session not found: {}", session_id));
        }
    };

    let mut tools = Vec::new();
    for server_id in proxy.builtin_tool_ids() {
        tools.extend(proxy.get_builtin_server_tools(&server_id));
    }

    ok_response(id, MCPResponseResult::ToolsList { tools })
}

async fn handle_tools_call(
    session_id: &str,
    id: Option<serde_json::Value>,
    params: Option<serde_json::Value>,
) -> MCPResponse {
    let params = match params {
        Some(p) => p,
        None => return error_response(id, -32602, "Missing params for tools/call".to_string()),
    };

    let call: ToolCallParams = match serde_json::from_value(params) {
        Ok(c) => c,
        Err(e) => return error_response(id, -32602, format!("Invalid params: {}", e)),
    };

    let args = call
        .arguments
        .unwrap_or_else(|| serde_json::Value::Object(Default::default()));

    let proxy_manager = get_mcp_service_proxy_manager();
    match proxy_manager.call_tool(session_id, &call.name, args).await {
        Ok(mcp_response) => {
            if let Some(MCPResponseResult::ToolCall(result)) = mcp_response.result {
                // Strip structured_content before sending to external MCP clients.
                // structured_content is a LibrAgent-internal UI extension — it must not
                // be exposed to external AI agents (only the text content array is canonical).
                use crate::mcp::types::MCPResult;
                let sanitized = MCPResult {
                    structured_content: None,
                    ..result
                };
                ok_response(id, MCPResponseResult::ToolCall(sanitized))
            } else {
                error_response(id, -32603, "Unexpected tool response format".to_string())
            }
        }
        Err(e) => error_response(id, -32603, e),
    }
}

/// Handler for `POST /mcp/{session_id}` — routes JSON-RPC 2.0 MCP requests to builtin tools.
pub async fn mcp_rpc(
    session_id: String,
    body: serde_json::Value,
) -> Result<warp::reply::WithStatus<warp::reply::Json>, Rejection> {
    let req: JsonRpcRequest = match serde_json::from_value(body) {
        Ok(r) => r,
        Err(e) => {
            let response = error_response(None, -32700, format!("Parse error: {}", e));
            return Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            ));
        }
    };

    let id = req.id.clone();
    let response = match req.method.as_str() {
        "initialize" => handle_initialize(id),
        "tools/list" => handle_tools_list(&session_id, id).await,
        "tools/call" => handle_tools_call(&session_id, id, req.params).await,
        other => error_response(id, -32601, format!("Method not found: {}", other)),
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        StatusCode::OK,
    ))
}

/// Gated wrapper that rejects requests when MCP endpoint is disabled.
pub async fn mcp_rpc_gated(
    session_id: String,
    enabled: bool,
    body: serde_json::Value,
) -> Result<warp::reply::WithStatus<warp::reply::Json>, Rejection> {
    if !enabled {
        return Err(warp::reject::not_found());
    }
    mcp_rpc(session_id, body).await
}

/// Sessionless handler for `POST /mcp` — auto-selects the first active session.
///
/// Useful for MCP clients (e.g. Copilot CLI) that register a static endpoint URL
/// without knowing the session ID upfront.
pub async fn mcp_rpc_auto(
    enabled: bool,
    body: serde_json::Value,
) -> Result<warp::reply::WithStatus<warp::reply::Json>, Rejection> {
    if !enabled {
        return Err(warp::reject::not_found());
    }

    let proxy_manager = get_mcp_service_proxy_manager();
    let sessions = proxy_manager.list_sessions().await;

    let session_id = match sessions.into_iter().next() {
        Some(id) => id,
        None => {
            let req_id = body.get("id").cloned();
            let response = error_response(
                req_id,
                -32603,
                "No active agent session found. Create a session first via POST /api/sessions or open an agent in the UI.".to_string(),
            );
            return Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            ));
        }
    };

    mcp_rpc(session_id, body).await
}
