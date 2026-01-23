/// Global state management module
///
/// This module provides centralized access to application-wide state including
/// the MCP service proxy manager, SQLite database URL,
/// database connection, and repositories.
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::{
    SqliteContentStoreRepository, SqliteMCPServerRepository, SqliteMessageRepository,
    SqliteSessionRepository, SqliteSettingsRepository,
};
use sea_orm::DatabaseConnection;
use std::sync::OnceLock;

/// A global, thread-safe, once-initialized instance of the `MCPServiceProxyManager`.
static MCP_SERVICE_PROXY_MANAGER: OnceLock<MCPServiceProxyManager> = OnceLock::new();

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
pub fn set_mcp_service_proxy_manager(manager: MCPServiceProxyManager) {
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
pub fn get_mcp_service_proxy_manager() -> &'static MCPServiceProxyManager {
    MCP_SERVICE_PROXY_MANAGER.get().expect(
        "MCP Service Proxy Manager not initialized. Call set_mcp_service_proxy_manager() first.",
    )
}
