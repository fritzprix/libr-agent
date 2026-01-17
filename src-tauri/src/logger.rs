/// Custom logger setup to ensure Rust logs are written to file
use chrono;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

extern crate fern;

pub fn setup_file_logger(log_dir: PathBuf) -> Result<(), String> {
    // Ensure log directory exists
    if !log_dir.exists() {
        fs::create_dir_all(&log_dir).map_err(|e| format!("Failed to create log directory: {}", e))?;
    }

    let log_file = log_dir.join("libragent.log");
    
    // Open file in append mode
    let file = OpenOptions::new()
        .create(true)
        .append(true)
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
        .chain(std::io::stdout())  // Keep stdout
        .chain(file)               // Add file output
        .apply()
        .map_err(|e| format!("Failed to set logger: {}", e))?;

    log::info!("🔥 File logger initialized at: {}", log_file.display());
    Ok(())
}
