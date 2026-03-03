/// Global state management module
///
/// This module provides centralized access to application-wide state including
/// the MCP service proxy manager, SQLite database URL,
/// database connection, and repositories.
use crate::agent::concurrency::ConcurrencyGate;
use crate::agent::session_bus::SessionBus;
use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::{
    SqliteAssistantRepository, SqliteContentStoreRepository, SqliteKnowledgeRepository,
    SqliteMCPServerRepository, SqliteMessageRepository, SqlitePlanningRepository,
    SqlitePlaybookRepository, SqliteScheduledTaskRepository, SqliteSessionRepository,
    SqliteSettingsRepository,
};
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, OnceLock};
use tauri::AppHandle;
use tokio::sync::RwLock as TokioRwLock;

/// A global, thread-safe, once-initialized instance of the `MCPServiceProxyManager`.
static MCP_SERVICE_PROXY_MANAGER: OnceLock<Arc<MCPServiceProxyManager>> = OnceLock::new();

/// A global, thread-safe, once-initialized string for the SQLite database URL.
static SQLITE_DB_URL: OnceLock<String> = OnceLock::new();

/// A global, thread-safe, once-initialized database connection.
static DATABASE_CONNECTION: OnceLock<DatabaseConnection> = OnceLock::new();

/// A global, thread-safe, once-initialized message repository.
static MESSAGE_REPOSITORY: OnceLock<SqliteMessageRepository> = OnceLock::new();

/// A global, thread-safe, once-initialized content store repository.
static CONTENT_STORE_REPOSITORY: OnceLock<SqliteContentStoreRepository> = OnceLock::new();

/// A global, thread-safe, once-initialized session repository.
static SESSION_REPOSITORY: OnceLock<SqliteSessionRepository> = OnceLock::new();

/// A global, thread-safe, once-initialized settings repository.
static SETTINGS_REPOSITORY: OnceLock<SqliteSettingsRepository> = OnceLock::new();

/// A global, thread-safe, once-initialized MCP server repository.
static MCP_SERVER_REPOSITORY: OnceLock<SqliteMCPServerRepository> = OnceLock::new();

/// A global, thread-safe, once-initialized assistant repository.
static ASSISTANT_REPOSITORY: OnceLock<SqliteAssistantRepository> = OnceLock::new();

/// A global, thread-safe, once-initialized playbook repository.
static PLAYBOOK_REPOSITORY: OnceLock<SqlitePlaybookRepository> = OnceLock::new();

/// A global, thread-safe, once-initialized knowledge repository.
static KNOWLEDGE_REPOSITORY: OnceLock<SqliteKnowledgeRepository> = OnceLock::new();

/// A global, thread-safe, once-initialized planning repository.
static PLANNING_REPOSITORY: OnceLock<SqlitePlanningRepository> = OnceLock::new();

/// A global, thread-safe, once-initialized scheduled task repository.
static SCHEDULED_TASK_REPOSITORY: OnceLock<SqliteScheduledTaskRepository> = OnceLock::new();

/// A global, thread-safe, once-initialized Tauri AppHandle for event emission.
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// A global, thread-safe, once-initialized session event bus (SP1).
static SESSION_BUS: OnceLock<SessionBus> = OnceLock::new();

/// A global, thread-safe, once-initialized concurrency gate (SP2).
static CONCURRENCY_GATE: OnceLock<ConcurrencyGate> = OnceLock::new();

/// A global, thread-safe, once-initialized active sessions map (SP6).
/// Shared Arc from AgentSessionManager so external subsystems (e.g. builtin MCP tools)
/// can read per-session cancellation tokens without going through Tauri managed state.
static ACTIVE_SESSIONS: OnceLock<Arc<TokioRwLock<HashMap<String, AgentSession>>>> = OnceLock::new();

/// Initialize the global AppHandle
/// Should be called once during application setup
pub fn init_app_handle(handle: AppHandle) {
    if APP_HANDLE.set(handle).is_err() {
        log::warn!("AppHandle already initialized");
    }
}

/// Get the global AppHandle for event emission
/// Returns None if not initialized yet
pub fn get_app_handle() -> Option<&'static AppHandle> {
    APP_HANDLE.get()
}

/// Sets the global SQLite database URL.
///
/// # Panics
/// This function will panic if the URL is already set.
pub fn set_sqlite_db_url(url: String) {
    SQLITE_DB_URL.set(url).expect("SQLite DB URL already set");
}

/// Gets a reference to the global SQLite database URL, if it has been set.
///
/// # Returns
/// An `Option` containing a reference to the URL string, or `None` if not yet set.
pub fn get_sqlite_db_url() -> Option<&'static String> {
    SQLITE_DB_URL.get()
}

/// Sets the global database connection.
///
/// # Panics
/// This function will panic if the connection is already set.
pub fn set_database_connection(db: DatabaseConnection) {
    DATABASE_CONNECTION
        .set(db)
        .expect("Database connection already initialized");
}

/// Gets a reference to the global database connection.
///
/// # Returns
/// A reference to the database connection.
///
/// # Panics
/// Panics if the connection has not been initialized.
pub fn get_database_connection() -> &'static DatabaseConnection {
    DATABASE_CONNECTION
        .get()
        .expect("Database connection not initialized. Call set_database_connection() first.")
}

