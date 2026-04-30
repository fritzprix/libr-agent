use super::super::service_proxy::MCPServiceProxy;
use super::super::session_isolation::{HttpSessionManager, SessionMCPManager};
use super::super::session_isolation_config::SessionIsolationConfig;
use super::background_discovery::{spawn_background_tool_loading, BackgroundDiscoveryPlan};
use super::proxy_config::{
    apply_startup_timeout_settings, decide_existing_proxy_disposition,
    load_requested_server_configs, ExistingProxyDisposition,
};
use super::runtime_updates::{
    apply_config_load_failed, apply_http_connecting, apply_loading_tool_config,
    apply_proxy_created, apply_server_discovering, apply_server_failed,
    build_bootstrap_runtime_state, emit_runtime_state, mutate_runtime_state_store,
};
use super::MCPServiceProxyManager;
use crate::agent::runtime_state::{SessionRuntimeState, SessionRuntimeTransport};
use crate::session::SessionManager;
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinSet;

struct HttpStartupResult {
    server_name: String,
    outcome: Result<(), String>,
}

async fn cleanup_session_resources(manager: &MCPServiceProxyManager, session_id: &str) {
    manager.proxy_readiness.write().await.remove(session_id);
    manager.runtime_states.write().await.remove(session_id);
    if let Some(old_mgr) = manager
        .session_stdio_managers
        .write()
        .await
        .remove(session_id)
    {
        tokio::spawn(async move {
            old_mgr.shutdown_all().await;
        });
    }
    manager
        .session_http_managers
        .write()
        .await
        .remove(session_id);
}

async fn commit_runtime_state_update<F>(
    manager: &MCPServiceProxyManager,
    session_id: &str,
    app_handle: Option<&AppHandle>,
    update: F,
) -> SessionRuntimeState
where
    F: FnOnce(&mut SessionRuntimeState),
{
    let runtime_state =
        mutate_runtime_state_store(&manager.runtime_states, session_id, update).await;
    emit_runtime_state(session_id, &runtime_state, app_handle);
    runtime_state
}

async fn start_http_servers_in_parallel(
    http_manager: &HttpSessionManager,
    http_configs: &HashMap<String, crate::mcp::types::MCPServerConfig>,
) -> Vec<HttpStartupResult> {
    let mut tasks: JoinSet<HttpStartupResult> = JoinSet::new();

    for (server_name, config) in http_configs {
        let manager = http_manager.clone();
        let server_name = server_name.clone();
        let config = config.clone();
        tasks.spawn(async move {
            let outcome = manager
                .start_server(&server_name, config)
                .await
                .map_err(|error| error.to_string());
            HttpStartupResult {
                server_name,
                outcome,
            }
        });
    }

    let mut results = Vec::with_capacity(http_configs.len());
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(startup_result) => results.push(startup_result),
            Err(error) => {
                log::error!("HTTP server startup task panicked: {:?}", error);
            }
        }
    }
    results.sort_by(|left, right| left.server_name.cmp(&right.server_name));
    results
}

impl MCPServiceProxyManager {
    /// Create a new proxy manager
    ///
    /// # Arguments
    /// * `db` - Shared SeaORM database connection
    /// * `session_manager` - Shared SessionManager for workspace/attachments
    pub fn new(db: Arc<DatabaseConnection>, session_manager: Arc<SessionManager>) -> Self {
        Self::new_with_config(db, session_manager, SessionIsolationConfig::default())
    }

