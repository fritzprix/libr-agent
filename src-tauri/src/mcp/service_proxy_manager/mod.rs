use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;
use tokio::sync::{Mutex, RwLock};
use tokio::task::{JoinHandle, JoinSet};

use super::service_proxy::MCPServiceProxy;
use super::session_isolation::{HttpSessionManager, SessionMCPManager};
use super::session_isolation_config::SessionIsolationConfig;
use super::types::MCPResponse;
use crate::agent::events::InitializationStatus;
use crate::repositories::settings_repository::SettingsRepository;
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

    /// Readiness signal per session: true when background tool loading is complete.
    /// Sessions with no external servers are considered immediately ready (no entry).
    proxy_readiness: Arc<RwLock<HashMap<String, Arc<tokio::sync::watch::Sender<bool>>>>>,
}

impl std::fmt::Debug for MCPServiceProxyManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MCPServiceProxyManager")
            .field("proxies", &"<RwLock<HashMap>>")
            .field("session_stdio_managers", &"<RwLock<HashMap>>")
            .field("session_http_managers", &"<RwLock<HashMap>>")
            .field("db", &"<DatabaseConnection>")
            .field("session_manager", &self.session_manager)
            .field("cleanup_task", &"<Mutex<Option<JoinHandle>>>")
            .field(
                "cleanup_shutdown",
                &self.cleanup_shutdown.load(Ordering::Relaxed),
            )
            .field("config", &self.config)
            .field("proxy_readiness", &"<RwLock<HashMap>>")
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
    /// * `db` - Shared SeaORM database connection
    /// * `session_manager` - Shared SessionManager for workspace/content_store
    pub fn new(db: Arc<DatabaseConnection>, session_manager: Arc<SessionManager>) -> Self {
        Self::new_with_config(db, session_manager, SessionIsolationConfig::default())
    }

    /// Create a new proxy manager with custom configuration
    ///
    /// # Arguments
    /// * `db` - Shared SeaORM database connection
    /// * `session_manager` - Shared SessionManager for workspace/content_store
    /// * `config` - Session isolation configuration
    pub fn new_with_config(
        db: Arc<DatabaseConnection>,
        session_manager: Arc<SessionManager>,
        config: SessionIsolationConfig,
    ) -> Self {
        let manager = Self {
            proxies: Arc::new(RwLock::new(HashMap::new())),
            session_stdio_managers: Arc::new(RwLock::new(HashMap::new())),
            session_http_managers: Arc::new(RwLock::new(HashMap::new())),
            db,
            session_manager,
            cleanup_task: Arc::new(Mutex::new(None)),
            cleanup_shutdown: Arc::new(AtomicBool::new(false)),
            config,
            proxy_readiness: Arc::new(RwLock::new(HashMap::new())),
        };

        manager.start_cleanup_task();
        manager
    }

    /// Create a new proxy manager from static singleton references
    ///
    /// This is a convenience constructor that retrieves the global MCP manager
    /// and SeaORM database connection from the application state and creates Arc references.
    pub fn new_from_static_refs() -> Self {
        use crate::state::get_database_connection;

        let db = get_database_connection();
        let db_arc = Arc::new(db.clone());

        // Get SessionManager from the session module
        let session_manager =
            crate::session::get_session_manager().expect("SessionManager not initialized");
        let session_manager_arc = Arc::new(session_manager.clone());

        Self::new(db_arc, session_manager_arc)
    }

    /// Create a new session-specific proxy with dedicated tool instances
    ///
    /// # Arguments
    /// * `session_id` - Unique identifier for the agent session
    /// * `tool_ids` - List of builtin tool IDs to initialize (e.g., ["knowledge", "planning"])
    /// * `mcp_server_ids` - List of external MCP server IDs to connect (from agent config)
    ///
    /// # Returns
    /// * `Ok(Arc<MCPServiceProxy>)` - Session-bound proxy instance
    /// * `Err(String)` - Error message if proxy creation fails
    ///
    /// # Example
    /// ```rust,ignore
    /// let proxy = manager.create_proxy(
    ///     "session-123".to_string(),
    ///     vec!["knowledge".to_string(), "planning".to_string()],
    ///     vec!["filesystem".to_string()],
    ///     None
    /// ).await?;
    /// ```
    pub async fn create_proxy(
        &self,
        session_id: String,
        tool_ids: Vec<String>,
        mcp_server_ids: Vec<String>,
        app_handle: Option<AppHandle>,
    ) -> Result<Arc<MCPServiceProxy>, String> {
        // Helper to emit status updates
        let emit_status = |step: &str, status: crate::agent::events::InitializationStatus| {
            if let Some(app) = &app_handle {
                let event = crate::agent::events::AgentEvent::InitializationStep {
                    session_id: session_id.clone(),
                    step: step.to_string(),
                    status,
                };
                if let Err(e) = crate::agent::events::emit_agent_event(app, event) {
                    log::warn!("Failed to emit initialization status: {}", e);
                }
            }
        };

        // CRITICAL: Check if already exists (prevent race conditions)
        {
            let proxies = self.proxies.read().await;
            if let Some(existing) = proxies.get(&session_id) {
                log::debug!("Proxy already exists for session: {}", session_id);
                // Even if exists, we can emit complete (idempotent for UI)
                emit_status("Session services ready", InitializationStatus::Complete);
                return Ok(existing.clone());
            }
        }

        emit_status(
            "Initializing session environment",
            InitializationStatus::Running,
        );

        // Clean up any stale stdio manager (rapid create/destroy cycles)
        {
            let mut stdio_managers = self.session_stdio_managers.write().await;
            if let Some(old_mgr) = stdio_managers.remove(&session_id) {
                log::debug!(
                    "Cleaning up stale stdio manager for session: {}",
                    session_id
                );
                // Emit cleanup status if needed, but it might be too fast
                tokio::spawn(async move {
                    old_mgr.shutdown_all().await;
                });
            }
        }

        // Fetch configs directly from DB to support Session Isolation (independent of global connections)
        use crate::repositories::mcp_server_repository::MCPServerRepository;
        use crate::state::get_mcp_server_repository;

        emit_status("Loading tool configurations", InitializationStatus::Running);

        let mut stdio_configs = HashMap::new();
        let mut http_configs = HashMap::new();
        let mut server_name_to_id = HashMap::new(); // Map server names to IDs for tool count updates
        let repo = get_mcp_server_repository();

        // Filter servers based on mcp_server_ids:
        // - Empty array = NO external servers (assistant doesn't use any)
        // - Non-empty array = Only specified servers
        let use_external_servers = !mcp_server_ids.is_empty();

        match repo.list().await {
            Ok(models) => {
                log::debug!(
                    "Loaded {} MCP server configs from DB for session {} (use_external_servers: {}, allowed_ids: {:?})",
                    models.len(),
                    session_id,
                    use_external_servers,
                    mcp_server_ids
                );

                // Skip all external servers if mcp_server_ids is empty
                if !use_external_servers {
                    log::info!(
                        "Session {} has no external MCP servers configured (mcp_server_ids is empty)",
                        session_id
                    );
                } else {
                    for model in models {
                        // Only load servers specified in mcp_server_ids (IDs, not names)
                        if !mcp_server_ids.contains(&model.id) {
                            log::debug!(
                                "Skipping MCP server '{}' (ID: {}) - not in assistant's mcp_server_ids",
                                model.name,
                                model.id
                            );
                            continue;
                        }

                        match serde_json::from_str::<crate::mcp::types::MCPServerConfig>(
                            &model.config,
                        ) {
                            Ok(mut config) => {
                                // Use DB name if JSON doesn't specify one (type-safe approach)
                                let server_name = config.name.unwrap_or_else(|| model.name.clone());
                                config.name = Some(server_name.clone());

                                // Store name -> ID mapping for tool count updates
                                server_name_to_id.insert(server_name.clone(), model.id.clone());

                                log::debug!(
                                    "Loading MCP server '{}' (ID: {}) into session {}",
                                    server_name,
                                    model.id,
                                    session_id
                                );

                                match config.transport {
                                    crate::mcp::types::TransportConfig::Stdio { .. } => {
                                        stdio_configs.insert(server_name, config);
                                    }
                                    crate::mcp::types::TransportConfig::Http { .. } => {
                                        http_configs.insert(server_name, config);
                                    }
                                }
                            }
                            Err(e) => {
                                log::warn!(
                                    "Failed to parse config for MCP server '{}' (ID: {}): {}",
                                    model.name,
                                    model.id,
                                    e
                                );
                            }
                        }
                    }
                }

                log::info!(
                    "Session {} will connect to {} stdio servers and {} HTTP servers",
                    session_id,
                    stdio_configs.len(),
                    http_configs.len()
                );
            }
            Err(e) => {
                log::error!(
                    "Failed to fetch MCP server configs from DB for session {}: {}",
                    session_id,
                    e
                );
            }
        }

        // Apply user settings to config (especially startup timeout)
        let mut config = self.config.clone();
        if let Ok(settings_repo) = std::panic::catch_unwind(crate::state::get_settings_repository) {
            if let Ok(Some(model)) = settings_repo.get("systemSettings").await {
                #[derive(serde::Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct SystemSettings {
                    mcp_server_startup_timeout_seconds: Option<u64>,
                }

                if let Ok(settings) = serde_json::from_str::<SystemSettings>(&model.value) {
                    if let Some(timeout) = settings.mcp_server_startup_timeout_seconds {
                        log::debug!("Applying user setting: MCP startup timeout = {}s", timeout);
                        config = config.with_startup_timeout(timeout);
                    }
                }
            }
        }

        // Create session stdio manager
        let workspace_dir = self
            .session_manager
            .get_session_workspace_dir_by_id(&session_id);
        let stdio_manager = SessionMCPManager::new(
            session_id.clone(),
            stdio_configs.clone(),
            config,
            workspace_dir,
        );

        // Create session HTTP manager
        let http_manager = HttpSessionManager::new(session_id.clone(), http_configs.clone());

        // Start HTTP servers eagerly for session isolation
        if !http_configs.is_empty() {
            emit_status(
                "Connecting to HTTP tool servers",
                InitializationStatus::Running,
            );
        }

        for (server_name, config) in &http_configs {
            if let Err(e) = http_manager.start_server(server_name, config.clone()).await {
                log::error!(
                    "Failed to start HTTP server {} for session {}: {}",
                    server_name,
                    session_id,
                    e
                );
            }
        }

        // Create builtin proxy
        let proxy = MCPServiceProxy::builder(
            session_id.clone(),
            self.db.clone(),
            self.session_manager.clone(),
            Arc::new(http_manager.clone()),
            Arc::new(stdio_manager.clone()),
        )
        .with_tool_ids(tool_ids)
        .with_app_handle(app_handle.clone()) // Pass clone for internal use
        .build()
        .await?;

        // Store proxy
        let proxy_arc = Arc::new(proxy);
        self.proxies
            .write()
            .await
            .insert(session_id.clone(), proxy_arc.clone());

        self.session_stdio_managers
            .write()
            .await
            .insert(session_id.clone(), stdio_manager.clone());

        self.session_http_managers
            .write()
            .await
            .insert(session_id.clone(), http_manager.clone());

        // Spawn background tool loading to decouple proxy availability from external MCP server
        // startup time. POST /api/sessions must return quickly; stdio process spawn and HTTP
        // discovery can take 10-30s which previously caused the internal HTTP client to time out.
        //
        // The proxy is already registered in self.proxies at this point, so tool calls can be
        // routed immediately. Tools discovered in the background are registered via
        // set_session_stdio_tools / set_session_http_tools as they come online.
        let has_external_servers = !stdio_configs.is_empty() || !http_configs.is_empty();

        if has_external_servers {
            // Create a readiness signal so start_workflow() can wait for tool loading to complete.
            // The Sender is stored in proxy_readiness; the background task sends true when done.
            let (ready_tx, _) = tokio::sync::watch::channel(false);
            let ready_tx = Arc::new(ready_tx);
            self.proxy_readiness
                .write()
                .await
                .insert(session_id.clone(), ready_tx.clone());
            let ready_tx_bg = ready_tx;

            let proxy_bg = proxy_arc.clone();
            let stdio_manager_bg = stdio_manager;
            let http_manager_bg = http_manager;
            let session_id_bg = session_id.clone();
            let app_handle_bg = app_handle.clone(); // clone so emit_status (else branch) can still use original
            let server_name_to_id_bg = server_name_to_id;
            let stdio_configs_bg = stdio_configs;
            let http_configs_bg = http_configs;

            tokio::spawn(async move {
                let emit_bg = |step: &str, status: InitializationStatus| {
                    if let Some(app) = &app_handle_bg {
                        let event = crate::agent::events::AgentEvent::InitializationStep {
                            session_id: session_id_bg.clone(),
                            step: step.to_string(),
                            status,
                        };
                        if let Err(e) = crate::agent::events::emit_agent_event(app, event) {
                            log::warn!("Failed to emit initialization status: {}", e);
                        }
                    }
                };

                // Wrap shared lookup map in Arc so parallel tasks can share it cheaply.
                let server_name_to_id_arc = Arc::new(server_name_to_id_bg);

                // Load stdio server tools — all servers spawned concurrently.
                if !stdio_configs_bg.is_empty() {
                    log::info!(
                        "[bg] Loading tools for {} stdio servers in parallel (session: {})",
                        stdio_configs_bg.len(),
                        session_id_bg
                    );
                    emit_bg(
                        &format!("Connecting to {} stdio servers", stdio_configs_bg.len()),
                        InitializationStatus::Running,
                    );
                }

                let mut stdio_tasks: JoinSet<()> = JoinSet::new();
                for server_name in stdio_configs_bg.keys() {
                    let mgr = stdio_manager_bg.clone();
                    let proxy = proxy_bg.clone();
                    let id_map = server_name_to_id_arc.clone();
                    let session_id = session_id_bg.clone();
                    let app = app_handle_bg.clone();
                    let server_name = server_name.clone();
                    stdio_tasks.spawn(async move {
                        let emit = |step: &str, status: InitializationStatus| {
                            if let Some(app_h) = &app {
                                let event = crate::agent::events::AgentEvent::InitializationStep {
                                    session_id: session_id.clone(),
                                    step: step.to_string(),
                                    status,
                                };
                                if let Err(e) =
                                    crate::agent::events::emit_agent_event(app_h, event)
                                {
                                    log::warn!("Failed to emit initialization status: {}", e);
                                }
                            }
                        };
                        emit(
                            &format!("Connecting to {}", server_name),
                            InitializationStatus::Running,
                        );
                        log::debug!(
                            "[bg] Fetching tools from stdio server '{}' for session '{}'",
                            server_name,
                            session_id
                        );
                        match mgr.list_tools(&server_name).await {
                            Ok(tools) => {
                                log::info!(
                                    "[bg] ✅ Fetched {} tools from stdio server '{}' for session '{}'",
                                    tools.len(),
                                    server_name,
                                    session_id
                                );
                                if let Some(server_id) = id_map.get(&server_name) {
                                    let repo = crate::state::get_mcp_server_repository();
                                    if let Err(e) =
                                        repo.update_tool_count(server_id, tools.len() as i32)
                                            .await
                                    {
                                        log::warn!(
                                            "[bg] Failed to cache tool count for '{}' (ID: {}): {}",
                                            server_name,
                                            server_id,
                                            e
                                        );
                                    }
                                }
                                let prefixed_tools: Vec<_> = tools
                                    .into_iter()
                                    .map(|mut tool| {
                                        tool.name = format!("{}__{}", server_name, tool.name);
                                        tool
                                    })
                                    .collect();
                                proxy
                                    .set_session_stdio_tools(server_name.clone(), prefixed_tools)
                                    .await;
                            }
                            Err(e) => {
                                log::error!(
                                    "[bg] ❌ Failed to fetch tools from stdio server '{}' for session '{}': {:?}",
                                    server_name,
                                    session_id,
                                    e
                                );
                            }
                        }
                    });
                }
                // Await all stdio init tasks before proceeding.
                while let Some(res) = stdio_tasks.join_next().await {
                    if let Err(e) = res {
                        log::error!("[bg] stdio server init task panicked: {:?}", e);
                    }
                }

                // Load HTTP server tools — all servers spawned concurrently.
                if !http_configs_bg.is_empty() {
                    log::info!(
                        "[bg] Loading tools for {} HTTP servers in parallel (session: {})",
                        http_configs_bg.len(),
                        session_id_bg
                    );
                    emit_bg(
                        "Loading tools from HTTP servers",
                        InitializationStatus::Running,
                    );
                }

                let mut http_tasks: JoinSet<()> = JoinSet::new();
                for server_name in http_configs_bg.keys() {
                    let mgr = http_manager_bg.clone();
                    let proxy = proxy_bg.clone();
                    let id_map = server_name_to_id_arc.clone();
                    let session_id = session_id_bg.clone();
                    let server_name = server_name.clone();
                    http_tasks.spawn(async move {
                        let has_cache = proxy.has_http_tools_cached(&server_name).await;
                        if has_cache {
                            log::info!(
                                "[bg] ⚡ Skipping HTTP server '{}' - tools already cached",
                                server_name
                            );
                            return;
                        }
                        log::debug!(
                            "[bg] Fetching tools from HTTP server '{}' for session '{}'",
                            server_name,
                            session_id
                        );
                        match mgr.list_tools(&server_name).await {
                            Ok(tools) => {
                                log::info!(
                                    "[bg] ✅ Fetched {} tools from HTTP server '{}' for session '{}'",
                                    tools.len(),
                                    server_name,
                                    session_id
                                );
                                if let Some(server_id) = id_map.get(&server_name) {
                                    let repo = crate::state::get_mcp_server_repository();
                                    if let Err(e) =
                                        repo.update_tool_count(server_id, tools.len() as i32)
                                            .await
                                    {
                                        log::warn!(
                                            "[bg] Failed to cache tool count for '{}' (ID: {}): {}",
                                            server_name,
                                            server_id,
                                            e
                                        );
                                    }
                                }
                                let prefixed_tools: Vec<_> = tools
                                    .into_iter()
                                    .map(|mut tool| {
                                        tool.name = format!("{}__{}", server_name, tool.name);
                                        tool
                                    })
                                    .collect();
                                proxy
                                    .set_session_http_tools(server_name.clone(), prefixed_tools)
                                    .await;
                            }
                            Err(e) => {
                                log::error!(
                                    "[bg] ❌ Failed to fetch tools from HTTP server '{}' for session '{}': {:?}",
                                    server_name,
                                    session_id,
                                    e
                                );
                            }
                        }
                    });
                }
                // Await all HTTP init tasks before signalling readiness.
                while let Some(res) = http_tasks.join_next().await {
                    if let Err(e) = res {
                        log::error!("[bg] HTTP server init task panicked: {:?}", e);
                    }
                }

                log::info!("[bg] Tool loading complete for session: {}", session_id_bg);
                // Signal workflow start that tool loading is done.
                let _ = ready_tx_bg.send(true);
                emit_bg(
                    "Session initialization complete",
                    InitializationStatus::Complete,
                );
            });
        } else {
            // No external servers - emit complete immediately
            emit_status(
                "Session initialization complete",
                InitializationStatus::Complete,
            );
        }

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

        // 2. Cleanup readiness signal (drops Sender, waking any waiters with RecvError)
        self.proxy_readiness.write().await.remove(session_id);

        // 3. Shutdown stdio processes
        if let Some(stdio_mgr) = self.session_stdio_managers.write().await.remove(session_id) {
            tokio::spawn(async move {
                stdio_mgr.shutdown_all().await;
            });
        }

        // 4. Remove HTTP session manager (HTTP connections are shared, just remove the manager)
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

    /// Wait for background tool loading to complete for a session.
    ///
    /// Sessions backed only by builtin tools are immediately ready (no entry in the map).
    /// Sessions with external stdio/HTTP servers signal readiness via a `watch::channel`.
    ///
    /// # Arguments
    /// * `session_id` - The session to wait for
    /// * `timeout_secs` - Maximum seconds to wait before returning an error
    ///
    /// # Returns
    /// * `Ok(())` - Tools are loaded (or no external servers)
    /// * `Err(String)` - Timeout or channel error
    pub async fn wait_until_proxy_ready(
        &self,
        session_id: &str,
        timeout_secs: u64,
    ) -> Result<(), String> {
        let rx = {
            let map = self.proxy_readiness.read().await;
            map.get(session_id).map(|tx| tx.subscribe())
        };

        let Some(mut rx) = rx else {
            // No entry = no external servers = already ready
            return Ok(());
        };

        if *rx.borrow() {
            return Ok(()); // Already signaled true
        }

        tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            rx.wait_for(|v| *v),
        )
        .await
        .map_err(|_| {
            format!(
                "Proxy tool loading timed out after {}s for session: {}",
                timeout_secs, session_id
            )
        })?
        .map_err(|e| format!("Proxy readiness watch error: {}", e))?;

        Ok(())
    }

    /// Call a tool via the appropriate session proxy
    ///
    /// This is the primary entry point for tool execution from agent workflows.
    /// It implements dual routing:
    /// - Builtin tools -> session proxy
    /// - External stdio tools -> session-specific stdio manager
    /// - External HTTP tools -> shared HTTP manager
    ///
    /// # Arguments
    /// * `session_id` - The session making the tool call
    /// * `tool_name` - Name of the tool to invoke (e.g., "builtin_content_store__addContent" or "filesystem__read_file")
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
    ///     "builtin_content_store__addContent",
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

        // Check if server exists in session-specific stdio manager first (primary check)
        let stdio_managers = self.session_stdio_managers.read().await;
        let has_stdio = stdio_managers
            .get(session_id)
            .map(|mgr| mgr.has_server(server_name))
            .unwrap_or(false);

        if has_stdio {
            // Route to session-specific stdio manager
            let manager = stdio_managers
                .get(session_id)
                .ok_or_else(|| format!("No stdio manager for session: {}", session_id))?;

            return manager
                .call_tool(server_name, real_tool_name, args)
                .await
                .map_err(|e| format!("{}", e));
        }
        drop(stdio_managers);

        // Otherwise, route to session-specific HTTP manager
        let http_managers = self.session_http_managers.read().await;
        let manager = http_managers
            .get(session_id)
            .ok_or_else(|| format!("No HTTP manager for session: {}", session_id))?;

        manager
            .call_tool(server_name, real_tool_name, args)
            .await
            .map_err(|e| format!("{}", e))
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

    // list_all_external_tools removed - session isolation migration
    // See `agent/tools.rs` using `get_session_stdio_tools` and `get_session_http_tools` instead

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

