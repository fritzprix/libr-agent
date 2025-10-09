/// Global state management module
///
/// This module provides centralized access to application-wide state including
/// the MCP server manager, SQLite database URL, and SQLite connection pool.
use crate::mcp::MCPServerManager;
use sqlx::sqlite::SqlitePool;
use std::sync::OnceLock;

/// A global, thread-safe, once-initialized instance of the `MCPServerManager`.
static MCP_MANAGER: OnceLock<MCPServerManager> = OnceLock::new();

/// A global, thread-safe, once-initialized string for the SQLite database URL.
static SQLITE_DB_URL: OnceLock<String> = OnceLock::new();

/// A global, thread-safe, once-initialized SQLite connection pool.
static SQLITE_POOL: OnceLock<SqlitePool> = OnceLock::new();

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

/// Sets the global MCP server manager instance.
///
/// # Panics
/// This function will panic if the manager is already set.
pub fn set_mcp_manager(manager: MCPServerManager) {
    MCP_MANAGER
        .set(manager)
        .expect("MCP Manager already initialized");
}

/// Gets a static reference to the global `MCPServerManager`.
///
/// If the manager has not been initialized, it initializes it with the default
/// `SessionManager`. This function will panic if the `SessionManager` itself is not initialized.
///
/// # Panics
/// Panics if `SessionManager` is not initialized when lazy initialization is attempted.
pub fn get_mcp_manager() -> &'static MCPServerManager {
    MCP_MANAGER.get_or_init(|| {
        let session_manager =
            crate::session::get_session_manager().expect("SessionManager not initialized");
        let session_manager_arc = std::sync::Arc::new(session_manager.clone());
        MCPServerManager::new_with_session_manager(session_manager_arc)
    })
}

/// Sets the global SQLite connection pool.
///
/// # Panics
/// This function will panic if the pool is already set.
pub fn set_sqlite_pool(pool: SqlitePool) {
    SQLITE_POOL
        .set(pool)
        .expect("SQLite pool already initialized");
}

/// Gets a reference to the global SQLite connection pool.
///
/// # Returns
/// A reference to the SQLite connection pool.
///
/// # Panics
/// Panics if the pool has not been initialized.
pub fn get_sqlite_pool() -> &'static SqlitePool {
    SQLITE_POOL
        .get()
        .expect("SQLite pool not initialized. Call set_sqlite_pool() first.")
}
