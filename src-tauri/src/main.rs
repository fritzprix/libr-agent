// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// The main entry point for the LibrAgent application.
///
/// This function is responsible for:
/// 1. Optionally enabling Linux-specific WebKit compatibility mode (if on Linux)
/// 2. Loading environment variables from .env file (development mode only)
/// 3. Determining the path for the SQLite database. It prioritizes the `LIBRAGENT_DB_PATH`
///    environment variable, falling back to a default location within the user's data directory.
/// 4. Ensuring the directory for the database exists.
/// 5. Constructing the final SQLite connection URL.
/// 6. Calling the main application runner (`run_with_sqlite_sync`) from the `tauri_mcp_agent_lib`
///    crate, passing it the database URL to initialize the application with database support.
#[cfg(target_os = "linux")]
fn linux_compatibility_mode_enabled() -> bool {
    matches!(
        std::env::var("LIBRAGENT_LINUX_COMPATIBILITY_MODE"),
        Ok(value)
            if matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
    )
}

fn main() {
    // Set Linux-specific WebKit compatibility environment variables FIRST when explicitly enabled.
    // This must happen before any WebView initialization.
    #[cfg(target_os = "linux")]
    {
        if linux_compatibility_mode_enabled() {
            println!("🐧 Linux compatibility mode enabled - applying WebKit fallback flags...");

            // Disable WebKit compositing and force software rendering for problematic Linux setups.
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
            std::env::set_var("GDK_BACKEND", "x11");
            std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");

            println!("✅ Linux compatibility mode active (software rendering + X11 fallback)");
        } else {
            println!("🐧 Linux detected - using default WebKit rendering path");
            println!(
                "ℹ️  Set LIBRAGENT_LINUX_COMPATIBILITY_MODE=1 only if you hit blank screens or driver issues"
            );
        }
    }

    // Load environment variables from .env file
    // Development: loads .env.dev (if exists) or .env from current directory
    // Production: loads .env from executable directory or current directory
    #[cfg(debug_assertions)]
    {
        // Try .env.dev first in development, fallback to .env
        match dotenvy::from_filename(".env.dev") {
            Ok(path) => println!("✅ Loaded .env.dev file from: {}", path.display()),
            Err(_) => {
                // Fallback to .env
                match dotenvy::dotenv() {
                    Ok(path) => println!("✅ Loaded .env file from: {}", path.display()),
                    Err(dotenvy::Error::Io(err)) if err.kind() == std::io::ErrorKind::NotFound => {
                        println!("ℹ️  No .env or .env.dev file found (using system environment variables)");
                    }
                    Err(e) => {
                        eprintln!("⚠️  Warning: Failed to load .env file: {e}");
                    }
                }
            }
        }
    }

    #[cfg(not(debug_assertions))]
    {
        // Production: Try multiple .env locations for better compatibility
        // 1. Current working directory (when run from project root)
        // 2. Executable directory (when installed/distributed)
        let loaded = match dotenvy::dotenv() {
            Ok(path) => {
                println!("✅ Loaded .env file from: {}", path.display());
                true
            }
            Err(dotenvy::Error::Io(err)) if err.kind() == std::io::ErrorKind::NotFound => {
                // Try loading from executable directory
                if let Ok(exe_path) = std::env::current_exe() {
                    if let Some(exe_dir) = exe_path.parent() {
                        let env_path = exe_dir.join(".env");
                        match dotenvy::from_path(&env_path) {
                            Ok(_) => {
                                println!("✅ Loaded .env file from: {}", env_path.display());
                                true
                            }
                            Err(_) => false,
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            Err(e) => {
                eprintln!("⚠️  Warning: Failed to load .env file: {e}");
                false
            }
        };

        if !loaded {
            println!("ℹ️  No .env file found (using system environment variables and defaults)");
        }
    }

    // Set the SQLite database path - stored in the user's data directory.
    let db_path = std::env::var("LIBRAGENT_DB_PATH").unwrap_or_else(|_| {
        // Default to storing in the user's data directory.
        let data_dir = dirs::data_dir()
            .expect("Failed to get data directory")
            .join("com.fritzprix.libragent");
        // Use a different filename to avoid potential locking issues.
        data_dir
            .join("libragent_v2.db")
            .to_string_lossy()
            .to_string()
    });

    // Check if the database directory exists and create it if it doesn't.
    if let Some(parent_dir) = std::path::Path::new(&db_path).parent() {
        std::fs::create_dir_all(parent_dir).expect("Failed to create database directory");
    }

    let db_url = tauri_mcp_agent_lib::utils::sqlite::format_sqlite_url(&db_path);

    println!("🚀 Starting LibrAgent with SQLite database: {db_url}");

    tauri_mcp_agent_lib::run_with_sqlite_sync(db_url)
}
