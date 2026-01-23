use crate::agent::AgentSessionManager;

use crate::mcp::types::ServiceContext;
use crate::repositories::SessionMetadata;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{command, State};

use crate::agent::types::AgentMessageDto;
use crate::commands::workspace_commands::get_app_logs_dir;
use std::fs;

/// Request to create a new agent session
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentSessionRequest {
    pub session_id: String,
    pub name: Option<String>,
    pub agent_config: crate::agent::AgentConfig,
    #[serde(default)]
    pub is_ephemeral: bool,
}

/// Request to send a user message to trigger workflow
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendUserMessageRequest {
    pub session_id: String,
    pub message: AgentMessageDto,
}

/// Request to inject messages silently or with workflow trigger
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectMessagesRequest {
    pub session_id: String,
    pub messages: Vec<AgentMessageDto>,
    pub trigger_workflow: bool,
}

/// Request to update agent configuration for a session
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgentConfigRequest {
    pub session_id: String,
    pub agent_config: crate::agent::AgentConfig,
}

/// Response for agent operations
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// Create a new agent session
#[command]
pub async fn agent_create_session(
    manager: State<'_, AgentSessionManager>,
    request: CreateAgentSessionRequest,
) -> Result<SessionMetadata, String> {
    use crate::repositories::in_memory_session_repository::InMemorySessionRepository;
    use crate::repositories::SessionRepository;
    use std::sync::Arc;

    // Select repository based on is_ephemeral flag
    let session_repo: Arc<dyn SessionRepository> = if request.is_ephemeral {
        log::info!(
            "Creating ephemeral session (in-memory only): {}",
            request.session_id
        );
        Arc::new(InMemorySessionRepository::new()) as Arc<dyn SessionRepository>
    } else {
        log::info!(
            "Creating persistent session (DB-backed): {}",
            request.session_id
        );
        Arc::new(crate::state::get_session_repository().clone())
    };

    manager
        .create_session_with_repo(
            session_repo,
            request.session_id,
            request.name,
            request.agent_config,
        )
        .await
}

/// Resume an existing agent session
#[command]
#[allow(dead_code)]
pub async fn agent_resume_session(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<SessionMetadata, String> {
    manager.resume_session(&session_id).await
}

/// Initialize session with messages from database
#[command]
#[allow(dead_code)]
pub async fn agent_init_session_with_messages(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<AgentResponse, String> {
    manager.init_session_with_messages(&session_id).await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Session initialized with messages: {}", session_id),
        data: None,
    })
}

/// Send a user message to start an agent workflow
#[command]
pub async fn agent_send_message(
    manager: State<'_, AgentSessionManager>,
    request: SendUserMessageRequest,
) -> Result<AgentResponse, String> {
    // Message is already the correct type, no conversion needed
    let message = request.message;

    manager
        .start_workflow(request.session_id, message)
        .await
        .map(|_| AgentResponse {
            success: true,
            message: "Message sent".to_string(),
            data: None,
        })
}

/// Update agent configuration for a session
#[command]
pub async fn agent_update_session_config(
    manager: State<'_, AgentSessionManager>,
    request: UpdateAgentConfigRequest,
) -> Result<AgentResponse, String> {
    manager
        .update_session_config(request.session_id.clone(), request.agent_config)
        .await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Agent config updated for session: {}", request.session_id),
        data: None,
    })
}

/// Inject messages into the session
#[command]
pub async fn agent_inject_messages(
    manager: State<'_, AgentSessionManager>,
    request: InjectMessagesRequest,
) -> Result<AgentResponse, String> {
    manager
        .inject_messages(
            request.session_id.clone(),
            request.messages,
            request.trigger_workflow,
        )
        .await?;

    Ok(AgentResponse {
        success: true,
        message: format!(
            "Injected messages for session: {} (triggered: {})",
            request.session_id, request.trigger_workflow
        ),
        data: None,
    })
}