/// Sets the global message repository instance.
///
/// # Panics
/// This function will panic if the repository is already set.
pub fn set_message_repository(repo: SqliteMessageRepository) {
    MESSAGE_REPOSITORY
        .set(repo)
        .expect("Message repository already initialized");
}

/// Gets a reference to the global message repository.
///
/// # Returns
/// A reference to the message repository.
///
/// # Panics
/// Panics if the repository has not been initialized.
pub fn get_message_repository() -> &'static SqliteMessageRepository {
    MESSAGE_REPOSITORY
        .get()
        .expect("Message repository not initialized. Call set_message_repository() first.")
}

/// Sets the global content store repository instance.
///
/// # Panics
/// This function will panic if the repository is already set.
pub fn set_content_store_repository(repo: SqliteContentStoreRepository) {
    CONTENT_STORE_REPOSITORY
        .set(repo)
        .expect("Content store repository already initialized");
}

/// Gets a reference to the global content store repository.
///
/// # Returns
/// A reference to the content store repository.
///
/// # Panics
/// Panics if the repository has not been initialized.
pub fn get_content_store_repository() -> &'static SqliteContentStoreRepository {
    CONTENT_STORE_REPOSITORY.get().expect(
        "Content store repository not initialized. Call set_content_store_repository() first.",
    )
}

/// Sets the global session repository instance.
///
/// # Panics
/// This function will panic if the repository is already set.
pub fn set_session_repository(repo: SqliteSessionRepository) {
    SESSION_REPOSITORY
        .set(repo)
        .expect("Session repository already initialized");
}

/// Gets a reference to the global session repository.
///
/// # Returns
/// A reference to the session repository.
///
/// # Panics
/// Panics if the repository has not been initialized.
pub fn get_session_repository() -> &'static SqliteSessionRepository {
    SESSION_REPOSITORY
        .get()
        .expect("Session repository not initialized. Call set_session_repository() first.")
}

/// Sets the global settings repository instance.
pub fn set_settings_repository(repo: SqliteSettingsRepository) {
    SETTINGS_REPOSITORY
        .set(repo)
        .expect("Settings repository already initialized");
}

/// Gets a reference to the global settings repository.
pub fn get_settings_repository() -> &'static SqliteSettingsRepository {
    SETTINGS_REPOSITORY
        .get()
        .expect("Settings repository not initialized. Call set_settings_repository() first.")
}

/// Sets the global MCP server repository instance.
pub fn set_mcp_server_repository(repo: SqliteMCPServerRepository) {
    MCP_SERVER_REPOSITORY
        .set(repo)
        .expect("MCP server repository already initialized");
}

/// Gets a reference to the global MCP server repository.
pub fn get_mcp_server_repository() -> &'static SqliteMCPServerRepository {
    MCP_SERVER_REPOSITORY
        .get()
        .expect("MCP server repository not initialized. Call set_mcp_server_repository() first.")
}

/// Sets the global MCP service proxy manager instance.
///
/// # Panics
/// This function will panic if the manager is already set.
pub fn set_mcp_service_proxy_manager(manager: Arc<MCPServiceProxyManager>) {
    MCP_SERVICE_PROXY_MANAGER
        .set(manager)
        .expect("MCP Service Proxy Manager already initialized");
}

/// Gets a reference to the global MCP service proxy manager.
///
/// # Returns
/// A reference to the MCP service proxy manager.
///
/// # Panics
/// Panics if the manager has not been initialized.
pub fn get_mcp_service_proxy_manager() -> Arc<MCPServiceProxyManager> {
    MCP_SERVICE_PROXY_MANAGER
        .get()
        .expect(
            "MCP Service Proxy Manager not initialized. Call set_mcp_service_proxy_manager() first.",
        )
        .clone()
}

/// Sets the global assistant repository instance.
///
/// # Panics
/// This function will panic if the repository is already set.
pub fn set_assistant_repository(repo: SqliteAssistantRepository) {
    ASSISTANT_REPOSITORY
        .set(repo)
        .expect("Assistant repository already initialized");
}

/// Gets a reference to the global assistant repository.
///
/// # Returns
/// A reference to the assistant repository.
///
/// # Panics
/// Panics if the repository has not been initialized.
pub fn get_assistant_repository() -> &'static SqliteAssistantRepository {
    ASSISTANT_REPOSITORY
        .get()
        .expect("Assistant repository not initialized. Call set_assistant_repository() first.")
}

/// Sets the global playbook repository instance.
///
/// # Panics
/// This function will panic if the repository is already set.
pub fn set_playbook_repository(repo: SqlitePlaybookRepository) {
    PLAYBOOK_REPOSITORY
        .set(repo)
        .expect("Playbook repository already initialized");
}

/// Gets a reference to the global playbook repository.
///
/// # Returns
/// A reference to the playbook repository.
///
/// # Panics
/// Panics if the repository has not been initialized.
pub fn get_playbook_repository() -> &'static SqlitePlaybookRepository {
    PLAYBOOK_REPOSITORY
        .get()
        .expect("Playbook repository not initialized. Call set_playbook_repository() first.")
}

