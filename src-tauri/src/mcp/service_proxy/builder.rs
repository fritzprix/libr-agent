use std::sync::Arc;
use sea_orm::DatabaseConnection;
use tauri::AppHandle;
use crate::session::SessionManager;
use crate::mcp::session_isolation::{HttpSessionManager, SessionMCPManager};
use super::MCPServiceProxy;

/// Builder for MCPServiceProxy
pub struct MCPServiceProxyBuilder {
    session_id: String,
    tool_ids: Vec<String>,
    // external_mcp_manager: Arc<MCPServerManager>, // Removed as we use session isolation
    db: Arc<DatabaseConnection>,
    session_manager: Arc<SessionManager>,
    app_handle: Option<AppHandle>,
    http_manager: Arc<HttpSessionManager>,
    stdio_manager: Arc<SessionMCPManager>,
}

impl MCPServiceProxyBuilder {
    /// Create a new builder with required fields
    pub fn new(
        session_id: String,
        db: Arc<DatabaseConnection>,
        session_manager: Arc<SessionManager>,
        http_manager: Arc<HttpSessionManager>,
        stdio_manager: Arc<SessionMCPManager>,
    ) -> Self {
        Self {
            session_id,
            tool_ids: Vec::new(),
            db,
            session_manager,
            app_handle: None,
            http_manager,
            stdio_manager,
        }
    }

    /// Set the tool IDs to initialize
    pub fn with_tool_ids(mut self, tool_ids: Vec<String>) -> Self {
        self.tool_ids = tool_ids;
        self
    }

    /// Set the app handle
    pub fn with_app_handle(mut self, app_handle: Option<AppHandle>) -> Self {
        self.app_handle = app_handle;
        self
    }

    /// Build the MCPServiceProxy
    pub async fn build(self) -> Result<MCPServiceProxy, String> {
        // Fetch system settings to get timeout configuration
        let timeout = Self::fetch_tool_timeout().await;

        MCPServiceProxy::create(
            self.session_id,
            self.tool_ids,
            self.db,
            self.session_manager,
            self.app_handle,
            self.http_manager,
            self.stdio_manager,
            timeout,
        )
        .await
    }

    /// Helper to fetch tool timeout from system settings in DB
    async fn fetch_tool_timeout() -> u64 {
        use crate::repositories::settings_repository::SettingsRepository;
        use crate::state::get_settings_repository;

        // Use the repository via dependency injection if possible, or global state as fallback
        // Since we have db connection, we could query directly, but using repo is cleaner.
        // However, repo requires global state access or instantiation.
        // Let's use the global repo getter since it's available in this context usually.
        let repo = get_settings_repository();

        match repo.get("systemSettings").await {
            Ok(Some(model)) => match serde_json::from_str::<serde_json::Value>(&model.value) {
                Ok(json) => json
                    .get("mcpToolTimeoutSeconds")
                    .and_then(|v| v.as_u64())
                    .unwrap_or_else(crate::config::mcp_tool_call_timeout_seconds),
                Err(_) => crate::config::mcp_tool_call_timeout_seconds(),
            },
            _ => crate::config::mcp_tool_call_timeout_seconds(),
        }
    }
}