/// Handle LLM response from frontend (called by LLMServiceProvider in TS)
#[command]
pub async fn agent_handle_llm_response(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
    assistant_message: AgentMessageDto,
) -> Result<AgentResponse, String> {
    // AgentMessageDto is now a type alias for Message, no conversion needed
    let message = assistant_message;

    log::info!(
        "📥 Received LLM response from frontend: session={}, message_id={}, has_tool_calls={}, tool_call_count={}, content_len={}",
        session_id,
        message.id,
        message.tool_calls.is_some(),
        message.tool_calls.as_ref().map(|tc| tc.len()).unwrap_or(0),
        message.content.len()
    );

    log::debug!("📥 Full message received: {:#?}", message);

    manager
        .handle_llm_response(session_id.clone(), message)
        .await?;

    Ok(AgentResponse {
        success: true,
        message: format!("LLM response processed for session: {}", session_id),
        data: None,
    })
}

/// Get session metadata
#[command]
pub async fn agent_get_session(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<Option<SessionMetadata>, String> {
    manager.get_session(&session_id).await
}

/// Get all sessions
#[command]
pub async fn agent_get_all_sessions(
    manager: State<'_, AgentSessionManager>,
) -> Result<Vec<SessionMetadata>, String> {
    manager.get_all_sessions().await
}

/// Pause a running workflow
#[command]
pub async fn agent_pause_workflow(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<AgentResponse, String> {
    manager.pause_workflow(session_id.clone()).await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Workflow paused for session: {}", session_id),
        data: None,
    })
}

/// Resume a paused workflow
#[command]
pub async fn agent_resume_workflow(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<AgentResponse, String> {
    // Resume the workflow (internal logic handles cache validation)
    manager.resume_workflow(session_id.clone()).await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Workflow resumed: {}", session_id),
        data: None,
    })
}

/// Terminate a running workflow
#[command]
pub async fn agent_terminate_workflow(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<AgentResponse, String> {
    manager.terminate_session(session_id.clone()).await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Workflow terminated for session: {}", session_id),
        data: None,
    })
}

/// Tool execution result from frontend
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionResult {
    pub success: bool,
    pub content: String,
    pub mcp_content: Option<Vec<crate::mcp::types::MCPContent>>,
    pub error: Option<String>,
    pub is_error: bool,
}

/// Handle tool execution result from frontend (called by ToolBridgeProvider in TS)
#[command]
pub async fn agent_handle_tool_result(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
    tool_call_id: String,
    result: ToolExecutionResult,
) -> Result<AgentResponse, String> {
    manager
        .handle_tool_result(session_id.clone(), tool_call_id, result)
        .await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Tool result processed for session: {}", session_id),
        data: None,
    })
}

/// Handle LLM error from frontend (called by LLMServiceProvider in TS)
#[command]
pub async fn agent_handle_llm_error(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
    error: String,
) -> Result<AgentResponse, String> {
    manager.handle_llm_error(session_id.clone(), error).await?;

    Ok(AgentResponse {
        success: true,
        message: format!("LLM error handled for session: {}", session_id),
        data: None,
    })
}

/// Call a builtin tool directly via proxy_manager (for testing and direct execution)
/// Returns the unwrapped MCPResult (not the full MCPResponse wrapper)
#[command]
pub async fn agent_call_builtin_tool(
    session_id: String,
    tool_name: String,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    use crate::mcp::types::MCPResponseResult;
    use crate::state::get_mcp_service_proxy_manager;

    let proxy_manager = get_mcp_service_proxy_manager();

    let response = proxy_manager
        .call_tool(&session_id, &tool_name, args)
        .await?;

    // Handle errors from tool execution
    if let Some(error) = response.error {
        return Err(format!("Tool execution error: {}", error.message));
    }

    // Extract result from MCPResponse
    let result = response
        .result
        .ok_or_else(|| "Tool execution returned no result or error".to_string())?;

    // Unwrap MCPResult from MCPResponseResult::ToolCall variant
    // This matches the TypeScript expectation of receiving MCPResult directly
    match result {
        MCPResponseResult::ToolCall(mcp_result) => {
            // Serialize MCPResult (with camelCase field names matching TypeScript interface)
            serde_json::to_value(mcp_result)
                .map_err(|e| format!("Failed to serialize MCPResult: {}", e))
        }
        _ => Err(format!(
            "Unexpected response type for builtin tool '{}': expected ToolCall variant",
            tool_name
        )),
    }
}

