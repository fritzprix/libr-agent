use super::persistent_shell;
use super::terminal_manager;
use crate::session::SessionManager;

/// Build the service context prompt text used in BuiltinMCPServer::get_service_context.
pub async fn build_context_prompt(
    session_id: &str,
    session_manager: &SessionManager,
    process_registry: &terminal_manager::ProcessRegistry,
    shell_manager: &persistent_shell::PersistentShellManager,
) -> String {
    let workspace_dir_path = session_manager.get_session_workspace_dir_by_id(session_id);
    let workspace_dir = {
        let path_str = workspace_dir_path.to_string_lossy().to_string();
        #[cfg(target_os = "windows")]
        let path_str = path_str.replace('\\', "/");
        path_str
    };

    // Get current shell CWD
    let shell_cwd = if let Some(cwd) = shell_manager.get_shell_cwd(session_id).await {
        let cwd = {
            #[cfg(target_os = "windows")]
            let cwd = cwd.replace('\\', "/");
            cwd
        };

        // Convert to relative path if within workspace for better readability
        if cwd.starts_with(&workspace_dir) {
            cwd.replacen(&workspace_dir, ".", 1)
        } else {
            cwd
        }
    } else {
        ".".to_string()
    };

    // ✅ ENHANCED: Get running processes with IDs and commands for AI visibility
    let (running_count, total_count, running_processes_text) = {
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

                let total_count = reg
                    .entries
                    .values()
                    .filter(|e| e.session_id == session_id)
                    .count();

                let running_text = if running_count == 0 {
                    "None".to_string()
                } else {
                    let process_list = displayed_processes
                        .iter()
                        .map(|(id, cmd)| {
                            // Truncate command if too long (safe string slicing)
                            let display_cmd = crate::utils::truncate_chars(cmd, 77);
                            format!("  • {} - {}", id, display_cmd)
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("\n{}", process_list)
                };

                (running_count, total_count, running_text)
            }
            Err(_) => {
                // Lock is held by another task, return defaults to avoid blocking
                (0, 0, "None".to_string())
            }
        }
    };

    let context_prompt = format!(
        "## Workspace

### Live State
- Workspace Root: {workspace_dir}
- Persistent Shell CWD: {shell_cwd}
- Running Processes: {running_count}{running_processes_text}
- Internal Paths: `.libragent/tmp/` (process I/O), `.libragent/exports/` (exported files) are hidden from listing to keep workspace clean.
- Total Processes: {total_count}"
    );

    context_prompt
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
