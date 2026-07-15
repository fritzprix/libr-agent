use super::edit_mode::workspace_file_tools_context_list;
use super::persistent_shell;
use super::terminal_manager;
use crate::session::SessionManager;

/// Agent-facing workspace state shared by the context prompt and structured service context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceLiveState {
    pub workspace_dir: String,
    pub shell_cwd: String,
    pub is_docker: bool,
}

/// Build workspace display state for a session.
pub async fn build_workspace_live_state(
    session_id: &str,
    session_manager: &SessionManager,
    shell_manager: &persistent_shell::PersistentShellManager,
) -> WorkspaceLiveState {
    let is_docker = super::utils::is_session_docker_isolated(session_id).await;
    let host_workspace = session_manager.get_session_workspace_dir_by_id(session_id);
    let workspace_dir = super::utils::effective_workspace_root(is_docker, &host_workspace);
    let shell_cwd = match shell_manager.get_shell_cwd(session_id).await {
        Some(cwd) => super::utils::display_shell_cwd(&cwd, &workspace_dir, is_docker),
        None => ".".to_string(),
    };

    WorkspaceLiveState {
        workspace_dir,
        shell_cwd,
        is_docker,
    }
}

/// Build the service context prompt text used in BuiltinMCPServer::get_service_context.
pub async fn build_context_prompt(
    session_id: &str,
    session_manager: &SessionManager,
    process_registry: &terminal_manager::ProcessRegistry,
    shell_manager: &persistent_shell::PersistentShellManager,
) -> String {
    let state = build_workspace_live_state(session_id, session_manager, shell_manager).await;

    // Running processes with IDs for AI visibility (finished-only totals omitted — no actionable IDs).
    let running_processes_text = {
        match process_registry.try_read() {
            Ok(reg) => {
                let mut running_processes: Vec<(String, String)> = reg
                    .entries
                    .values()
                    .filter(|e| e.session_id == session_id)
                    .filter(|e| terminal_manager::is_active_process_status(&e.status))
                    .map(|e| (e.id.clone(), e.command.clone()))
                    .collect();

                running_processes.sort_by(|left, right| left.0.cmp(&right.0));
                let running_count = running_processes.len();
                let displayed_processes = running_processes.into_iter().take(5).collect::<Vec<_>>();

                if running_count == 0 {
                    "None".to_string()
                } else {
                    let process_list = displayed_processes
                        .iter()
                        .map(|(id, cmd)| {
                            let display_cmd = crate::utils::truncate_chars(cmd, 77);
                            format!("  • {} - {}", id, display_cmd)
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("{}\n{}", running_count, process_list)
                }
            }
            Err(_) => {
                // Lock is held by another task, return defaults to avoid blocking
                "None".to_string()
            }
        }
    };

    let file_tools_list = workspace_file_tools_context_list();
    let isolation_lines = if state.is_docker {
        format!(
            "- Isolation: Docker (shell commands run in a Linux container; workspace root is /workspace)\n\
             - File tools ({file_tools_list}) access the same /workspace files via the host bind mount; changes outside /workspace are visible to shell only, not to file tools\n"
        )
    } else {
        String::new()
    };

    format!(
        "## Workspace

### Live State
{isolation_lines}- Workspace Root: {workspace_dir}
- Persistent Shell CWD: {shell_cwd}
- Running Processes: {running_processes_text}
- Internal Paths: `.libragent/tmp/` (process I/O), `.libragent/exports/` (exported files) are hidden from listing to keep workspace clean.",
        workspace_dir = state.workspace_dir,
        shell_cwd = state.shell_cwd,
    )
}

/// Detect default shell for the platform
pub fn detect_shell(os: &str) -> String {
    match os {
        "windows" => {
            // Check for PowerShell vs CMD
            if std::env::var("PSModulePath").is_ok() {
                "powershell".to_string()
            } else {
                "cmd".to_string()
            }
        }
        "macos" | "linux" => {
            // Check SHELL environment variable
            std::env::var("SHELL")
                .ok()
                .and_then(|shell_path| shell_path.split('/').next_back().map(|s| s.to_string()))
                .unwrap_or_else(|| "bash".to_string())
        }
        _ => "unknown".to_string(),
    }
}
