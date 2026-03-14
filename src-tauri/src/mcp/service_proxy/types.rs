use crate::mcp::server::MCPServerManager;
use crate::mcp::session_isolation::{HttpSessionManager, SessionMCPManager};
use crate::session::SessionManager;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tauri::AppHandle;

/// Configuration for creating an `MCPServiceProxy`
#[derive(Debug)]
pub struct ProxyConfig {
    /// Unique identifier for the agent session
    pub session_id: String,
    /// List of builtin tool IDs to initialize
    pub tool_ids: Vec<String>,
    /// Optional Tauri app handle for builtin servers
    pub app_handle: Option<AppHandle>,
}

/// Shared manager dependencies for `MCPServiceProxy`
#[derive(Debug, Clone)]
pub struct SharedManagers {
    /// Shared manager for external MCP servers
    pub external_mcp: Arc<MCPServerManager>,
    /// Shared `SeaORM` database connection
    pub db: Arc<DatabaseConnection>,
    /// Shared `SessionManager` for workspace/attachments
    pub session_manager: Arc<SessionManager>,
}

/// Session-specific manager dependencies
#[derive(Debug, Clone)]
pub struct SessionManagers {
    /// Session-specific HTTP manager
    pub http: Arc<HttpSessionManager>,
    /// Session-specific Stdio manager
    pub stdio: Arc<SessionMCPManager>,
}
