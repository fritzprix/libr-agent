/// Log file management commands and logging from frontend
///
/// This module contains commands for managing application log files,
/// including backup, clearing, listing, and forwarding logs from TypeScript.
use crate::logger;
use crate::mcp::builtin::workspace::persistent_shell::secrets::mask_log_line;
use crate::services::LogService;
use log::{debug, error as log_error, info, trace, warn};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
    pub timestamp: i64,
    /// The frontend's length limit, applied here after masking: a cut made before
    /// masking could keep part of a stored value that no whole-value pattern matches.
    #[serde(default, rename = "maxMessageLength")]
    pub max_message_length: Option<usize>,
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

/// Returns the effective launch-time log level after applying CLI overrides and defaults.
#[tauri::command]
pub fn get_launch_log_level() -> String {
    logger::resolve_launch_log_level_name().to_string()
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
        let message = masked_and_truncated(&entry.message, entry.max_message_length);
        match entry.level.as_str() {
            "trace" => trace!("[webview] {}", message),
            "debug" => debug!("[webview] {}", message),
            "info" => info!("[webview] {}", message),
            "warn" => warn!("[webview] {}", message),
            "error" => log_error!("[webview] {}", message),
            _ => info!("[webview] [UNKNOWN:{}] {}", entry.level, message),
        }
    }
}

/// Masks stored values, then cuts the message to `max_chars` characters ending in `...`.
fn masked_and_truncated(message: &str, max_chars: Option<usize>) -> String {
    let masked = mask_log_line(message);
    match max_chars {
        Some(max) if masked.chars().count() > max => {
            let mut cut: String = masked.chars().take(max.saturating_sub(3)).collect();
            cut.push_str("...");
            cut
        }
        _ => masked.into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::builtin::workspace::persistent_shell::PersistentShellManager;

    #[tokio::test]
    async fn a_stored_value_straddling_the_length_limit_is_masked_before_the_cut() {
        let stored = "trunc-stored-8e2f1a";
        let manager = PersistentShellManager::new();
        manager
            .store_secret("log-truncation-session", "DEMO_TOKEN", stored.to_string())
            .await
            .expect("store secret");

        let max = 60;
        let message = format!("{}{stored} {}", "x".repeat(max - 3 - 6), "y".repeat(20));
        let cut_before_masking = format!("{}...", &message[..max - 3]);
        assert!(cut_before_masking.contains(&stored[..6]));

        let logged = masked_and_truncated(&message, Some(max));
        let short = masked_and_truncated("short trunc-stored-8e2f1a", Some(max));
        let uncut = masked_and_truncated(&message, None);
        manager.clear_session_state("log-truncation-session").await;

        assert!(!logged.contains(&stored[..4]), "{logged}");
        assert!(logged.ends_with("..."), "{logged}");
        assert_eq!(logged.chars().count(), max);
        assert_eq!(short, "short ***");
        assert_eq!(uncut, message.replace(stored, "***"));
    }
}
