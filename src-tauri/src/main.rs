// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// The main entry point for the SynapticFlow application.
///
/// This function is responsible for:
/// 1. Loading environment variables from .env file (development mode only)
/// 2. Determining the path for the SQLite database. It prioritizes the `SYNAPTICFLOW_DB_PATH`
///    environment variable, falling back to a default location within the user's data directory.
/// 3. Ensuring the directory for the database exists.
/// 4. Constructing the final SQLite connection URL.
/// 5. Calling the main application runner (`run_with_sqlite_sync`) from the `tauri_mcp_agent_lib`
///    crate, passing it the database URL to initialize the application with database support.
fn main() {
    // Load environment variables from .env file (development mode only)
    // In production, use system environment variables instead
    #[cfg(debug_assertions)]
    {
        match dotenvy::dotenv() {
            Ok(path) => println!("✅ Loaded .env file from: {}", path.display()),
            Err(dotenvy::Error::Io(err)) if err.kind() == std::io::ErrorKind::NotFound => {
                println!("ℹ️  No .env file found (this is OK, using system environment variables)");
            }
            Err(e) => {
                eprintln!("⚠️  Warning: Failed to load .env file: {e}");
            }
        }
    }

    // Set the SQLite database path - stored in the user's data directory.
    let db_path = std::env::var("SYNAPTICFLOW_DB_PATH").unwrap_or_else(|_| {
        // Default to storing in the user's data directory.
        let data_dir = dirs::data_dir()
            .expect("Failed to get data directory")
            .join("com.fritzprix.synapticflow");
        // Use a different filename to avoid potential locking issues.
        data_dir
            .join("synapticflow_v2.db")
            .to_string_lossy()
            .to_string()
    });

    // Check if the database directory exists and create it if it doesn't.
    if let Some(parent_dir) = std::path::Path::new(&db_path).parent() {
        std::fs::create_dir_all(parent_dir).expect("Failed to create database directory");
    }

    let db_url = format!("sqlite://{db_path}");

    println!("🚀 Starting SynapticFlow with SQLite database: {db_url}");

    tauri_mcp_agent_lib::run_with_sqlite_sync(db_url)
}
