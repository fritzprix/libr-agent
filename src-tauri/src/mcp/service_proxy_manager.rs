use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;

use super::server::MCPServerManager;
use super::service_proxy::MCPServiceProxy;
use super::session_isolation::{HttpSessionManager, SessionMCPManager};
use super::session_isolation_config::SessionIsolationConfig;
use super::types::MCPResponse;
use crate::session::SessionManager;

/// Manages per-session MCP service proxies for isolated tool execution
///
/// Each agent session gets its own proxy instance with dedicated builtin server instances,
/// ensuring complete isolation of tool state and context across concurrent sessions.
///
/// # Session Isolation for External MCP Servers
///
/// - **Stdio servers**: Each session gets independent process instances via SessionMCPManager
/// - **HTTP servers**: Shared connections with session ID injection via HttpSessionManager
pub struct MCPServiceProxyManager {
    /// Map of session_id to session-specific proxy instances
    proxies: Arc<RwLock<HashMap<String, Arc<MCPServiceProxy>>>>,

    /// Session-specific stdio MCP server managers (lazy-spawned per session)
    session_stdio_managers: Arc<RwLock<HashMap<String, SessionMCPManager>>>,

    /// Session-specific HTTP MCP server managers (shared connections with session headers)
    session_http_managers: Arc<RwLock<HashMap<String, HttpSessionManager>>>,

    /// Shared external MCP server manager (for HTTP servers and config)
    external_mcp_manager: Arc<MCPServerManager>,

    /// Shared SeaORM database connection for all sessions
    db: Arc<DatabaseConnection>,

    /// Shared SessionManager for workspace/content_store servers
    session_manager: Arc<SessionManager>,

    /// Background cleanup task handle
    cleanup_task: Arc<Mutex<Option<JoinHandle<()>>>>,

    /// Signal to stop the cleanup task
    cleanup_shutdown: Arc<AtomicBool>,

    /// Session isolation configuration
    config: SessionIsolationConfig,
}

impl std::fmt::Debug for MCPServiceProxyManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MCPServiceProxyManager")
            .field("proxies", &"<RwLock<HashMap>>")
            .field("session_stdio_managers", &"<RwLock<HashMap>>")
            .field("session_http_managers", &"<RwLock<HashMap>>")
            .field("external_mcp_manager", &self.external_mcp_manager)
            .field("db", &"<DatabaseConnection>")
            .field("session_manager", &self.session_manager)
            .field("cleanup_task", &"<Mutex<Option<JoinHandle>>>")
            .field(
                "cleanup_shutdown",
                &self.cleanup_shutdown.load(Ordering::Relaxed),
            )
            .field("config", &self.config)
            .finish()
    }
}

impl Drop for MCPServiceProxyManager {
    fn drop(&mut self) {
        // Signal cleanup task to stop
        self.cleanup_shutdown.store(true, Ordering::Relaxed);

        // Abort cleanup task
        if let Ok(mut task) = self.cleanup_task.try_lock() {
            if let Some(handle) = task.take() {
                handle.abort();
            }
        }
    }
}

impl MCPServiceProxyManager {
    /// Create a new proxy manager
    ///
    /// # Arguments
    /// * `external_mcp_manager` - Shared manager for external MCP servers
    /// * `db` - Shared SeaORM database connection
    /// * `session_manager` - Shared SessionManager for workspace/content_store
    pub fn new(
        external_mcp_manager: Arc<MCPServerManager>,
        db: Arc<DatabaseConnection>,
        session_manager: Arc<SessionManager>,
    ) -> Self {
        Self::new_with_config(
            external_mcp_manager,
            db,
            session_manager,
            SessionIsolationConfig::default(),
        )
    }

