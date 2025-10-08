// Allow dead code in tests
#![allow(dead_code)]

use std::sync::Arc;
use tauri_mcp_agent_lib::terminal_manager::TerminalManager;

pub fn setup_terminal_manager() -> Arc<TerminalManager> {
    Arc::new(TerminalManager::new())
}
