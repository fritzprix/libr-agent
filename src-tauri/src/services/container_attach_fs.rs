//! File sync helpers for Docker attach mode (Harbor task containers).
//!
//! Staging host workspace is a buffer; the attached container workdir is source of truth.

use std::path::Path;

use crate::models::workspace_isolation::WorkspaceIsolationMode;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::SessionMetadata;
use tokio::process::Command as AsyncCommand;

fn apply_docker_cli_env(cmd: &mut AsyncCommand) {
    if let Ok(host) = std::env::var("DOCKER_HOST") {
        cmd.env("DOCKER_HOST", host);
    }
    if let Ok(context) = std::env::var("DOCKER_CONTEXT") {
        cmd.env("DOCKER_CONTEXT", context);
    }
}

/// Returns attach session metadata when the session is Docker-attach mode.
pub fn attach_session_info(session: &SessionMetadata) -> Option<AttachSessionInfo<'_>> {
    if session.workspace_isolation != WorkspaceIsolationMode::Docker {
        return None;
    }
    let config = session.docker_config.as_ref()?;
    if !config.is_attach() {
        return None;
    }
    let container = config.attach_container_name()?;
    let host_workspace = session.docker_host_workspace_path.as_deref()?;
    Some(AttachSessionInfo {
        container,
        workdir: config.workdir(),
        host_workspace: Path::new(host_workspace),
    })
}

pub struct AttachSessionInfo<'a> {
    pub container: &'a str,
    pub workdir: &'a str,
    pub host_workspace: &'a Path,
}

impl AttachSessionInfo<'_> {
    pub fn container_path_for_host_file(&self, host_file: &Path) -> Result<String, String> {
        let relative = host_file.strip_prefix(self.host_workspace).map_err(|_| {
            format!(
                "Host path '{}' is outside attach staging workspace '{}'",
                host_file.display(),
                self.host_workspace.display()
            )
        })?;
        let rel = relative.to_string_lossy().replace('\\', "/");
        if rel.is_empty() || rel == "." {
            Ok(self.workdir.trim_end_matches('/').to_string())
        } else {
            Ok(format!(
                "{}/{}",
                self.workdir.trim_end_matches('/'),
                rel.trim_start_matches('/')
            ))
        }
    }
}

async fn run_docker(args: &[&str]) -> Result<(), String> {
    let mut cmd = AsyncCommand::new("docker");
    apply_docker_cli_env(&mut cmd);
    cmd.args(args);
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Failed to run docker {}: {e}", args.join(" ")))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(format!(
        "docker {} failed: {}{}",
        args.join(" "),
        stderr.trim(),
        if stderr.is_empty() { stdout.trim() } else { "" }
    ))
}

async fn run_docker_stdout(args: &[&str]) -> Result<String, String> {
    let mut cmd = AsyncCommand::new("docker");
    apply_docker_cli_env(&mut cmd);
    cmd.args(args);
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Failed to run docker {}: {e}", args.join(" ")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "docker {} failed: {}",
            args.join(" "),
            stderr.trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Ensure parent directories exist inside the container for `container_path`.
async fn ensure_container_parent_dirs(container: &str, container_path: &str) -> Result<(), String> {
    let parent = Path::new(container_path)
        .parent()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .filter(|p| !p.is_empty() && p != "/");
    let Some(parent) = parent else {
        return Ok(());
    };
    run_docker(&["exec", container, "mkdir", "-p", &parent]).await
}

/// Push a staged host file into the attached container.
pub async fn push_host_file_to_container(
    session: &SessionMetadata,
    host_file: &Path,
) -> Result<(), String> {
    let Some(info) = attach_session_info(session) else {
        return Ok(());
    };
    if host_file.strip_prefix(info.host_workspace).is_err() {
        // Teamwork / skill / other host-only roots — not mirrored into the task container.
        return Ok(());
    }
    if !host_file.is_file() {
        return Err(format!(
            "Cannot push missing host file '{}' to attach container",
            host_file.display()
        ));
    }
    let container_path = info.container_path_for_host_file(host_file)?;
    ensure_container_parent_dirs(info.container, &container_path).await?;
    let dest = format!("{}:{}", info.container, container_path);
    run_docker(&["cp", &host_file.to_string_lossy(), &dest]).await
}

/// Pull a container file into the staging host workspace.
///
/// Returns `Ok(())` when the session is not attach-mode, or the path is host-only
/// (skills/teamwork outside staging). Missing remote files are soft-ok; other docker
/// failures propagate so callers do not silently read stale staging content.
pub async fn pull_container_file_to_host(
    session: &SessionMetadata,
    host_file: &Path,
) -> Result<(), String> {
    let Some(info) = attach_session_info(session) else {
        return Ok(());
    };
    if host_file.strip_prefix(info.host_workspace).is_err() {
        // Skill / teamwork / other host-only roots — no container sync.
        return Ok(());
    }
    let container_path = info.container_path_for_host_file(host_file)?;
    if let Some(parent) = host_file.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            format!(
                "Failed to create staging parent '{}': {e}",
                parent.display()
            )
        })?;
    }
    let source = format!("{}:{}", info.container, container_path);
    match run_docker(&["cp", &source, &host_file.to_string_lossy()]).await {
        Ok(()) => Ok(()),
        Err(err) if is_missing_container_path_error(&err) => Ok(()),
        Err(err) => Err(err),
    }
}

