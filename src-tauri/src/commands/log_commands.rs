/// Log file management commands
///
/// This module contains commands for managing application log files,
/// including backup, clearing, and listing log files.
use crate::commands::workspace_commands::get_app_logs_dir;
use chrono::Utc;
use std::fs;

/// Creates a timestamped backup of the current main log file.
///
/// # Returns
/// A `Result` containing the path of the created backup file, or an error string.
#[tauri::command]
pub async fn backup_current_log() -> Result<String, String> {
    let log_dir_str = get_app_logs_dir().await?;
    let log_dir = std::path::PathBuf::from(log_dir_str);

    // Find the current log file (using the specified filename)
    let log_file = log_dir.join("synaptic-flow.log");

    if !log_file.exists() {
        return Err("No current log file found".to_string());
    }

    // Create backup filename (including timestamp)
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_file = log_dir.join(format!("synaptic-flow_{timestamp}.log.bak"));

    // Copy the file
    fs::copy(&log_file, &backup_file).map_err(|e| format!("Failed to backup log file: {e}"))?;

    Ok(backup_file.to_string_lossy().to_string())
}

/// Clears the content of the current main log file.
#[tauri::command]
pub async fn clear_current_log() -> Result<(), String> {
    let log_dir_str = get_app_logs_dir().await?;
    let log_dir = std::path::PathBuf::from(log_dir_str);
    let log_file = log_dir.join("synaptic-flow.log");

    if log_file.exists() {
        fs::write(&log_file, "").map_err(|e| format!("Failed to clear log file: {e}"))?;
    }

    Ok(())
}

/// Lists all log files (`.log`) and log backups (`.log.bak`) in the log directory.
#[tauri::command]
pub async fn list_log_files() -> Result<Vec<String>, String> {
    let log_dir_str = get_app_logs_dir().await?;
    let log_dir = std::path::PathBuf::from(log_dir_str);

    if !log_dir.exists() {
        return Ok(vec![]);
    }

    let entries =
        fs::read_dir(&log_dir).map_err(|e| format!("Failed to read log directory: {e}"))?;

    let mut log_files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
        let path = entry.path();

        if path.is_file() {
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if filename.ends_with(".log") || filename.ends_with(".log.bak") {
                    log_files.push(filename.to_string());
                }
            }
        }
    }

    log_files.sort();
    Ok(log_files)
}
