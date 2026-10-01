// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri_mcp_agent_lib::profile::{self, AppProfile};

/// The main entry point for the LibrAgent application.
///
/// This function is responsible for:
/// 1. Loading environment variables from profile-specific `.env*` files
/// 2. Resolving runtime profile (`prod` | `dev` | `demo`) and DB / data paths
/// 3. Optionally resetting demo data for a clean slate
/// 4. Calling `run_with_sqlite_sync` with the SQLite URL
fn main() {
    if std::env::args().any(|arg| arg == tauri_mcp_agent_lib::browser_sidecar::BROWSER_SIDECAR_FLAG)
    {
        if let Err(error) = tauri_mcp_agent_lib::browser_sidecar::run_sidecar_mode() {
            eprintln!("❌ Browser sidecar failed: {error}");
            std::process::exit(1);
        }
        return;
    }

    #[cfg(target_os = "linux")]
    {
        println!("🐧 Linux detected - using default WebKit rendering path");
    }

    load_dotenv_files();

    let profile = profile::resolve_profile();
    println!("📦 App profile: {}", profile.as_str());

    if let Err(err) = profile::maybe_reset_demo_data(profile) {
        eprintln!("❌ {err}");
        std::process::exit(1);
    }

    let db_path = profile::db_path(profile);
    let data_dir = profile::data_dir(profile);

    if let Some(parent_dir) = db_path.parent() {
        std::fs::create_dir_all(parent_dir).expect("Failed to create database directory");
    } else {
        std::fs::create_dir_all(&data_dir).expect("Failed to create data directory");
    }

    let db_url =
        tauri_mcp_agent_lib::utils::sqlite::format_sqlite_url(db_path.to_string_lossy().as_ref());

    match profile {
        AppProfile::Demo => {
            println!(
                "ℹ️  Demo profile: data={} db={}",
                data_dir.display(),
                db_path.display()
            );
        }
        AppProfile::Dev => {
            if std::env::var("LIBRAGENT_DB_PATH").is_err() {
                println!(
                    "ℹ️  Dev profile using isolated DB (set LIBRAGENT_DB_PATH to override): {}",
                    db_path.display()
                );
            }
        }
        AppProfile::Prod => {}
    }

    println!("🚀 Starting LibrAgent with SQLite database: {db_url}");

    tauri_mcp_agent_lib::run_with_sqlite_sync(db_url)
}

fn wants_demo_profile() -> bool {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--demo") {
        return true;
    }
    if let Some(idx) = args.iter().position(|a| a == "--profile") {
        if args
            .get(idx + 1)
            .is_some_and(|v| v.eq_ignore_ascii_case("demo"))
        {
            return true;
        }
    }
    std::env::var("LIBRAGENT_PROFILE")
        .map(|v| v.eq_ignore_ascii_case("demo"))
        .unwrap_or(false)
}

fn try_load_dotenv(path: &std::path::Path) -> bool {
    match dotenvy::from_path(path) {
        Ok(()) => {
            println!("✅ Loaded env file from: {}", path.display());
            true
        }
        Err(dotenvy::Error::Io(err)) if err.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => {
            eprintln!("⚠️  Warning: Failed to load {}: {e}", path.display());
            false
        }
    }
}

/// Load dotenv files. Demo prefers `.env.demo`; debug prefers `.env.dev`.
fn load_dotenv_files() {
    if wants_demo_profile() {
        let candidates = [
            std::path::PathBuf::from(".env.demo"),
            std::path::PathBuf::from("../.env.demo"),
        ];
        for path in &candidates {
            if try_load_dotenv(path) {
                return;
            }
        }
        println!("ℹ️  No .env.demo found (using system environment variables)");
        let _ = dotenvy::dotenv();
        return;
    }

    #[cfg(debug_assertions)]
    {
        match dotenvy::from_filename(".env.dev") {
            Ok(path) => println!("✅ Loaded .env.dev file from: {}", path.display()),
            Err(_) => match dotenvy::dotenv() {
                Ok(path) => println!("✅ Loaded .env file from: {}", path.display()),
                Err(dotenvy::Error::Io(err)) if err.kind() == std::io::ErrorKind::NotFound => {
                    println!(
                        "ℹ️  No .env or .env.dev file found (using system environment variables)"
                    );
                }
                Err(e) => {
                    eprintln!("⚠️  Warning: Failed to load .env file: {e}");
                }
            },
        }
    }

    #[cfg(not(debug_assertions))]
    {
        let loaded = match dotenvy::dotenv() {
            Ok(path) => {
                println!("✅ Loaded .env file from: {}", path.display());
                true
            }
            Err(dotenvy::Error::Io(err)) if err.kind() == std::io::ErrorKind::NotFound => {
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
}
