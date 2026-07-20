// Tool modules organized by functionality
pub mod code_tools;
pub mod export_tools;
pub mod file_tools;
pub mod terminal_tools;

use crate::mcp::MCPTool;
use crate::models::workspace_isolation::WorkspaceIsolationMode;

/// Which shell dialect tools to expose for a session.
///
/// Host Windows exposes PowerShell only. Host Unix and Docker (any host OS)
/// expose bash/sh tools only — Docker always executes via `docker exec … -lc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeToolsProfile {
    HostUnix,
    HostWindows,
    Docker,
}

impl CodeToolsProfile {
    pub fn from_isolation(isolation: WorkspaceIsolationMode) -> Self {
        match isolation {
            WorkspaceIsolationMode::Docker => Self::Docker,
            WorkspaceIsolationMode::Host => {
                if cfg!(windows) {
                    Self::HostWindows
                } else {
                    Self::HostUnix
                }
            }
        }
    }

    /// Whether a workspace shell tool name is discoverable/callable for this profile.
    pub fn allows_shell_tool(self, tool_name: &str) -> bool {
        match tool_name {
            "runShell" | "runInPersistentShell" => {
                matches!(self, Self::HostUnix | Self::Docker)
            }
            "runPowerShell" | "runInPersistentPowerShell" => {
                matches!(self, Self::HostWindows)
            }
            _ => true,
        }
    }
}

pub fn file_tools() -> Vec<MCPTool> {
    let mut tools = vec![
        file_tools::create_read_file_tool(),
        file_tools::create_write_file_tool(),
        file_tools::create_list_directory_tool(),
        file_tools::create_import_files_tool(),
        file_tools::create_glob_files_tool(),
        file_tools::create_grep_files_tool(),
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
pub fn code_tools(profile: CodeToolsProfile) -> Vec<MCPTool> {
    let mut list = Vec::new();

    match profile {
        CodeToolsProfile::HostUnix | CodeToolsProfile::Docker => {
            list.push(code_tools::create_run_shell_tool());
            list.push(code_tools::create_run_persistent_shell_tool());
        }
        CodeToolsProfile::HostWindows => {
            // Selected only when cfg!(windows) in from_isolation(). The non-Windows
            // arm exists solely so the match stays exhaustive on Unix builds; it is
            // unreachable at runtime (Docker uses Self::Docker, never HostWindows).
            #[cfg(windows)]
            {
                list.push(code_tools::create_run_powershell_tool());
                list.push(code_tools::create_run_persistent_powershell_tool());
            }
            #[cfg(not(windows))]
            {
                unreachable!("CodeToolsProfile::HostWindows is only constructed on Windows hosts");
            }
        }
    }

    // Background process execution (platform-agnostic)
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
