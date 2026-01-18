/// Custom logger setup to ensure Rust logs are written to file
use std::fs::{self, OpenOptions};
use std::path::PathBuf;

extern crate fern;

pub fn setup_file_logger(log_dir: PathBuf) -> Result<(), String> {
    // Ensure log directory exists
    if !log_dir.exists() {
        fs::create_dir_all(&log_dir)
            .map_err(|e| format!("Failed to create log directory: {}", e))?;
    }

    let log_file = log_dir.join("libragent.log");

    // Backup existing log file with datetime suffix
    if log_file.exists() {
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let backup_file = log_dir.join(format!("libragent_{}.log", timestamp));
        fs::rename(&log_file, &backup_file)
            .map_err(|e| format!("Failed to backup log file: {}", e))?;
        log::info!("📦 Backed up previous log to: {}", backup_file.display());
    }

    // Create new log file (truncate mode for fresh start)
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_file)
        .map_err(|e| format!("Failed to open log file: {}", e))?;

    // Configure fern logger
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}][{}][{}] {}",
                chrono::Local::now().format("%Y-%m-%d][%H:%M:%S"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(std::io::stdout()) // Keep stdout
        .chain(file) // Add file output
        .apply()
        .map_err(|e| format!("Failed to set logger: {}", e))?;

    log::info!("🔥 File logger initialized at: {}", log_file.display());
    Ok(())
}