/// Sets the global knowledge repository instance.
///
/// # Panics
/// This function will panic if the repository is already set.
pub fn set_knowledge_repository(repo: SqliteKnowledgeRepository) {
    KNOWLEDGE_REPOSITORY
        .set(repo)
        .expect("Knowledge repository already initialized");
}

/// Gets a reference to the global knowledge repository.
///
/// # Returns
/// A reference to the knowledge repository.
///
/// # Panics
/// Panics if the repository has not been initialized.
pub fn get_knowledge_repository() -> &'static SqliteKnowledgeRepository {
    KNOWLEDGE_REPOSITORY
        .get()
        .expect("Knowledge repository not initialized. Call set_knowledge_repository() first.")
}

/// Sets the global planning repository instance.
///
/// # Panics
/// This function will panic if the repository is already set.
pub fn set_planning_repository(repo: SqlitePlanningRepository) {
    PLANNING_REPOSITORY
        .set(repo)
        .expect("Planning repository already initialized");
}

/// Gets a reference to the global planning repository.
///
/// # Returns
/// A reference to the planning repository.
///
/// # Panics
/// Panics if the repository has not been initialized.
pub fn get_planning_repository() -> &'static SqlitePlanningRepository {
    PLANNING_REPOSITORY
        .get()
        .expect("Planning repository not initialized. Call set_planning_repository() first.")
}

/// Sets the global scheduled task repository instance.
///
/// # Panics
/// This function will panic if the repository is already set.
pub fn set_scheduled_task_repository(repo: SqliteScheduledTaskRepository) {
    SCHEDULED_TASK_REPOSITORY
        .set(repo)
        .expect("Scheduled task repository already initialized");
}

/// Gets a reference to the global scheduled task repository.
///
/// # Panics
/// Panics if the repository has not been initialized.
pub fn get_scheduled_task_repository() -> &'static SqliteScheduledTaskRepository {
    SCHEDULED_TASK_REPOSITORY.get().expect(
        "Scheduled task repository not initialized. Call set_scheduled_task_repository() first.",
    )
}

// ── SP1: SessionBus ───────────────────────────────────────────────────────────

/// Initialize the global `SessionBus`.  Called once during application setup.
///
/// # Panics
/// Panics if the bus has already been initialized.
pub fn init_session_bus(bus: SessionBus) {
    if SESSION_BUS.set(bus).is_err() {
        log::warn!("SessionBus already initialized");
    }
}

/// Get a reference to the global `SessionBus`.
///
/// # Panics
/// Panics if `init_session_bus` has not been called yet.
pub fn get_session_bus() -> &'static SessionBus {
    SESSION_BUS
        .get()
        .expect("SessionBus not initialized. Call init_session_bus() first.")
}

// ── SP2: ConcurrencyGate ──────────────────────────────────────────────────────

/// Initialize the global `ConcurrencyGate`.  Called once during application setup
/// with values read from the user's advanced settings.
///
/// # Panics
/// Panics if the gate has already been initialized.
pub fn init_concurrency_gate(gate: ConcurrencyGate) {
    if CONCURRENCY_GATE.set(gate).is_err() {
        log::warn!("ConcurrencyGate already initialized");
    }
}

/// Get a reference to the global `ConcurrencyGate`.
///
/// # Panics
/// Panics if `init_concurrency_gate` has not been called yet.
pub fn get_concurrency_gate() -> &'static ConcurrencyGate {
    CONCURRENCY_GATE
        .get()
        .expect("ConcurrencyGate not initialized. Call init_concurrency_gate() first.")
}

// ── SP6: Active Sessions (for cancellation token access) ─────────────────────

/// Store a shared Arc to the active sessions map so that builtin MCP tools
/// can look up per-session cancellation tokens without Tauri managed-state access.
/// Must be called once during application setup, immediately after the
/// `AgentSessionManager` is created.
pub fn init_active_sessions(sessions: Arc<TokioRwLock<HashMap<String, AgentSession>>>) {
    if ACTIVE_SESSIONS.set(sessions).is_err() {
        log::warn!("ACTIVE_SESSIONS already initialized");
    }
}

/// Return a reference to the global active sessions map.
///
/// # Panics
/// Panics if `init_active_sessions` has not been called yet.
pub fn get_active_sessions() -> &'static Arc<TokioRwLock<HashMap<String, AgentSession>>> {
    ACTIVE_SESSIONS
        .get()
        .expect("ACTIVE_SESSIONS not initialized. Call init_active_sessions() first.")
}

/// Retrieve the `cancel_pending` flag Arc for the given session, or `None` if the
/// session is not currently active.  Holds the read-lock only for the duration of
/// the clone — very cheap.  Callers can then poll the AtomicBool without holding
/// any lock, making this safe to use inside hot async loops.
pub async fn get_session_cancel_pending(session_id: &str) -> Option<Arc<AtomicBool>> {
    let sessions = get_active_sessions().read().await;
    sessions.get(session_id).map(|s| s.cancel_pending.clone())
}