    /// Create a new proxy manager with custom configuration
    ///
    /// # Arguments
    /// * `external_mcp_manager` - Shared manager for external MCP servers
    /// * `db` - Shared SeaORM database connection
    /// * `session_manager` - Shared SessionManager for workspace/content_store
    /// * `config` - Session isolation configuration
    pub fn new_with_config(
        external_mcp_manager: Arc<MCPServerManager>,
        db: Arc<DatabaseConnection>,
        session_manager: Arc<SessionManager>,
        config: SessionIsolationConfig,
    ) -> Self {
        let manager = Self {
            proxies: Arc::new(RwLock::new(HashMap::new())),
            session_stdio_managers: Arc::new(RwLock::new(HashMap::new())),
            session_http_managers: Arc::new(RwLock::new(HashMap::new())),
            external_mcp_manager,
            db,
            session_manager,
            cleanup_task: Arc::new(Mutex::new(None)),
            cleanup_shutdown: Arc::new(AtomicBool::new(false)),
            config,
        };

        manager.start_cleanup_task();
        manager
    }

    /// Create a new proxy manager from static singleton references
    ///
    /// This is a convenience constructor that retrieves the global MCP manager
    /// and SeaORM database connection from the application state and creates Arc references.
    ///
    /// # Safety
    /// This uses unsafe Arc::from_raw with static references. The Arc is cloned
    /// and the original is forgotten to prevent double-free. This is safe because
    /// the underlying data has 'static lifetime.
    pub fn new_from_static_refs() -> Self {
        use crate::state::{get_database_connection, get_mcp_manager};

        // SAFETY: We now clone the manager and db connection which are safe to share
        // because they internally use Arc/ref-counting or are designed to be cloned.
        // This avoids the UB of creating an Arc from a pointer to static memory.
        let mcp_manager = get_mcp_manager();
        let mcp_manager_arc = Arc::new(mcp_manager.clone());

        let db = get_database_connection();
        let db_arc = Arc::new(db.clone());

        // Get SessionManager from the session module
        let session_manager =
            crate::session::get_session_manager().expect("SessionManager not initialized");
        let session_manager_arc = Arc::new(session_manager.clone());

        Self::new(mcp_manager_arc, db_arc, session_manager_arc)
    }

    /// Create a new session-specific proxy with dedicated tool instances
    ///
    /// # Arguments
    /// * `session_id` - Unique identifier for the agent session
    /// * `tool_ids` - List of builtin tool IDs to initialize (e.g., ["knowledge", "planning"])
    ///
    /// # Returns
    /// * `Ok(Arc<MCPServiceProxy>)` - Session-bound proxy instance
    /// * `Err(String)` - Error message if proxy creation fails
    ///
    /// # Example
    /// ```rust,ignore
    /// let proxy = manager.create_proxy(
    ///     "session-123".to_string(),
    ///     vec!["knowledge".to_string(), "planning".to_string()]
    /// ).await?;
    /// ```
    pub async fn create_proxy(
        &self,
        session_id: String,
        tool_ids: Vec<String>,
        app_handle: Option<AppHandle>,
    ) -> Result<Arc<MCPServiceProxy>, String> {
        // CRITICAL: Check if already exists (prevent race conditions)
        {
            let proxies = self.proxies.read().await;
            if let Some(existing) = proxies.get(&session_id) {
                log::debug!("Proxy already exists for session: {}", session_id);
                return Ok(existing.clone());
            }
        }

        // Clean up any stale stdio manager (rapid create/destroy cycles)
        {
            let mut stdio_managers = self.session_stdio_managers.write().await;
            if let Some(old_mgr) = stdio_managers.remove(&session_id) {
                log::debug!(
                    "Cleaning up stale stdio manager for session: {}",
                    session_id
                );
                tokio::spawn(async move {
                    old_mgr.shutdown_all().await;
                });
            }
        }

        // Create builtin proxy
        let proxy = MCPServiceProxy::new(
            session_id.clone(),
            tool_ids,
            self.external_mcp_manager.clone(),
            self.db.clone(),
            self.session_manager.clone(),
            app_handle,
        )
        .await?;

        // Create session stdio manager
        let stdio_configs = self.external_mcp_manager.get_stdio_configs().await;
        let stdio_manager =
            SessionMCPManager::new(session_id.clone(), stdio_configs, self.config.clone());

        self.session_stdio_managers
            .write()
            .await
            .insert(session_id.clone(), stdio_manager);

        // Create session HTTP manager
        let http_configs = self.external_mcp_manager.get_http_configs().await;
        let http_manager = HttpSessionManager::new(
            session_id.clone(),
            self.external_mcp_manager.clone(),
            http_configs,
        );

        self.session_http_managers
            .write()
            .await
            .insert(session_id.clone(), http_manager);

        let proxy_arc = Arc::new(proxy);
        self.proxies
            .write()
            .await
            .insert(session_id.clone(), proxy_arc.clone());

        log::info!("Created MCP service proxy for session: {}", session_id);

        Ok(proxy_arc)
    }