/// Update tool cache in background after server initialization
///
/// This centralized function fetches tools from a server and updates the database cache.
/// It's called automatically when stdio or HTTP servers are spawned/connected.
///
/// # Arguments
/// * `server_name` - Name of the MCP server
/// * `session_id` - Session identifier for logging context
/// * `server_type` - Type of server ("stdio" or "HTTP") for logging
/// * `fetch_tools` - Async closure that fetches tools from the server
///
/// # Type Parameters
/// * `F` - Function that returns a Future
/// * `Fut` - Future that resolves to Result<Vec<MCPTool>, String>
pub fn spawn_tool_cache_update<F, Fut>(
    server_name: String,
    session_id: String,
    server_type: &'static str,
    fetch_tools: F,
) where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<Vec<super::types::MCPTool>, String>> + Send,
{
    tokio::spawn(async move {
        log::debug!(
            "Fetching tools to update cache for {} server '{}' (session: {})",
            server_type,
            server_name,
            session_id
        );

        match fetch_tools().await {
            Ok(tools) => {
                let tool_count = tools.len();

                // Update database cache
                use crate::repositories::mcp_server_repository::MCPServerRepository;
                let repo = crate::state::get_mcp_server_repository();

                // Lookup server ID by name first
                match repo.get_by_name(&server_name).await {
                    Ok(Some(server)) => {
                        if let Err(e) = repo.update_tool_count(&server.id, tool_count as i32).await
                        {
                            log::warn!(
                                "Failed to cache tool count for {} server '{}' (ID: {}): {}",
                                server_type,
                                server_name,
                                server.id,
                                e
                            );
                        } else {
                            log::debug!(
                                "Cached {} tools for {} server '{}' (ID: {})",
                                tool_count,
                                server_type,
                                server_name,
                                server.id
                            );
                        }
                    }
                    Ok(None) => {
                        log::warn!(
                            "Cannot cache tool count for {} server '{}': server not found in database",
                            server_type,
                            server_name
                        );
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to lookup {} server '{}' for tool count caching: {}",
                            server_type,
                            server_name,
                            e
                        );
                    }
                }
            }
            Err(e) => {
                log::error!(
                    "Failed to fetch tools for cache update from {} server '{}': {}",
                    server_type,
                    server_name,
                    e
                );
            }
        }
    });
}

/// Test helpers — compiled only in test mode
#[cfg(test)]
impl MCPServiceProxyManager {
    /// Inject a pending (false) readiness entry for a session.
    ///
    /// Returns the `Arc<Sender>` so the test can fire the signal manually,
    /// allowing tests to verify `wait_until_proxy_ready` blocks and then unblocks.
    pub(crate) async fn inject_pending_readiness_for_test(
        &self,
        session_id: &str,
    ) -> Arc<tokio::sync::watch::Sender<bool>> {
        let (tx, _rx) = tokio::sync::watch::channel(false);
        let tx = Arc::new(tx);
        self.proxy_readiness
            .write()
            .await
            .insert(session_id.to_string(), tx.clone());
        tx
    }

    /// Return the current number of entries in the proxy_readiness map.
    pub(crate) async fn readiness_entry_count(&self) -> usize {
        self.proxy_readiness.read().await.len()
    }
}

#[cfg(test)]
mod tests;
