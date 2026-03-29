/// Custom logger setup to ensure Rust logs are written to file
use std::fs::{self, OpenOptions};
use std::path::PathBuf;

extern crate fern;

fn parse_log_level_arg(value: &str) -> Option<log::LevelFilter> {
    match value.trim().to_ascii_lowercase().as_str() {
        "trace" => Some(log::LevelFilter::Trace),
        "debug" => Some(log::LevelFilter::Debug),
        "info" => Some(log::LevelFilter::Info),
        "warn" | "warning" => Some(log::LevelFilter::Warn),
        "error" => Some(log::LevelFilter::Error),
        _ => None,
    }
}

pub fn resolve_launch_log_level() -> log::LevelFilter {
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        if let Some(value) = arg.strip_prefix("--log-level=") {
            if let Some(level) = parse_log_level_arg(value) {
                return level;
            }
            continue;
        }

        if arg == "--log-level" {
            if let Some(value) = args.next() {
                if let Some(level) = parse_log_level_arg(&value) {
                    return level;
                }
            }
        }
    }

    if cfg!(debug_assertions) {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    }
}

pub fn resolve_launch_log_level_name() -> &'static str {
    match resolve_launch_log_level() {
        log::LevelFilter::Trace => "trace",
        log::LevelFilter::Debug => "debug",
        log::LevelFilter::Info => "info",
        log::LevelFilter::Warn => "warn",
        log::LevelFilter::Error => "error",
        _ => "info",
    }
}

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

    let log_level = resolve_launch_log_level();

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
        .level(log_level)
        .chain(std::io::stdout()) // Keep stdout
        .chain(file) // Add file output
        .apply()
        .map_err(|e| format!("Failed to set logger: {}", e))?;

    log::info!(
        "🔥 File logger initialized at: {} (level: {})",
        log_file.display(),
        resolve_launch_log_level_name()
    );
    Ok(())
}
