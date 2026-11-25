//! `libragent-tui` binary entry point (#2761).
//!
//! Unlike `libragent` (`src/main.rs`), `libragent-tui` is a console
//! executable on all platforms (no `windows_subsystem = "windows"`), which
//! is the whole point on Windows: terminals and scripts wait for a
//! console-subsystem process and see its exit code, while a GUI-subsystem
//! process is dropped to the background. The measurement evidence for this
//! choice lives in `tauri_mcp_agent_lib::tui`.
//!
//! Developer-only after #2861: the binary refuses to start unless the hidden
//! `LIBRAGENT_ENABLE_TUI=1` flag is set, so it never becomes an undocumented
//! user entry.

fn main() {
    tauri_mcp_agent_lib::server::api_key::capture_env_key();
    if !tauri_mcp_agent_lib::tui::dev_tui_enabled() {
        std::process::exit(tauri_mcp_agent_lib::tui::refuse_hidden_tui(
            "`libragent-tui`",
        ));
    }
    std::process::exit(tauri_mcp_agent_lib::tui::run_tui());
}