    /// Create a new proxy manager with custom configuration
    ///
    /// # Arguments
    /// * `db` - Shared SeaORM database connection
    /// * `session_manager` - Shared SessionManager for workspace/attachments
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
            creation_guards: Arc::new(Mutex::new(HashMap::new())),
            runtime_states: Arc::new(RwLock::new(HashMap::new())),
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
    pub async fn create_proxy(
        &self,
        session_id: String,
        tool_ids: Vec<String>,
        mcp_server_ids: Vec<String>,
        app_handle: Option<AppHandle>,
    ) -> Result<Arc<MCPServiceProxy>, String> {
        let session_guard = {
            let mut guards = self.creation_guards.lock().await;
            guards
                .entry(session_id.clone())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _session_lock = session_guard.lock().await;

        let loaded = load_requested_server_configs(&mcp_server_ids, &tool_ids, &session_id).await;

        if let Some(existing) = self.get_proxy(&session_id).await {
            let mut existing_builtin_ids = existing.builtin_tool_ids();
            existing_builtin_ids.sort();
            existing_builtin_ids.dedup();

            let existing_external_server_names = existing.configured_external_server_names().await;
            match decide_existing_proxy_disposition(
                &existing_builtin_ids,
                &existing_external_server_names,
                &loaded.requested_builtin_ids,
                &loaded.requested_external_server_names,
                loaded.config_load_error.is_some(),
            ) {
                ExistingProxyDisposition::Reuse => {
                    if let Some(load_error) = loaded.config_load_error.as_ref() {
                        log::warn!(
                            "Reusing existing proxy for session {} because MCP server configs could not be loaded: {}",
                            session_id,
                            load_error
                        );
                    } else {
                        log::debug!("Proxy already exists for session: {}", session_id);
                    }
                    let runtime_state = self.get_runtime_state(&session_id).await;
                    self.set_runtime_state(&session_id, runtime_state, app_handle.as_ref())
                        .await;
                    return Ok(existing);
                }
                ExistingProxyDisposition::Fail => {
                    let load_error = loaded
                        .config_load_error
                        .as_deref()
                        .unwrap_or("unknown error");
                    return Err(format!(
                        "Failed to load MCP server configs for session {} while updating builtin tools: {}",
                        session_id, load_error
                    ));
                }
                ExistingProxyDisposition::Recreate => {
                    log::warn!(
                        "Recreating proxy for session {} due to config mismatch (builtin: {:?} -> {:?}, external: {:?} -> {:?})",
                        session_id,
                        existing_builtin_ids,
                        loaded.requested_builtin_ids,
                        existing_external_server_names,
                        loaded.requested_external_server_names
                    );
                    self.proxies.write().await.remove(&session_id);
                    cleanup_session_resources(self, &session_id).await;
                }
            }
        }

        cleanup_session_resources(self, &session_id).await;

        let runtime_servers = loaded.runtime_servers.clone();
        if let Some(load_error) = loaded.config_load_error.clone() {
            self.update_runtime_state(&session_id, app_handle.as_ref(), |state| {
                apply_config_load_failed(
                    state,
                    loaded.use_external_servers,
                    runtime_servers.clone(),
                    load_error.clone(),
                );
            })
            .await;
            return Err(format!(
                "Failed to load MCP server configs for session {}: {}",
                session_id, load_error
            ));
        }

        let config = apply_startup_timeout_settings(self.config.clone()).await;
        let workspace_dir =
            crate::session::resolve_session_workspace_dir(&self.session_manager, &session_id)
                .await?;
        let tool_discovery_timeout = Duration::from_secs(config.process_startup_timeout_seconds);

        let stdio_manager = SessionMCPManager::new(
            session_id.clone(),
            loaded.stdio_configs.clone(),
            config,
            workspace_dir,
        );
        let http_manager = HttpSessionManager::new(session_id.clone(), loaded.http_configs.clone());

        let http_server_names = loaded.http_configs.keys().cloned().collect::<Vec<_>>();
        let mut initial_runtime_state =
            build_bootstrap_runtime_state(loaded.has_external_servers(), runtime_servers);
        apply_loading_tool_config(&mut initial_runtime_state);
        if !http_server_names.is_empty() {
            apply_http_connecting(&mut initial_runtime_state, &http_server_names);
        }
        self.set_runtime_state(&session_id, initial_runtime_state, app_handle.as_ref())
            .await;

        if !loaded.http_configs.is_empty() {
            let http_startup_results =
                start_http_servers_in_parallel(&http_manager, &loaded.http_configs).await;
            commit_runtime_state_update(self, &session_id, app_handle.as_ref(), |state| {
                for startup_result in &http_startup_results {
                    match &startup_result.outcome {
                        Ok(()) => {
                            apply_server_discovering(
                                state,
                                &startup_result.server_name,
                                SessionRuntimeTransport::Http,
                            );
                        }
                        Err(error_message) => {
                            apply_server_failed(
                                state,
                                &startup_result.server_name,
                                SessionRuntimeTransport::Http,
                                error_message.clone(),
                            );
                        }
                    }
                }
            })
            .await;

            for startup_result in http_startup_results {
                if let Err(error_message) = startup_result.outcome {
                    log::error!(
                        "Failed to start HTTP server {} for session {}: {}",
                        startup_result.server_name,
                        session_id,
                        error_message
                    );
                }
            }
        }

        let proxy = MCPServiceProxy::builder(
            session_id.clone(),
            self.db.clone(),
            self.session_manager.clone(),
            Arc::new(http_manager.clone()),
            Arc::new(stdio_manager.clone()),
        )
        .with_tool_ids(tool_ids)
        .with_app_handle(app_handle.clone())
        .build()
        .await?;

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

        commit_runtime_state_update(self, &session_id, app_handle.as_ref(), |state| {
            apply_proxy_created(state, loaded.has_external_servers());
        })
        .await;

        if loaded.has_external_servers() {
            spawn_background_tool_loading(
                self,
                BackgroundDiscoveryPlan {
                    session_id: session_id.clone(),
                    proxy: proxy_arc.clone(),
                    stdio_manager,
                    http_manager,
                    stdio_configs: loaded.stdio_configs,
                    http_configs: loaded.http_configs,
                    server_name_to_id: loaded.server_name_to_id,
                    tool_discovery_timeout,
                    app_handle: app_handle.clone(),
                },
            )
            .await;
        } else {
            self.set_runtime_state(
                &session_id,
                SessionRuntimeState::builtin_ready(),
                app_handle.as_ref(),
            )
            .await;
        }

        log::info!("Created MCP service proxy for session: {}", session_id);
        Ok(proxy_arc)
    }
}
