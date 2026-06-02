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
    SqliteAssistantRepository, SqliteAttachmentsRepository, SqliteCompactContextRepository,
    SqliteKnowledgeRepository, SqliteKnowledgeV2Repository, SqliteMCPServerRepository,
    SqliteMessageRepository, SqlitePlanningRepository, SqlitePlaybookRepository,
    SqliteScheduledTaskRepository, SqliteSessionRepository, SqliteSettingsRepository,
};
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use tauri::AppHandle;
use tokio::sync::{Notify, RwLock as TokioRwLock};

/// A global, thread-safe, once-initialized instance of the `MCPServiceProxyManager`.
static MCP_SERVICE_PROXY_MANAGER: OnceLock<Arc<MCPServiceProxyManager>> = OnceLock::new();

/// A global, thread-safe, once-initialized string for the SQLite database URL.
static SQLITE_DB_URL: OnceLock<String> = OnceLock::new();

/// A global, thread-safe, once-initialized database connection.
static DATABASE_CONNECTION: OnceLock<DatabaseConnection> = OnceLock::new();

/// A global, thread-safe, once-initialized message repository.
static MESSAGE_REPOSITORY: OnceLock<SqliteMessageRepository> = OnceLock::new();

/// A global, thread-safe, once-initialized attachments repository.
static ATTACHMENTS_REPOSITORY: OnceLock<SqliteAttachmentsRepository> = OnceLock::new();

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

/// A global, thread-safe, once-initialized knowledge v2 repository.
static KNOWLEDGE_V2_REPOSITORY: OnceLock<SqliteKnowledgeV2Repository> = OnceLock::new();

/// A global, thread-safe, once-initialized planning repository.
static PLANNING_REPOSITORY: OnceLock<SqlitePlanningRepository> = OnceLock::new();

/// A global, thread-safe, once-initialized scheduled task repository.
static SCHEDULED_TASK_REPOSITORY: OnceLock<SqliteScheduledTaskRepository> = OnceLock::new();

/// A global, thread-safe, once-initialized compact context repository.
static COMPACT_CONTEXT_REPOSITORY: OnceLock<SqliteCompactContextRepository> = OnceLock::new();

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
/// A global revision for skill-directory derived caches.
static SKILLS_CATALOG_REVISION: OnceLock<AtomicU64> = OnceLock::new();
/// Coordinates background startup preparation of managed skills directories.
static MANAGED_SKILLS_SYNC_STATE: OnceLock<ManagedSkillsSyncState> = OnceLock::new();
static STARTUP_TIMER: OnceLock<Instant> = OnceLock::new();

struct ManagedSkillsSyncState {
    ready: AtomicBool,
    notify: Notify,
}

fn skills_catalog_revision() -> &'static AtomicU64 {
    SKILLS_CATALOG_REVISION.get_or_init(|| AtomicU64::new(0))
}

fn managed_skills_sync_state() -> &'static ManagedSkillsSyncState {
    MANAGED_SKILLS_SYNC_STATE.get_or_init(|| ManagedSkillsSyncState {
        ready: AtomicBool::new(true),
        notify: Notify::new(),
    })
}

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
    unsafe {
        force_set(&SQLITE_DB_URL, url);
    }
}

/// Gets a reference to the global SQLite database URL, if it has been set.
///
/// # Returns
/// An `Option` containing a reference to the URL string, or `None` if not yet set.
pub fn get_sqlite_db_url() -> Option<&'static String> {
    SQLITE_DB_URL.get()
}

pub fn start_startup_timer() {
    let _ = STARTUP_TIMER.set(Instant::now());
}

pub fn startup_elapsed_ms() -> Option<u128> {
    STARTUP_TIMER
        .get()
        .map(|started_at| started_at.elapsed().as_millis())
}

pub fn get_skills_catalog_revision() -> u64 {
    skills_catalog_revision().load(Ordering::Relaxed)
}

pub fn invalidate_skills_catalog() -> u64 {
    skills_catalog_revision().fetch_add(1, Ordering::Relaxed) + 1
}

pub fn begin_managed_skills_sync() {
    managed_skills_sync_state()
        .ready
        .store(false, Ordering::Release);
}

pub fn complete_managed_skills_sync() {
    let state = managed_skills_sync_state();
    state.ready.store(true, Ordering::Release);
    state.notify.notify_waiters();
}

pub async fn wait_for_managed_skills_sync() {
    let state = managed_skills_sync_state();

    loop {
        let notified = state.notify.notified();
        if state.ready.load(Ordering::Acquire) {
            break;
        }
        notified.await;
    }
}

/// Sets the global database connection.
///
/// # Panics
/// This function will panic if the connection is already set.
pub fn set_database_connection(db: DatabaseConnection) {
    unsafe {
        force_set(&DATABASE_CONNECTION, db);
    }
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
    unsafe {
        force_set(&MESSAGE_REPOSITORY, repo);
    }
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

/// Sets the global attachments repository instance.
///
/// # Panics
/// This function will panic if the repository is already set.
pub fn set_attachments_repository(repo: SqliteAttachmentsRepository) {
    unsafe {
        force_set(&ATTACHMENTS_REPOSITORY, repo);
    }
}

/// Gets a reference to the global attachments repository.
///
/// # Returns
/// A reference to the attachments repository.
///
/// # Panics
/// Panics if the repository has not been initialized.
pub fn get_attachments_repository() -> &'static SqliteAttachmentsRepository {
    ATTACHMENTS_REPOSITORY
        .get()
        .expect("Attachments repository not initialized. Call set_attachments_repository() first.")
}