    /// Get an existing proxy for a session
    ///
    /// # Arguments
    /// * `session_id` - The session identifier
    ///
    /// # Returns
    /// * `Some(Arc<MCPServiceProxy>)` - Existing proxy instance
    /// * `None` - No proxy found for this session
    pub async fn get_proxy(&self, session_id: &str) -> Option<Arc<MCPServiceProxy>> {
        self.proxies.read().await.get(session_id).cloned()
    }

    /// Destroy a proxy and cleanup its resources
    ///
    /// This should be called when an agent session terminates to free resources.
    /// Builtin tool instances are automatically dropped when the proxy is removed.
    ///
    /// # Arguments
    /// * `session_id` - The session identifier
    pub async fn destroy_proxy(&self, session_id: &str) {
        // 1. Remove builtin proxy
        let proxy_removed = self.proxies.write().await.remove(session_id).is_some();

        // 2. Shutdown stdio processes
        if let Some(stdio_mgr) = self.session_stdio_managers.write().await.remove(session_id) {
            tokio::spawn(async move {
                stdio_mgr.shutdown_all().await;
            });
        }

        // 3. Remove HTTP session manager (HTTP connections are shared, just remove the manager)
        self.session_http_managers.write().await.remove(session_id);

        if proxy_removed {
            log::info!("Destroyed all resources for session: {}", session_id);
        } else {
            log::warn!(
                "Attempted to destroy non-existent proxy for session: {}",
                session_id
            );
        }
    }

