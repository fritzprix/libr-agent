// Tool modules organized by functionality
pub mod code_tools;
pub mod export_tools;
pub mod file_tools;
pub mod terminal_tools;

use crate::mcp::MCPTool;

pub fn file_tools() -> Vec<MCPTool> {
    let mut tools = vec![
        file_tools::create_read_file_tool(),
        file_tools::create_write_file_tool(),
        file_tools::create_list_directory_tool(),
        file_tools::create_import_files_tool(),
        file_tools::create_search_tool(),
    ];

    #[cfg(feature = "workspace-str-replace")]
    tools.push(file_tools::create_str_replace_tool());

    #[cfg(feature = "workspace-edit-file")]
    {
        // editFile is the single model-facing mutation tool when line+anchor edits are enabled.
        // Per-operation aliases remain dispatchable for older clients, but are hidden from
        // discovery so the agent only plans against one contract.
        tools.push(file_tools::create_edit_file_tool());
    }

    tools
}

#[allow(clippy::vec_init_then_push)]
pub fn code_tools() -> Vec<MCPTool> {
    let mut list = Vec::new();

    // 1. PRIMARY shell execution tool (isolated)
    #[cfg(unix)]
    list.push(code_tools::create_run_shell_tool());
    #[cfg(windows)]
    {
        list.push(code_tools::create_run_powershell_tool());
        list.push(code_tools::create_run_shell_tool());
    }

    // 2. ADVANCED shell execution tool (persistent)
    #[cfg(unix)]
    list.push(code_tools::create_run_persistent_shell_tool());
    #[cfg(windows)]
    {
        list.push(code_tools::create_run_persistent_powershell_tool());
        list.push(code_tools::create_run_persistent_shell_tool());
    }

    // 3. Background process execution (platform-agnostic)
    list.push(code_tools::create_spawn_process_tool());

    list
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

        #[cfg(unix)]
        {
            assert_eq!(tools.len(), 3);
            assert_eq!(tools[0].name, "runShell");
            assert_eq!(tools[1].name, "runInPersistentShell");
            assert_eq!(tools[2].name, "spawnProcess");
        }

        #[cfg(windows)]
        {
            assert_eq!(tools.len(), 5);
            assert_eq!(tools[0].name, "runPowerShell");
            assert_eq!(tools[1].name, "runShell");
            assert_eq!(tools[2].name, "runInPersistentPowerShell");
            assert_eq!(tools[3].name, "runInPersistentShell");
            assert_eq!(tools[4].name, "spawnProcess");
        }
    }
}
