// Tool modules organized by functionality
pub mod code_tools;
pub mod export_tools;
pub mod file_tools;
pub mod terminal_tools;

use crate::mcp::MCPTool;

pub fn file_tools() -> Vec<MCPTool> {
    vec![
        file_tools::create_read_file_tool(),
        file_tools::create_write_file_tool(),
        file_tools::create_list_directory_tool(),
        file_tools::create_import_files_tool(),
        file_tools::create_search_tool(),
        // editFile is now the single model-facing mutation tool.
        // Legacy per-operation tools remain dispatchable for older clients, but are hidden from
        // discovery so the agent only plans against one contract.
        file_tools::create_edit_file_tool(),
    ]
}

pub fn code_tools() -> Vec<MCPTool> {
    vec![
        // PRIMARY shell execution tool (isolated, no state preservation)
        #[cfg(unix)]
        code_tools::create_run_shell_tool(), // Unix: runShell
        #[cfg(windows)]
        code_tools::create_run_powershell_tool(), // Windows: runPowerShell
        // ADVANCED shell execution tool (persistent state)
        code_tools::create_execute_shell_tool(), // Unix: runInPersistentShell, Windows: runInPersistentPowerShell
        // Background process execution (platform-agnostic)
        code_tools::create_spawn_process_tool(), // Async: background processes
    ]
}

pub fn export_tools() -> Vec<MCPTool> {
    vec![export_tools::create_export_tool()]
}

pub fn terminal_tools() -> Vec<MCPTool> {
    use terminal_tools::{
        create_list_processes_tool, create_read_process_output_tool, create_stop_process_tool,
        create_wait_for_process_tool,
    };

    vec![
        create_read_process_output_tool(),
        create_list_processes_tool(),
        create_stop_process_tool(),
        create_wait_for_process_tool(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_tools_returns_platform_tool() {
        let tools = code_tools();
        // Model-facing tools only: isolated shell, persistent shell, and background processes.
        // Interactive callback tools stay dispatchable but are intentionally hidden from discovery.
        assert_eq!(tools.len(), 3);

        let primary_tool = &tools[0];
        #[cfg(unix)]
        assert_eq!(primary_tool.name, "runShell");
        #[cfg(windows)]
        assert_eq!(primary_tool.name, "runPowerShell");

        let persistent_tool = &tools[1];
        #[cfg(unix)]
        assert_eq!(persistent_tool.name, "runInPersistentShell");
        #[cfg(windows)]
        assert_eq!(persistent_tool.name, "runInPersistentPowerShell");

        // Verify async tool exists
        let async_tool = &tools[2];
        assert_eq!(async_tool.name, "spawnProcess");
    }
}
