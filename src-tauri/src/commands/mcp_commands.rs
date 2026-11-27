/// MCP (Model Context Protocol) server management commands
///
/// This module contains commands related to built-in MCP servers,
/// server probing, OAuth token management, and tool schema validation.
/// Session-isolated external servers are managed through MCPServiceProxyManager per session.
use crate::mcp::types::BuiltinServerInfo;
use crate::mcp::{MCPServerManager, MCPTool};

/// Probe a single MCP server by ID: connect, list tools, disconnect.
///
/// Lightweight alternative to the deprecated `list_tools_from_config`. No agent session
/// is created. The server process is spawned (stdio) or connected (HTTP), tools are
/// fetched, and the connection is torn down — all inside this single call.
///
/// Returns the number of tools discovered and their names.
#[tauri::command]
pub async fn probe_mcp_server(server_id: String) -> Result<Vec<MCPTool>, String> {
    crate::services::mcp_server_service::McpServerService::probe_server(
        crate::state::get_mcp_server_repository(),
        &server_id,
    )
    .await
}

/// Validates the JSON schema of a single MCP tool.
///
/// Stateless utility — no global state involved.
#[tauri::command]
pub fn validate_tool_schema(tool: MCPTool) -> Result<(), String> {
    MCPServerManager::validate_tool_schema(&tool).map_err(|e| e.to_string())
}

// ============================================================================
// Built-in MCP Server Commands (Session-Agnostic)
// ============================================================================

/// Lists the names of all available built-in MCP servers.
#[tauri::command]
pub async fn list_builtin_servers() -> Vec<String> {
    MCPServerManager::list_available_builtin_server_definitions()
        .into_iter()
        .map(|info| info.name)
        .collect()
}

/// Lists all tools available from the built-in MCP servers.
///
/// `server_name` filters to a specific server; omit for all servers.
/// Returns static schemas — actual tools come from session-specific proxies.
#[tauri::command]
pub async fn list_builtin_tools(server_name: Option<String>) -> Vec<MCPTool> {
    MCPServerManager::list_available_builtin_server_definitions()
        .into_iter()
        .filter(|info| {
            server_name.is_none()
                || server_name
                    .as_ref()
                    .map(|n| n == &info.name)
                    .unwrap_or(false)
        })
        .flat_map(|_info| Vec::<MCPTool>::new())
        .collect()
}

/// Lists all built-in MCP servers with their UI metadata (displayName, description, icon, etc.).
#[tauri::command]
pub async fn list_builtin_servers_with_metadata() -> Vec<BuiltinServerInfo> {
    MCPServerManager::list_available_builtin_server_definitions()
}

/// Lists all possible builtin server definitions for UI configuration.
/// Shows what builtin tools are available to assign to assistants/agents.
#[tauri::command]
pub fn list_available_builtin_server_definitions() -> Vec<BuiltinServerInfo> {
    MCPServerManager::list_available_builtin_server_definitions()
}

// ============================================================================
// OAuth Keychain Commands
// ============================================================================

/// Returns `true` if an OAuth token exists in the OS keychain for the given server.
#[tauri::command]
pub async fn has_oauth_token(server_id: String) -> bool {
    crate::mcp::keychain::has_token(&server_id).await
}

/// Retrieves a cached OAuth token from the OS keychain.
#[tauri::command]
pub async fn get_oauth_token(server_id: String) -> Result<Option<String>, String> {
    crate::mcp::keychain::get_cached_token(&server_id).await
}

/// Revokes and deletes an OAuth token from the OS keychain.
#[tauri::command]
pub async fn revoke_oauth_token(server_id: String) -> Result<String, String> {
    crate::mcp::keychain::delete_token(&server_id).await?;
    // Retire any stored success so a remount cannot show Authorized (#3040 G4.1).
    crate::mcp::oauth::clear_oauth_flow_last_outcome(&server_id).await;
    Ok(format!(
        "OAuth token revoked successfully for server: {server_id}"
    ))
}

/// Cancels an in-progress OAuth loopback wait for `server_id` (#3040).
#[tauri::command]
pub async fn cancel_oauth_flow(server_id: String) -> Result<String, String> {
    crate::mcp::oauth::cancel_oauth_flow(&server_id).await?;
    Ok(format!(
        "OAuth authorization cancelled for server: {server_id}"
    ))
}

/// Returns `true` while an OAuth flow is in progress for `server_id` (#3040
/// G1.5): the Extensions card derives its "in progress" state from the flow
/// owner so a remounted card shows Cancel for a flow it did not start.
#[tauri::command]
pub async fn is_oauth_flow_active(server_id: String) -> bool {
    crate::mcp::oauth::is_oauth_flow_active(&server_id).await
}