/// Get service contexts for a session
#[command]
pub async fn agent_get_service_contexts(
    session_id: String,
) -> Result<HashMap<String, ServiceContext>, String> {
    use crate::state::get_mcp_service_proxy_manager;

    let proxy_manager = get_mcp_service_proxy_manager();

    let proxy = proxy_manager
        .get_proxy(&session_id)
        .await
        .ok_or_else(|| format!("No proxy found for session: {}", session_id))?;

    Ok(proxy.get_service_contexts().await)
}

/// Delete an agent session and all its data
#[command]
pub async fn agent_delete_session(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<AgentResponse, String> {
    manager.delete_session(session_id.clone()).await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Session deleted: {}", session_id),
        data: None,
    })
}

/// Get available tools for a specific agent session
/// Returns the filtered tool list based on agent configuration
/// This ensures UI displays the same tools that LLM can actually use
#[command]
pub async fn agent_get_available_tools(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<Vec<crate::mcp::types::MCPTool>, String> {
    manager.get_available_tools(&session_id).await
}

/// Get available tools for a session
#[command]
pub async fn agent_get_tools(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<Vec<crate::mcp::types::MCPTool>, String> {
    manager.get_tools_for_session(&session_id).await
}

/// Clear all agent sessions (used for "Clear All Sessions" feature)
#[command]
pub async fn agent_clear_all_sessions(
    manager: State<'_, AgentSessionManager>,
) -> Result<AgentResponse, String> {
    // 1. Get all sessions
    let sessions = manager.get_all_sessions().await?;
    let count = sessions.len();

    // 2. Delete each session
    for session in sessions {
        if let Err(e) = manager.delete_session(session.id.clone()).await {
            log::error!(
                "Failed to delete session {} during clear all: {}",
                session.id,
                e
            );
        }
    }

    Ok(AgentResponse {
        success: true,
        message: format!("Cleared {} sessions", count),
        data: None,
    })
}

/// Factory reset the agent system (used for "Reset All Data & Settings" feature)
/// Deletes all sessions, assistants, playbooks, mcp servers, and logs.
#[command]
pub async fn agent_factory_reset(
    manager: State<'_, AgentSessionManager>,
) -> Result<AgentResponse, String> {
    use crate::entity::{assistant, playbook};
    use crate::repositories::mcp_server_repository::MCPServerRepository;
    use crate::state::get_mcp_server_repository;
    use sea_orm::EntityTrait;

    // 1. Clear all sessions first
    agent_clear_all_sessions(manager).await?;

    let db = crate::state::get_database_connection();

    // 2. Delete all Assistants
    assistant::Entity::delete_many()
        .exec(db)
        .await
        .map_err(|e| format!("Failed to clear assistants: {}", e))?;

    // 3. Delete all Playbooks
    playbook::Entity::delete_many()
        .exec(db)
        .await
        .map_err(|e| format!("Failed to clear playbooks: {}", e))?;

    // 4. Delete all MCP Servers
    let mcp_repo = get_mcp_server_repository();
    let servers = mcp_repo
        .list()
        .await
        .map_err(|e| format!("Failed to list MCP servers: {}", e))?;
    for server in servers {
        mcp_repo
            .delete(&server.name)
            .await
            .map_err(|e| format!("Failed to delete MCP server {}: {}", server.name, e))?;
    }

    // 5. Restore default assistants so the app is not empty
    if let Err(e) = crate::services::assistant_init::ensure_default_assistants(db).await {
        return Err(format!(
            "Factory reset failed to restore default assistants: {}",
            e
        ));
    }

    // 6. Clear application logs
    // We do this last to preserve logging of the reset process as much as possible
    if let Ok(log_dir_str) = get_app_logs_dir().await {
        let log_dir = std::path::PathBuf::from(log_dir_str);
        if log_dir.exists() {
            if let Ok(entries) = fs::read_dir(&log_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                            if filename.ends_with(".log") || filename.ends_with(".log.bak") {
                                if let Err(e) = fs::remove_file(&path) {
                                    log::warn!("Failed to delete log file {:?}: {}", path, e);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(AgentResponse {
        success: true,
        message: "Factory reset completed successfully".to_string(),
        data: None,
    })
}
