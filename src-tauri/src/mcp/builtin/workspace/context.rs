use super::edit_mode::workspace_file_tools_context_list;
use super::persistent_shell;
use super::terminal_manager;
use crate::services::workspace_runtime_manager::WorkspaceRuntimeManager;
use crate::session::SessionManager;

/// Agent-facing workspace state shared by the context prompt and structured service context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceLiveState {
    pub workspace_dir: String,
    pub shell_cwd: String,
    pub is_docker: bool,
}

/// OS / arch / shell that shell tools actually execute against.
///
/// In Docker isolation this describes the container, not the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPlatform {
    pub os: String,
    pub arch: String,
    pub shell: String,
}

impl ExecutionPlatform {
    /// Resolve execution platform for prompt text and structured UI state.
    ///
    /// `docker_shell` / `docker_arch` come from runtime caches when available.
    /// Fallbacks avoid hardcoding a single arch and avoid claiming bash when
    /// the container may only expose `sh`.
    pub fn resolve(
        is_docker: bool,
        host_os: &str,
        host_arch: &str,
        docker_shell: Option<&str>,
        docker_arch: Option<&str>,
    ) -> Self {
        if is_docker {
            Self {
                os: "linux".to_string(),
                arch: docker_arch.unwrap_or(host_arch).to_string(),
                shell: docker_shell.unwrap_or("bash").to_string(),
            }
        } else {
            Self {
                os: host_os.to_string(),
                arch: host_arch.to_string(),
                shell: detect_shell(host_os),
            }
        }
    }

    /// Look up Docker runtime caches and resolve the platform for a session.
    pub fn for_session(session_id: &str, is_docker: bool) -> Self {
        let docker_shell = if is_docker {
            WorkspaceRuntimeManager::cached_docker_shell(session_id)
                .map(|shell| shell.command().to_string())
        } else {
            None
        };
        let docker_arch = if is_docker {
            WorkspaceRuntimeManager::cached_docker_arch(session_id)
        } else {
            None
        };

        Self::resolve(
            is_docker,
            std::env::consts::OS,
            std::env::consts::ARCH,
            docker_shell.as_deref(),
            docker_arch.as_deref(),
        )
    }

    pub fn platform_line(&self) -> String {
        format!("- Platform: {} ({})", self.os, self.arch)
    }

    pub fn shell_line(&self) -> String {
        format!("- Default Shell: {}", self.shell)
    }

    pub fn to_structured_json(&self) -> serde_json::Value {
        serde_json::json!({
            "os": self.os,
            "arch": self.arch,
            "shell": self.shell,
        })
    }
}

/// Build workspace display state for a session.
pub async fn build_workspace_live_state(
    session_id: &str,
    session_manager: &SessionManager,
    shell_manager: &persistent_shell::PersistentShellManager,
) -> WorkspaceLiveState {
    let (is_docker, docker_root) = super::utils::session_docker_root(session_id).await;
    let host_workspace = session_manager.get_session_workspace_dir_by_id(session_id);
    let workspace_dir = super::utils::effective_workspace_root_with_docker_root(
        is_docker,
        &host_workspace,
        &docker_root,
    );
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

/// Format the Running Processes section for the workspace service context.
///
/// Awaits the registry lock so lock contention never produces a false "None".
pub async fn format_running_processes_text(
    process_registry: &terminal_manager::ProcessRegistry,
    session_id: &str,
) -> String {
    let reg = process_registry.read().await;
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

/// Build the service context prompt text used in BuiltinMCPServer::get_service_context.
pub async fn build_context_prompt(
    session_id: &str,
    session_manager: &SessionManager,
    process_registry: &terminal_manager::ProcessRegistry,
    shell_manager: &persistent_shell::PersistentShellManager,
) -> String {
    let state = build_workspace_live_state(session_id, session_manager, shell_manager).await;

    // Running processes with IDs for AI visibility (finished-only totals omitted — no actionable IDs).
    let running_processes_text = format_running_processes_text(process_registry, session_id).await;

    let file_tools_list = workspace_file_tools_context_list();
    let isolation_lines = if state.is_docker {
        let root = &state.workspace_dir;
        format!(
            "- Isolation: Docker (shell commands run in a Linux container; workspace root is {root})\n\
             - File tools ({file_tools_list}) access the same {root} files (bind mount or attach sync); changes outside {root} are visible to shell only, not to file tools\n"
        )
    } else {
        String::new()
    };

    let platform = ExecutionPlatform::for_session(session_id, state.is_docker);
    let platform_info = platform.platform_line();
    let shell_info = platform.shell_line();

    format!(
        "## Workspace

### Live State
{isolation_lines}- Workspace Root: {workspace_dir}
- Persistent Shell CWD: {shell_cwd}
{platform_info}
{shell_info}
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
