//! File sync helpers for Docker attach mode (Harbor task containers).
//!
//! Staging host workspace is a buffer; the attached container workdir is source of truth.

use std::path::Path;

use crate::mcp::builtin::utils::relative_path_under_base;
use crate::models::workspace_isolation::WorkspaceIsolationMode;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::SessionMetadata;
use tokio::process::Command as AsyncCommand;

fn apply_docker_cli_env(cmd: &mut AsyncCommand) {
    // Same as WorkspaceRuntimeManager: hide docker.exe console on Windows GUI hosts.
    crate::utils::platform::suppress_console_window_async(cmd);
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
        // Use Windows-aware relative matching (verbatim `\\?\` vs normal drive, case).
        // Raw Path::strip_prefix silently fails across those forms and previously caused
        // writeFile to skip docker cp while listDirectory fell back to host staging —
        // so the agent saw the file but the container shell did not.
        let relative =
            relative_path_under_base(host_file, self.host_workspace).ok_or_else(|| {
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
    if relative_path_under_base(host_file, info.host_workspace).is_none() {
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
    // Prefer a non-verbatim host path for docker.exe on Windows.
    let host_cp_path = simplify_host_path_for_docker(host_file);
    run_docker(&["cp", &host_cp_path, &dest]).await?;
    verify_container_file(info.container, &container_path).await
}

fn simplify_host_path_for_docker(path: &Path) -> String {
    crate::mcp::builtin::utils::display_workspace_path(path)
}

async fn verify_container_file(container: &str, container_path: &str) -> Result<(), String> {
    let mut cmd = AsyncCommand::new("docker");
    apply_docker_cli_env(&mut cmd);
    cmd.args(["exec", container, "test", "-f", container_path]);
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Failed to verify attach file after docker cp: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "docker cp reported success but '{container_path}' is not a regular file inside container '{container}'. \
         File tools and shell may disagree until sync succeeds — retry writeFile."
    ))
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
    if relative_path_under_base(host_file, info.host_workspace).is_none() {
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
    let host_dest = simplify_host_path_for_docker(host_file);
    match run_docker(&["cp", &source, &host_dest]).await {
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
    if relative_path_under_base(host_dir, info.host_workspace).is_none() {
        return Ok(None);
    }
    let container_path = info.container_path_for_host_file(host_dir)?;

    // Prefer `ls -lA` for type + size. Fall back to name-only `ls -1ApF` when long
    // listing fails or yields no parseable entries (BusyBox quirks, empty dirs still OK).
    match run_docker_stdout(&["exec", info.container, "ls", "-lA", &container_path]).await {
        Ok(output) => {
            let entries = parse_ls_la_output(&output);
            if !entries.is_empty() || output_looks_like_empty_directory(&output) {
                return Ok(Some(entries));
            }
            // Unparseable long listing — try name-only fallback below.
        }
        Err(err) => {
            // Hard path errors should not silently fall through to name-only when the
            // directory truly does not exist; still attempt APF fallback for ls-flag
            // incompatibilities, then surface the original error if that also fails.
            if let Ok(apf_output) =
                run_docker_stdout(&["exec", info.container, "ls", "-1ApF", &container_path]).await
            {
                return Ok(Some(parse_ls_apf_output(&apf_output)));
            }
            return Err(err);
        }
    }

    let apf_output =
        run_docker_stdout(&["exec", info.container, "ls", "-1ApF", &container_path]).await?;
    Ok(Some(parse_ls_apf_output(&apf_output)))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerDirEntry {
    pub name: String,
    /// `"file"` or `"directory"` for agent tool consumers.
    pub entry_type: &'static str,
    /// File size in bytes; `None` for directories or when size could not be parsed.
    pub size: Option<u64>,
}

fn parse_ls_apf_output(output: &str) -> Vec<ContainerDirEntry> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(parse_ls_apf_entry)
        .collect()
}

fn parse_ls_la_output(output: &str) -> Vec<ContainerDirEntry> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(parse_ls_la_entry)
        .collect()
}

fn output_looks_like_empty_directory(output: &str) -> bool {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .all(|line| line.starts_with("total ") || line.starts_with("total\t"))
}

/// Parse one `ls -lA` line (GNU or BusyBox). Returns `None` for `total N` / unparseable.
fn parse_ls_la_entry(raw: &str) -> Option<ContainerDirEntry> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with("total ") || trimmed.starts_with("total\t") {
        return None;
    }

    let mut parts = trimmed.split_whitespace();
    let perms = parts.next()?;
    let first = perms.chars().next()?;
    // Require a permission-style first field (e.g. drwxr-xr-x, -rw-r--r--, lrwxrwxrwx).
    if !matches!(first, 'd' | '-' | 'l' | 'c' | 'b' | 'p' | 's') || perms.len() < 10 {
        return None;
    }

    let _links = parts.next()?;
    let _owner = parts.next()?;
    let _group = parts.next()?;
    let size_str = parts.next()?;
    let _month_or_date = parts.next()?;
    let _day_or_time = parts.next()?;
    let _time_or_year = parts.next()?;

    let name_with_link: String = parts.collect::<Vec<_>>().join(" ");
    if name_with_link.is_empty() {
        return None;
    }

    // Symlinks: `name -> target`
    let name = name_with_link
        .split_once(" -> ")
        .map(|(name, _)| name)
        .unwrap_or(&name_with_link)
        .to_string();

    if name == "." || name == ".." {
        return None;
    }

    let is_dir = first == 'd';
    let size = if is_dir {
        None
    } else {
        size_str.parse::<u64>().ok()
    };

    Some(ContainerDirEntry {
        name,
        entry_type: if is_dir { "directory" } else { "file" },
        size,
    })
}

/// Parse `ls -1ApF` output: directories end with `/`; strip other `-F` indicators.
fn parse_ls_apf_entry(raw: &str) -> ContainerDirEntry {
    if let Some(name) = raw.strip_suffix('/') {
        return ContainerDirEntry {
            name: name.to_string(),
            entry_type: "directory",
            size: None,
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
        size: None,
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

    #[cfg(windows)]
    #[test]
    fn attach_paths_match_across_verbatim_and_disk_prefixes() {
        use std::path::PathBuf;
        let host_workspace = Path::new(r"C:\Users\test\staging");
        let info = AttachSessionInfo {
            container: "abc",
            workdir: "/workspace",
            host_workspace,
        };
        let verbatim = PathBuf::from(r"\\?\C:\Users\test\staging\analysis.py");
        assert_eq!(
            info.container_path_for_host_file(&verbatim)
                .expect("verbatim path under staging"),
            "/workspace/analysis.py"
        );
    }

    #[cfg(windows)]
    #[test]
    fn simplify_host_path_strips_verbatim_prefix() {
        let path = Path::new(r"\\?\C:\Users\test\staging\analysis.py");
        assert_eq!(
            simplify_host_path_for_docker(path),
            r"C:\Users\test\staging\analysis.py"
        );
    }

    #[test]
    fn parse_ls_apf_entry_distinguishes_directories() {
        assert_eq!(
            parse_ls_apf_entry("src/"),
            ContainerDirEntry {
                name: "src".to_string(),
                entry_type: "directory",
                size: None,
            }
        );
        assert_eq!(
            parse_ls_apf_entry("gpt2.c"),
            ContainerDirEntry {
                name: "gpt2.c".to_string(),
                entry_type: "file",
                size: None,
            }
        );
        assert_eq!(
            parse_ls_apf_entry("run*"),
            ContainerDirEntry {
                name: "run".to_string(),
                entry_type: "file",
                size: None,
            }
        );
        assert_eq!(
            parse_ls_apf_entry("link@"),
            ContainerDirEntry {
                name: "link".to_string(),
                entry_type: "file",
                size: None,
            }
        );
    }

    #[test]
    fn parse_ls_la_entry_gnu_style_file_and_directory() {
        assert_eq!(
            parse_ls_la_entry("-rw-r--r-- 1 root root 123 Jan  1 00:00 gpt2.c"),
            Some(ContainerDirEntry {
                name: "gpt2.c".to_string(),
                entry_type: "file",
                size: Some(123),
            })
        );
        assert_eq!(
            parse_ls_la_entry("drwxr-xr-x 2 root root 4096 Jan  1 00:00 src"),
            Some(ContainerDirEntry {
                name: "src".to_string(),
                entry_type: "directory",
                size: None,
            })
        );
    }

    #[test]
    fn parse_ls_la_entry_busybox_style_and_symlink() {
        assert_eq!(
            parse_ls_la_entry("-rw-r--r--    1 root     root           512 Jan  1  1970 main.db"),
            Some(ContainerDirEntry {
                name: "main.db".to_string(),
                entry_type: "file",
                size: Some(512),
            })
        );
        assert_eq!(
            parse_ls_la_entry("lrwxrwxrwx 1 root root 11 Jan  1 00:00 link -> target.txt"),
            Some(ContainerDirEntry {
                name: "link".to_string(),
                entry_type: "file",
                size: Some(11),
            })
        );
    }

    #[test]
    fn parse_ls_la_entry_handles_names_with_spaces_and_skips_total() {
        assert_eq!(
            parse_ls_la_entry("-rw-r--r-- 1 root root 42 Jul 26 12:00 my file.txt"),
            Some(ContainerDirEntry {
                name: "my file.txt".to_string(),
                entry_type: "file",
                size: Some(42),
            })
        );
        assert!(parse_ls_la_entry("total 16").is_none());
        assert!(parse_ls_la_entry("total 0").is_none());
    }

    #[test]
    fn parse_ls_la_output_collects_entries_and_empty_total_only() {
        let output = "\
total 20
drwxr-xr-x 2 root root 4096 Jan  1 00:00 src
-rw-r--r-- 1 root root  123 Jan  1 00:00 gpt2.c
";
        let entries = parse_ls_la_output(output);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "src");
        assert_eq!(entries[1].size, Some(123));

        assert!(output_looks_like_empty_directory("total 0\n"));
        assert!(!output_looks_like_empty_directory(output));
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