/// Sets the global session repository instance.
///
/// # Panics
/// This function will panic if the repository is already set.
pub fn set_session_repository(repo: SqliteSessionRepository) {
    unsafe {
        force_set(&SESSION_REPOSITORY, repo);
    }
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

/// Gets a reference to the global session repository if it has been initialized.
pub fn try_get_session_repository() -> Option<&'static SqliteSessionRepository> {
    SESSION_REPOSITORY.get()
}

/// Sets the global settings repository instance.
pub fn set_settings_repository(repo: SqliteSettingsRepository) {
    unsafe {
        force_set(&SETTINGS_REPOSITORY, repo);
    }
}

/// Gets a reference to the global settings repository.
pub fn get_settings_repository() -> &'static SqliteSettingsRepository {
    SETTINGS_REPOSITORY
        .get()
        .expect("Settings repository not initialized. Call set_settings_repository() first.")
}

/// Gets a reference to the global settings repository if it has been initialized.
pub fn try_get_settings_repository() -> Option<&'static SqliteSettingsRepository> {
    SETTINGS_REPOSITORY.get()
}

/// Sets the global MCP server repository instance.
pub fn set_mcp_server_repository(repo: SqliteMCPServerRepository) {
    unsafe {
        force_set(&MCP_SERVER_REPOSITORY, repo);
    }
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
    unsafe {
        force_set(&MCP_SERVICE_PROXY_MANAGER, manager);
    }
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
    unsafe {
        force_set(&ASSISTANT_REPOSITORY, repo);
    }
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
    unsafe {
        force_set(&PLAYBOOK_REPOSITORY, repo);
    }
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
    unsafe {
        force_set(&KNOWLEDGE_REPOSITORY, repo);
    }
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

/// Sets the global knowledge v2 repository instance.
pub fn set_knowledge_v2_repository(repo: SqliteKnowledgeV2Repository) {
    unsafe {
        force_set(&KNOWLEDGE_V2_REPOSITORY, repo);
    }
}

/// Gets a reference to the global knowledge v2 repository.
pub fn get_knowledge_v2_repository() -> &'static SqliteKnowledgeV2Repository {
    KNOWLEDGE_V2_REPOSITORY.get().expect(
        "Knowledge v2 repository not initialized. Call set_knowledge_v2_repository() first.",
    )
}

/// Sets the global planning repository instance.
///
/// # Panics
/// This function will panic if the repository is already set.
pub fn set_planning_repository(repo: SqlitePlanningRepository) {
    unsafe {
        force_set(&PLANNING_REPOSITORY, repo);
    }
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
    unsafe {
        force_set(&SCHEDULED_TASK_REPOSITORY, repo);
    }
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

/// Sets the global compact context repository instance.
pub fn set_compact_context_repository(repo: SqliteCompactContextRepository) {
    unsafe {
        force_set(&COMPACT_CONTEXT_REPOSITORY, repo);
    }
}

/// Gets a reference to the global compact context repository.
pub fn get_compact_context_repository() -> &'static SqliteCompactContextRepository {
    COMPACT_CONTEXT_REPOSITORY.get().expect(
        "Compact context repository not initialized. Call set_compact_context_repository() first.",
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

/// Return the active sessions map when initialization has already completed.
pub fn try_get_active_sessions() -> Option<&'static Arc<TokioRwLock<HashMap<String, AgentSession>>>>
{
    ACTIVE_SESSIONS.get()
}

/// Retrieve the `cancel_pending` flag Arc for the given session, or `None` if the
/// session is not currently active.  Holds the read-lock only for the duration of
/// the clone — very cheap.  Callers can then poll the AtomicBool without holding
/// any lock, making this safe to use inside hot async loops.
pub async fn get_session_cancel_pending(session_id: &str) -> Option<Arc<AtomicBool>> {
    let sessions = get_active_sessions().read().await;
    sessions.get(session_id).map(|s| s.cancel_pending.clone())
}

unsafe fn reset_lock<T>(lock: &OnceLock<T>) {
    let ptr = lock as *const OnceLock<T> as *mut OnceLock<T>;
    let _ = std::ptr::replace(ptr, OnceLock::new());
}

unsafe fn force_set<T>(lock: &OnceLock<T>, value: T) {
    let ptr = lock as *const OnceLock<T> as *mut OnceLock<T>;
    let new_lock = OnceLock::new();
    let _ = new_lock.set(value);
    let _ = std::ptr::replace(ptr, new_lock);
}

/// Reset all global OnceLock structures. Primarily used for integration testing.
pub fn reset_state() {
    unsafe {
        reset_lock(&MCP_SERVICE_PROXY_MANAGER);
        reset_lock(&SQLITE_DB_URL);
        reset_lock(&DATABASE_CONNECTION);
        reset_lock(&MESSAGE_REPOSITORY);
        reset_lock(&ATTACHMENTS_REPOSITORY);
        reset_lock(&SESSION_REPOSITORY);
        reset_lock(&SETTINGS_REPOSITORY);
        reset_lock(&MCP_SERVER_REPOSITORY);
        reset_lock(&ASSISTANT_REPOSITORY);
        reset_lock(&PLAYBOOK_REPOSITORY);
        reset_lock(&KNOWLEDGE_REPOSITORY);
        reset_lock(&KNOWLEDGE_V2_REPOSITORY);
        reset_lock(&PLANNING_REPOSITORY);
        reset_lock(&SCHEDULED_TASK_REPOSITORY);
        reset_lock(&COMPACT_CONTEXT_REPOSITORY);
        reset_lock(&APP_HANDLE);
        reset_lock(&SESSION_BUS);
        reset_lock(&CONCURRENCY_GATE);
        reset_lock(&ACTIVE_SESSIONS);
        reset_lock(&SKILLS_CATALOG_REVISION);
        reset_lock(&MANAGED_SKILLS_SYNC_STATE);
        reset_lock(&STARTUP_TIMER);
    }
}