/// Take the last terminal OAuth outcome for `server_id` (#3040 G3.1 / G4.1).
/// Only a card that was waiting for the flow should call this; the outcome
/// is consumed so it cannot resurrect Authorized / old notices on remount.
#[tauri::command]
pub async fn take_oauth_flow_last_outcome(
    server_id: String,
) -> Option<crate::mcp::oauth::OAuthFlowEndedPayload> {
    crate::mcp::oauth::take_oauth_flow_last_outcome(&server_id).await
}

/// Starts the OAuth 2.1 authorization flow: opens browser, runs loopback listener,
/// exchanges code, and saves token in keychain.
#[tauri::command]
pub async fn start_oauth_flow(
    app_handle: tauri::AppHandle,
    server_id: String,
    config: crate::mcp::types::OAuthConfig,
) -> Result<String, String> {
    use tauri::Emitter;
    use tauri_plugin_opener::OpenerExt;

    // Single-flight: a second start must not bind the shared callback port (#3040).
    let cancel = crate::mcp::oauth::begin_oauth_flow(&server_id).await?;

    let result = async {
        let oauth_manager = crate::mcp::oauth::OAuthManager::new();

        // Step 1: Start auth flow and get URL + state
        let (auth_url, state) = oauth_manager
            .start_authorization_flow(&config, &server_id)
            .await?;

        // Step 2: Open browser
        log::info!("Opening authorization URL in browser: {}", auth_url);
        app_handle
            .opener()
            .open_url(&auth_url, Option::<String>::None)
            .map_err(|e| format!("Failed to open browser: {e}"))?;

        // Step 3: Run temporary loopback server and wait for code
        let redirect_uri = config
            .redirect_uri
            .clone()
            .unwrap_or_else(|| crate::mcp::oauth::DEFAULT_CALLBACK_URL.to_string());

        // Verify it is a localhost loopback callback with a port
        if let Ok(parsed_url) = url::Url::parse(&redirect_uri) {
            if parsed_url.scheme() != "http" {
                return Err(
                    "Unsupported redirect URI scheme. Only http:// is supported for loopback listeners."
                        .to_string(),
                );
            }
            let host = parsed_url.host_str().unwrap_or("");
            if host != "localhost" && host != "127.0.0.1" {
                return Err(format!(
                    "Security violation: host '{}' is not allowed for redirect URI. Only 'localhost' or '127.0.0.1' are allowed.",
                    host
                ));
            }
            if parsed_url.port().is_none() {
                return Err(format!(
                    "Redirect URI must specify a port number (e.g. {}) to bind the loopback listener.",
                    crate::mcp::oauth::DEFAULT_CALLBACK_URL
                ));
            }
        } else {
            return Err(format!("Invalid redirect URI: {}", redirect_uri));
        }

        let code = oauth_manager
            .wait_for_callback(
                &redirect_uri,
                &state,
                cancel.clone(),
                crate::mcp::oauth::OAUTH_CALLBACK_TIMEOUT,
            )
            .await?;

        // Step 4: Exchange code for token
        let access_token = oauth_manager
            .exchange_code_for_token(&config, &server_id, &code, &state)
            .await?;

        // Step 5: Save token in keychain. `finish_oauth_flow` re-checks the
        // flow's cancel token under the slot lock, so a cancel landing during
        // the code exchange still prevents storing (#3040 G1.4); fail paths
        // above already returned without storing.
        crate::mcp::oauth::finish_oauth_flow(
            &server_id,
            access_token,
            |sid, token| async move {
                crate::mcp::keychain::store_token_securely(&sid, &token).await
            },
        )
        .await?;

        Ok(format!(
            "OAuth integration successful for server: {}",
            server_id
        ))
    }
    .await;

    crate::mcp::oauth::end_oauth_flow(&server_id).await;
    // Broadcast the terminal outcome before returning so cards without a
    // local promise (reload / remount) learn the ending (#3040 G3.1).
    let ended = crate::mcp::oauth::record_oauth_flow_outcome(&server_id, &result).await;
    let _ = app_handle.emit("mcp-oauth-flow-ended", &ended);
    result
}

/// Manually retry a failed MCP server in an active session.
#[tauri::command]
pub async fn retry_session_mcp_server(
    session_id: String,
    server_name: String,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let proxy_manager = crate::state::get_mcp_service_proxy_manager();
    proxy_manager
        .retry_failed_server(&session_id, &server_name, Some(&app_handle))
        .await
}
