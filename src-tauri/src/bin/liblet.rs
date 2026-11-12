//! `liblet` CLI binary entry point (#2703).
//!
//! Unlike `libragent` (`src/main.rs`), `liblet` is a console executable on all
//! platforms (no `windows_subsystem = "windows"`) so standard streams and exit
//! codes work cleanly in terminals, pipes, and scripts.

fn main() {
    tauri_mcp_agent_lib::server::api_key::capture_env_key();
    let exit_code = tauri_mcp_agent_lib::headless::run_liblet_main();
    std::process::exit(exit_code);
}
