//! `libragent-tui` binary entry point (#2761).
//!
//! Unlike `libragent` (`src/main.rs`), `libragent-tui` is a console
//! executable on all platforms (no `windows_subsystem = "windows"`), which
//! is the whole point on Windows: terminals and scripts wait for a
//! console-subsystem process and see its exit code, while a GUI-subsystem
//! process is dropped to the background. The measurement evidence for this
//! choice lives in `tauri_mcp_agent_lib::tui`.

fn main() {
    tauri_mcp_agent_lib::server::api_key::capture_env_key();
    std::process::exit(tauri_mcp_agent_lib::tui::run_tui());
}