fn is_missing_container_path_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("no such file")
        || lower.contains("cannot find the file")
        || lower.contains("could not find the file")
        || lower.contains("does not exist")
}

/// List directory entries inside the attached container.
///
/// - `Ok(None)` — not an attach session, or path is outside staging (use host listing).
/// - `Ok(Some(_))` — container listing.
/// - `Err(_)` — attach path under staging but docker listing failed (do not fall back).
pub async fn list_container_directory(
    session: &SessionMetadata,
    host_dir: &Path,
) -> Result<Option<Vec<ContainerDirEntry>>, String> {
    let Some(info) = attach_session_info(session) else {
        return Ok(None);
    };
    if host_dir.strip_prefix(info.host_workspace).is_err() {
        return Ok(None);
    }
    let container_path = info.container_path_for_host_file(host_dir)?;
    let output =
        run_docker_stdout(&["exec", info.container, "ls", "-1ApF", &container_path]).await?;
    let entries = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(parse_ls_apf_entry)
        .collect();
    Ok(Some(entries))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerDirEntry {
    pub name: String,
    /// `"file"` or `"directory"` for agent tool consumers.
    pub entry_type: &'static str,
}

/// Parse `ls -1ApF` output: directories end with `/`; strip other `-F` indicators.
fn parse_ls_apf_entry(raw: &str) -> ContainerDirEntry {
    if let Some(name) = raw.strip_suffix('/') {
        return ContainerDirEntry {
            name: name.to_string(),
            entry_type: "directory",
        };
    }
    let name = raw
        .strip_suffix('*')
        .or_else(|| raw.strip_suffix('@'))
        .or_else(|| raw.strip_suffix('|'))
        .or_else(|| raw.strip_suffix('='))
        .or_else(|| raw.strip_suffix('>'))
        .unwrap_or(raw);
    ContainerDirEntry {
        name: name.to_string(),
        entry_type: "file",
    }
}

/// Resolve session metadata for attach sync helpers.
///
/// Returns `Ok(None)` when the repository is unavailable, the session is missing,
/// or metadata cannot be loaded. Callers must only treat docker sync failures as
/// hard errors after confirming the session is attach-mode.
pub async fn load_session(session_id: &str) -> Result<Option<SessionMetadata>, String> {
    let Some(repo) = crate::state::try_get_session_repository() else {
        return Ok(None);
    };
    match repo.get_session(session_id).await {
        Ok(session) => Ok(session),
        Err(error) => {
            tracing::warn!(
                session_id,
                error = %error,
                "Skipping attach sync; failed to load session metadata"
            );
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attach_session_info_builds_container_paths_under_workdir() {
        let info = AttachSessionInfo {
            container: "abc",
            workdir: "/app",
            host_workspace: Path::new("/tmp/staging"),
        };
        assert_eq!(
            info.container_path_for_host_file(Path::new("/tmp/staging/gpt2.c"))
                .expect("path under staging"),
            "/app/gpt2.c"
        );
        assert_eq!(
            info.container_path_for_host_file(Path::new("/tmp/staging"))
                .expect("staging root"),
            "/app"
        );
        assert!(info
            .container_path_for_host_file(Path::new("/tmp/other/file"))
            .is_err());
    }

    #[test]
    fn parse_ls_apf_entry_distinguishes_directories() {
        assert_eq!(
            parse_ls_apf_entry("src/"),
            ContainerDirEntry {
                name: "src".to_string(),
                entry_type: "directory",
            }
        );
        assert_eq!(
            parse_ls_apf_entry("gpt2.c"),
            ContainerDirEntry {
                name: "gpt2.c".to_string(),
                entry_type: "file",
            }
        );
        assert_eq!(
            parse_ls_apf_entry("run*"),
            ContainerDirEntry {
                name: "run".to_string(),
                entry_type: "file",
            }
        );
        assert_eq!(
            parse_ls_apf_entry("link@"),
            ContainerDirEntry {
                name: "link".to_string(),
                entry_type: "file",
            }
        );
    }

    #[test]
    fn missing_container_path_errors_are_detected() {
        assert!(is_missing_container_path_error(
            "docker cp failed: No such file or directory"
        ));
        assert!(!is_missing_container_path_error(
            "docker cp failed: permission denied"
        ));
    }
}