    /// Call a tool via the appropriate session proxy
    ///
    /// This is the primary entry point for tool execution from agent workflows.
    /// It implements dual routing:
    /// - Builtin tools -> session proxy
    /// - External stdio tools -> session-specific stdio manager
    /// - External HTTP tools -> shared HTTP manager (TODO: Phase 3)
    ///
    /// # Arguments
    /// * `session_id` - The session making the tool call
    /// * `tool_name` - Name of the tool to invoke (e.g., "builtin_knowledge__saveKnowledge" or "filesystem__read_file")
    /// * `args` - JSON arguments for the tool
    ///
    /// # Returns
    /// * `Ok(MCPResponse)` - Tool execution result
    /// * `Err(String)` - Error if proxy not found or tool execution fails
    ///
    /// # Example
    /// ```rust,ignore
    /// let result = manager.call_tool(
    ///     "session-123",
    ///     "builtin_knowledge__saveKnowledge",
    ///     json!({"title": "My Note", "content": "Content"})
    /// ).await?;
    /// ```
    pub async fn call_tool(
        &self,
        session_id: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<MCPResponse, String> {
        // Builtin tools route through proxy
        if tool_name.starts_with("builtin_") {
            let proxy = self.get_proxy(session_id).await.ok_or_else(|| {
                let active_sessions = futures::executor::block_on(async {
                    self.proxies
                        .read()
                        .await
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>()
                });
                log::error!(
                    "No proxy found for session: {}. Active sessions: {:?}",
                    session_id,
                    active_sessions
                );
                format!("Session context not found or expired (ID: {})", session_id)
            })?;
            return proxy.call_tool(tool_name, args).await;
        }

        // External tools: parse server__tool format
        let (server_name, real_tool_name) = tool_name
            .split_once("__")
            .ok_or_else(|| format!("Invalid tool name format: {}", tool_name))?;

        // Check transport type
        let is_stdio = self.external_mcp_manager.is_stdio_server(server_name).await;

        if is_stdio {
            // Route to session-specific stdio manager
            let managers = self.session_stdio_managers.read().await;
            let manager = managers
                .get(session_id)
                .ok_or_else(|| format!("No stdio manager for session: {}", session_id))?;

            manager
                .call_tool(server_name, real_tool_name, args)
                .await
                .map_err(|e| format!("{}", e))
        } else {
            // Route to session-specific HTTP manager (with session context injection)
            let managers = self.session_http_managers.read().await;
            let manager = managers
                .get(session_id)
                .ok_or_else(|| format!("No HTTP manager for session: {}", session_id))?;

            manager
                .call_tool(server_name, real_tool_name, args)
                .await
                .map_err(|e| format!("{}", e))
        }
    }

    /// Get the number of active proxies
    ///
    /// Useful for monitoring and debugging
    pub async fn proxy_count(&self) -> usize {
        self.proxies.read().await.len()
    }

    /// List all active session IDs
    ///
    /// Useful for monitoring and debugging
    pub async fn list_sessions(&self) -> Vec<String> {
        self.proxies.read().await.keys().cloned().collect()
    }

    /// List tools from all external MCP servers
    ///
    /// This is a convenience method to access external_mcp_manager functionality
    pub async fn list_all_external_tools(&self) -> anyhow::Result<Vec<super::types::MCPTool>> {
        self.external_mcp_manager.list_all_tools().await
    }

    /// Start the background cleanup task for idle process management
    ///
    /// This task runs periodically to clean up idle MCP server processes
    /// across all active sessions.
    fn start_cleanup_task(&self) {
        let managers = self.session_stdio_managers.clone();
        let shutdown = self.cleanup_shutdown.clone();
        let interval_secs = self.config.cleanup_interval_minutes * 60;

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;

                // Check shutdown signal
                if shutdown.load(Ordering::Relaxed) {
                    log::info!("MCP cleanup task shutting down");
                    break;
                }

                // Cleanup idle processes for all sessions
                let managers_read = managers.read().await;
                for (session_id, manager) in managers_read.iter() {
                    log::debug!("Checking idle processes for session '{}'", session_id);
                    manager.cleanup_idle_processes().await;
                }
            }
        });

        if let Ok(mut task) = self.cleanup_task.try_lock() {
            *task = Some(handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{
        assistant, knowledge, planning_goal, planning_scratchpad, planning_todo, playbook, session,
    };
    use sea_orm::{ConnectionTrait, Database, EntityTrait, Schema, Set};
    use serde_json::json;

    async fn create_test_manager() -> Arc<MCPServiceProxyManager> {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory database");

        let schema = Schema::new(db.get_database_backend());

        // Create tables
        let stmt = schema.create_table_from_entity(session::Entity);
        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("Failed to create session table");

        let stmt = schema.create_table_from_entity(playbook::Entity);
        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("Failed to create playbook table");

        let stmt = schema.create_table_from_entity(assistant::Entity);
        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("Failed to create assistant table");

        let stmt = schema.create_table_from_entity(knowledge::Entity);
        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("Failed to create knowledge table");

        let stmt = schema.create_table_from_entity(planning_goal::Entity);
        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("Failed to create planning_goal table");

        let stmt = schema.create_table_from_entity(planning_todo::Entity);
        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("Failed to create planning_todo table");

        let stmt = schema.create_table_from_entity(planning_scratchpad::Entity);
        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("Failed to create planning_scratchpad table");

        // Create a minimal SessionManager for MCPServerManager
        let session_manager = Arc::new(crate::session::SessionManager::new().unwrap());
        let external_mcp_manager = Arc::new(MCPServerManager::new_with_session_manager(
            session_manager.clone(),
        ));

        Arc::new(MCPServiceProxyManager::new(
            external_mcp_manager,
            Arc::new(db),
            session_manager,
        ))
    }

    #[tokio::test]
    async fn test_phase3_playbook_and_assistant_integration() {
        let manager = create_test_manager().await;

        // Create session 1 with all Phase 3 tools
        let session1 = "test-session-1".to_string();
        let tool_ids1 = vec!["playbook".to_string(), "assistant".to_string()];

        // Insert session 1 into sessions table
        let new_session = session::ActiveModel {
            id: Set(session1.clone()),
            created_at: Set(chrono::Utc::now().timestamp()),
            updated_at: Set(0),
            status: Set("idle".to_string()),
            ..Default::default()
        };
        session::Entity::insert(new_session)
            .exec(&*manager.db)
            .await
            .unwrap();

        manager
            .create_proxy(session1.clone(), tool_ids1, None)
            .await
            .unwrap();

        // Test 1: Save a playbook in session 1
        let playbook_result = manager
            .call_tool(
                &session1,
                "builtin_playbook__createPlaybook",
                json!({
                    "goal": "Test Workflow",
                    "initialCommand": "test",
                    "workflow": [
                        {
                            "description": "Step 1",
                            "action": { "toolName": "test", "purpose": "test" },
                            "outputVariable": "out"
                        }
                    ],
                    "successCriteria": {
                        "description": "Success"
                    }
                }),
            )
            .await
            .unwrap();

        assert!(
            playbook_result.error.is_none(),
            "Playbook save should succeed"
        );

        // Test 2: Create an assistant (global scope)
        let assistant_result = manager
            .call_tool(
                &session1,
                "builtin_assistant__createAssistant",
                json!({
                    "id": "assistant1",
                    "name": "Test Assistant",
                    "config": json!({
                        "model": "gpt-4",
                        "temperature": 0.7
                    })
                }),
            )
            .await
            .unwrap();

        assert!(
            assistant_result.error.is_none(),
            "Assistant create should succeed"
        );

        // Create session 2 with same tools
        let session2 = "test-session-2".to_string();
        let tool_ids2 = vec!["playbook".to_string(), "assistant".to_string()];

        // Insert session 2 into sessions table
        let new_session = session::ActiveModel {
            id: Set(session2.clone()),
            created_at: Set(chrono::Utc::now().timestamp()),
            updated_at: Set(0),
            status: Set("idle".to_string()),
            ..Default::default()
        };
        session::Entity::insert(new_session)
            .exec(&*manager.db)
            .await
            .unwrap();

        manager
            .create_proxy(session2.clone(), tool_ids2, None)
            .await
            .unwrap();

        // Test 3: Verify playbook isolation (session 2 can't see session 1's playbook)
        let list_result = manager
            .call_tool(&session2, "builtin_playbook__listPlaybooks", json!({}))
            .await
            .unwrap();

        assert!(list_result.error.is_none());
        let result_data = list_result.result.unwrap();

        // Extract text content from ToolCall result
        let text_content = match result_data {
            crate::mcp::types::MCPResponseResult::ToolCall(ref result) => {
                if let Some(content) = &result.content {
                    if let crate::mcp::types::MCPContent::Text { text } = &content[0] {
                        text
                    } else {
                        panic!("Expected Text content")
                    }
                } else {
                    panic!("Expected content")
                }
            }
            _ => panic!("Expected ToolCall result"),
        };
        assert!(
            text_content.contains("No playbooks found"),
            "Session 2 should have 0 playbooks, got: {}",
            text_content
        );

        // Test 4: Verify assistant is global (session 2 can see the assistant)
        let get_assistant_result = manager
            .call_tool(
                &session2,
                "builtin_assistant__getAssistant",
                json!({
                    "id": "assistant1"
                }),
            )
            .await
            .unwrap();

        assert!(get_assistant_result.error.is_none());
        let assistant = get_assistant_result.result.unwrap();
        let assistant_text = match assistant {
            crate::mcp::types::MCPResponseResult::ToolCall(ref result) => {
                if let Some(content) = &result.content {
                    if let crate::mcp::types::MCPContent::Text { text } = &content[0] {
                        text
                    } else {
                        panic!("Expected Text content")
                    }
                } else {
                    panic!("Expected content")
                }
            }
            _ => panic!("Expected ToolCall result"),
        };
        assert!(
            assistant_text.contains("Test Assistant"),
            "Session 2 should see the global assistant"
        );

        // Test 5: Save same playbook ID in session 2 (allowed due to composite PK)
        let playbook2_result = manager
            .call_tool(
                &session2,
                "builtin_playbook__createPlaybook",
                json!({
                    "goal": "Session 2 Workflow",
                    "initialCommand": "test2",
                    "workflow": [
                        {
                            "description": "Step 1",
                            "action": { "toolName": "test", "purpose": "test" },
                            "outputVariable": "out"
                        }
                    ],
                    "successCriteria": {
                        "description": "Success"
                    }
                }),
            )
            .await
            .unwrap();

        assert!(
            playbook2_result.error.is_none(),
            "Session 2 should save playbook with same ID"
        );

        // Test 6: Verify each session sees its own playbook
        let list_playbook1 = manager
            .call_tool(&session1, "builtin_playbook__listPlaybooks", json!({}))
            .await
            .unwrap();

        let playbook1_result = list_playbook1.result.unwrap();
        let playbook1_text = match playbook1_result {
            crate::mcp::types::MCPResponseResult::ToolCall(ref result) => {
                if let Some(content) = &result.content {
                    if let crate::mcp::types::MCPContent::Text { text } = &content[0] {
                        text
                    } else {
                        panic!("Expected Text content")
                    }
                } else {
                    panic!("Expected content")
                }
            }
            _ => panic!("Expected ToolCall result"),
        };
        assert!(
            playbook1_text.contains("Test Workflow"),
            "Session 1 should see its own playbook"
        );

        let list_playbook2 = manager
            .call_tool(&session2, "builtin_playbook__listPlaybooks", json!({}))
            .await
            .unwrap();

        let playbook2_result = list_playbook2.result.unwrap();
        let playbook2_text = match playbook2_result {
            crate::mcp::types::MCPResponseResult::ToolCall(ref result) => {
                if let Some(content) = &result.content {
                    if let crate::mcp::types::MCPContent::Text { text } = &content[0] {
                        text
                    } else {
                        panic!("Expected Text content")
                    }
                } else {
                    panic!("Expected content")
                }
            }
            _ => panic!("Expected ToolCall result"),
        };
        assert!(
            playbook2_text.contains("Session 2 Workflow"),
            "Session 2 should see its own playbook"
        );

        // Cleanup
        manager.destroy_proxy(&session1).await;
        manager.destroy_proxy(&session2).await;
    }

    #[tokio::test]
    async fn test_phase3_concurrent_operations() {
        let manager = create_test_manager().await;

        // Create 3 concurrent sessions
        let sessions = vec![
            "concurrent-1".to_string(),
            "concurrent-2".to_string(),
            "concurrent-3".to_string(),
        ];

        // Insert sessions into database and create proxies
        for session_id in &sessions {
            let new_session = session::ActiveModel {
                id: Set(session_id.clone()),
                created_at: Set(chrono::Utc::now().timestamp()),
                updated_at: Set(0),
                status: Set("idle".to_string()),
                ..Default::default()
            };
            session::Entity::insert(new_session)
                .exec(&*manager.db)
                .await
                .unwrap();

            let tool_ids = vec!["playbook".to_string(), "assistant".to_string()];
            manager
                .create_proxy(session_id.clone(), tool_ids, None)
                .await
                .unwrap();
        }

        // Execute concurrent playbook saves
        let mut handles = vec![];
        for (idx, session_id) in sessions.iter().enumerate() {
            let mgr = manager.clone();
            let sid = session_id.clone();

            let handle = tokio::spawn(async move {
                mgr.call_tool(
                    &sid,
                    "builtin_playbook__createPlaybook",
                    json!({
                        "goal": format!("Playbook {}", idx),
                        "initialCommand": format!("test {}", idx),
                        "workflow": [
                            {
                                "description": format!("Step {}", idx),
                                "action": { "toolName": "test", "purpose": "test" },
                                "outputVariable": "out"
                            }
                        ],
                        "successCriteria": {
                            "description": "Success"
                        }
                    }),
                )
                .await
            });

            handles.push(handle);
        }

        // Wait for all operations to complete
        for handle in handles {
            let result = handle.await.unwrap().unwrap();
            assert!(
                result.error.is_none(),
                "Concurrent playbook save should succeed"
            );
        }

        // Verify each session has its own playbooks
        for (idx, session_id) in sessions.iter().enumerate() {
            let list_result = manager
                .call_tool(session_id, "builtin_playbook__listPlaybooks", json!({}))
                .await
                .unwrap();

            let result_data = list_result.result.unwrap();
            let text_content = match result_data {
                crate::mcp::types::MCPResponseResult::ToolCall(ref result) => {
                    if let Some(content) = &result.content {
                        if let crate::mcp::types::MCPContent::Text { text } = &content[0] {
                            text
                        } else {
                            panic!("Expected Text content")
                        }
                    } else {
                        panic!("Expected content")
                    }
                }
                _ => panic!("Expected ToolCall result"),
            };

            // Each session should have exactly 1 playbook
            assert!(
                text_content.contains("Found 1 playbook"),
                "Session {} should have exactly 1 playbook, got: {}",
                idx,
                text_content
            );
        }

        // Cleanup
        for session_id in &sessions {
            manager.destroy_proxy(session_id).await;
        }
    }

    #[tokio::test]
    async fn test_phase3_all_servers_integration() {
        let manager = create_test_manager().await;

        let session_id = "integration-test".to_string();

        // Insert session into database
        use crate::entity::session;
        use sea_orm::Set;

        let new_session = session::ActiveModel {
            id: Set(session_id.clone()),
            created_at: Set(chrono::Utc::now().timestamp()),
            updated_at: Set(0),
            status: Set("idle".to_string()),
            ..Default::default()
        };
        session::Entity::insert(new_session)
            .exec(&*manager.db)
            .await
            .unwrap();

        // Create proxy with ALL builtin servers
        let all_tools = vec![
            "bootstrap".to_string(),
            "knowledge".to_string(),
            "planning".to_string(),
            "playbook".to_string(),
            "assistant".to_string(),
        ];

        manager
            .create_proxy(session_id.clone(), all_tools, None)
            .await
            .unwrap();

        // Test Bootstrap (stateless)
        let bootstrap_result = manager
            .call_tool(&session_id, "builtin_bootstrap__detectPlatform", json!({}))
            .await
            .unwrap();
        assert!(bootstrap_result.error.is_none(), "Bootstrap should work");

        // Test Knowledge (session-scoped)
        let knowledge_result = manager
            .call_tool(
                &session_id,
                "builtin_knowledge__saveKnowledge",
                json!({
                    "title": "Test Knowledge",
                    "content": "Integration test content",
                    "tags": ["test", "integration"]
                }),
            )
            .await
            .unwrap();
        assert!(
            knowledge_result.error.is_none(),
            "Knowledge save should work"
        );

        // Test Planning (session-scoped)
        let planning_result = manager
            .call_tool(
                &session_id,
                "builtin_planning__createGoal",
                json!({
                    "goal": "Complete Phase 3 integration"
                }),
            )
            .await
            .unwrap();
        assert!(planning_result.error.is_none(), "Planning should work");

        // Test Playbook (session-scoped)
        let playbook_result = manager
            .call_tool(
                &session_id,
                "builtin_playbook__createPlaybook",
                json!({
                    "goal": "Integration Playbook",
                    "initialCommand": "test",
                    "workflow": [
                        {
                            "description": "Step 1",
                            "action": { "toolName": "test", "purpose": "test" },
                            "outputVariable": "out"
                        }
                    ],
                    "successCriteria": {
                        "description": "Success"
                    }
                }),
            )
            .await
            .unwrap();
        assert!(playbook_result.error.is_none(), "Playbook should work");

        // Test Assistant (global-scoped)
        let assistant_result = manager
            .call_tool(
                &session_id,
                "builtin_assistant__createAssistant",
                json!({
                    "id": "integration-assistant",
                    "name": "Integration Test Assistant",
                    "config": json!({ "model": "test" })
                }),
            )
            .await
            .unwrap();
        assert!(assistant_result.error.is_none(), "Assistant should work");

        // Verify proxy has all servers
        let proxy = manager.get_proxy(&session_id).await.unwrap();
        assert_eq!(
            proxy.builtin_server_count(),
            5,
            "Should have all 5 builtin servers"
        );

        // Cleanup
        manager.destroy_proxy(&session_id).await;
        assert_eq!(
            manager.proxy_count().await,
            0,
            "All proxies should be destroyed"
        );
    }
}
