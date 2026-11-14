// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// The main entry point for the LibrAgent application.
///
/// This function is responsible for:
/// 1. Dispatching `--tui` to the TUI entry (#2761)
/// 2. Loading environment variables from profile-specific `.env*` files
/// 3. Resolving runtime profile (`prod` | `dev` | `demo`) and DB / data paths
/// 4. Optionally resetting demo data for a clean slate
/// 5. Calling `run_with_sqlite_sync` with the SQLite URL
fn main() {
    // First thing, while still single-threaded: take the injected Session API key out of the
    // environment so no child process can inherit it.
    tauri_mcp_agent_lib::server::api_key::capture_env_key();

    if std::env::args().any(|arg| arg == tauri_mcp_agent_lib::browser_sidecar::BROWSER_SIDECAR_FLAG)
    {
        if let Err(error) = tauri_mcp_agent_lib::browser_sidecar::run_sidecar_mode() {
            eprintln!("❌ Browser sidecar failed: {error}");
            std::process::exit(1);
        }
        return;
    }

    // `libragent --tui` (#2761). Windows forwards to the console-subsystem
    // `libragent-tui` sibling so the launching shell waits for the TUI and
    // sees its exit code (spike evidence in `tauri_mcp_agent_lib::tui`);
    // other platforms run the TUI in this process, where console semantics
    // are native.
    if tauri_mcp_agent_lib::tui::wants_tui() {
        #[cfg(windows)]
        std::process::exit(tauri_mcp_agent_lib::tui::forward_to_console_launcher());
        #[cfg(not(windows))]
        std::process::exit(tauri_mcp_agent_lib::tui::run_tui());
    }

    #[cfg(target_os = "linux")]
    {
        println!("🐧 Linux detected - using default WebKit rendering path");
    }

    tauri_mcp_agent_lib::bootstrap::load_dotenv_files();
    let db_url = tauri_mcp_agent_lib::bootstrap::prepare_profile_runtime();

    tauri_mcp_agent_lib::run_with_sqlite_sync(db_url)
}
