/// Log file management commands and logging from frontend
///
/// This module contains commands for managing application log files,
/// including backup, clearing, listing, and forwarding logs from TypeScript.
use crate::services::LogService;
use log::{debug, error as log_error, info, trace, warn};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
    pub timestamp: i64,
}

/// Creates a timestamped backup of the current main log file.
///
/// # Returns
/// A `Result` containing the path of the created backup file, or an error string.
#[tauri::command]
pub async fn backup_current_log() -> Result<String, String> {
    LogService::backup_current_log().await
}

/// Clears the content of the current main log file.
#[tauri::command]
pub async fn clear_current_log() -> Result<(), String> {
    LogService::clear_current_log().await
}

/// Lists all log files (`.log`) and log backups (`.log.bak`) in the log directory.
#[tauri::command]
pub async fn list_log_files() -> Result<Vec<String>, String> {
    LogService::list_log_files().await
}

/// Forward trace log from TypeScript to Rust logger
#[tauri::command]
pub fn log_trace(message: String) {
    trace!("[webview] {}", message);
}

/// Forward debug log from TypeScript to Rust logger
#[tauri::command]
pub fn log_debug(message: String) {
    debug!("[webview] {}", message);
}

/// Forward info log from TypeScript to Rust logger
#[tauri::command]
pub fn log_info(message: String) {
    info!("[webview] {}", message);
}

/// Forward warn log from TypeScript to Rust logger
#[tauri::command]
pub fn log_warn(message: String) {
    warn!("[webview] {}", message);
}

/// Forward error log from TypeScript to Rust logger
#[tauri::command]
pub fn log_error_from_frontend(message: String) {
    log_error!("[webview] {}", message);
}

/// Process a batch of log entries from the frontend
#[tauri::command]
pub fn log_batch(entries: Vec<LogEntry>) {
    for entry in entries {
        match entry.level.as_str() {
            "trace" => trace!("[webview] {}", entry.message),
            "debug" => debug!("[webview] {}", entry.message),
            "info" => info!("[webview] {}", entry.message),
            "warn" => warn!("[webview] {}", entry.message),
            "error" => log_error!("[webview] {}", entry.message),
            _ => info!("[webview] [UNKNOWN:{}] {}", entry.level, entry.message),
        }
    }
}
