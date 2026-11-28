//! Install-source routing for plugin/skill installs (#2928).
//!
//! One source string can be a github.com URL, any other http(s) git URL, a
//! `file://` git URL, or a local path (`.skill`, `.zip`, or directory).
//! `prepare_plugin_source_import` classifies the string and reuses the
//! matching fetcher; the github.com fast path and the local-import path are
//! the existing ones, untouched.

use super::super::contracts::{PreparedSkillImport, GIT_CLONE_TIMEOUT_SECS, MAX_CLONE_TREE_BYTES};
use super::super::scanning::discover_skill_roots;
use super::archive::{build_temp_root, prepare_github_skill_import, prepare_local_skill_import};
use crate::utils::platform::suppress_console_window_async;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use url::Url;
use uuid::Uuid;
use walkdir::WalkDir;

/// Whether one install source points at github.com, a generic git URL, or a
/// local path. Pure classification so the routing decision is testable
/// without network or filesystem access.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum InstallSourceKind {
    GitHub,
    /// Generic git URL; `git_ref` is the optional `?ref=<branch|tag>` ref.
    Git {
        url: String,
        git_ref: Option<String>,
    },
    Local,
}

pub(super) fn classify_install_source(source: &str) -> Result<InstallSourceKind, String> {
    let trimmed = source.trim();
    // Anything without `://` is a local path (this keeps Windows drive-letter
    // paths like `C:\plugins\x` out of the URL parser entirely).
    let Some(separator) = trimmed.find("://") else {
        return Ok(InstallSourceKind::Local);
    };
    let scheme = trimmed[..separator].to_ascii_lowercase();
    if !matches!(scheme.as_str(), "http" | "https" | "file") {
        return Err(format!(
            "Unsupported install source scheme '{scheme}'. Use http(s):// or file://, or a local path."
        ));
    }

    let mut parsed =
        Url::parse(trimmed).map_err(|error| format!("Invalid install URL: {error}"))?;
    match parsed.scheme() {
        "http" | "https" => {
            // Credentials embedded in the URL would be stored in the plugin
            // record and shown back in messages (#2928 gatekeeper G1.3).
            if !parsed.username().is_empty() || parsed.password().is_some() {
                return Err(
                    "Install URLs must not carry credentials. Use a credential-free URL; git \
                     reads its own credential store for private repositories."
                        .to_string(),
                );
            }
            let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
            if host == "github.com" || host == "www.github.com" {
                return Ok(InstallSourceKind::GitHub);
            }
            let git_ref = parsed
                .query_pairs()
                .into_owned()
                .find(|(key, value)| key == "ref" && !value.is_empty())
                .map(|(_, value)| value);
            parsed.set_query(None);
            parsed.set_fragment(None);
            Ok(InstallSourceKind::Git {
                url: parsed.to_string(),
                git_ref,
            })
        }
        "file" => {
            // `file://…?ref=` used to be dropped and the default branch
            // installed (#2928 gatekeeper G1.6); the ref is honored now.
            let git_ref = parsed
                .query_pairs()
                .into_owned()
                .find(|(key, value)| key == "ref" && !value.is_empty())
                .map(|(_, value)| value);
            parsed.set_query(None);
            parsed.set_fragment(None);
            let path = parsed
                .to_file_path()
                .map_err(|_| "file:// install source must point to a local path".to_string())?;
            Ok(InstallSourceKind::Git {
                url: path.to_string_lossy().to_string(),
                git_ref,
            })
        }
        scheme => Err(format!("Unsupported install source scheme '{scheme}'")),
    }
}

/// Shallow-clone one git URL into `dest`, finishing within `timeout`.
///
/// Arguments are passed as a list (never through a shell), `--` ends option
/// parsing, and a `ref` starting with `-` is rejected so neither can be read
/// back as a git option (`-c` is never accepted). `core.symlinks=false` stops
/// git materializing symlinks from the fetched tree, credential prompts are
/// disabled so a private URL fails instead of waiting, and the child is
/// killed when the timeout drops the future (#2928 gatekeeper G1.1/G1.2).
pub(super) async fn clone_git_repo(
    clone_url: &str,
    git_ref: Option<&str>,
    dest: &Path,
    timeout: Duration,
) -> Result<(), String> {
    if clone_url.starts_with('-') {
        return Err("Install URL must not start with '-'".to_string());
    }
    if let Some(git_ref) = git_ref {
        if git_ref.is_empty() || git_ref.starts_with('-') {
            return Err(format!("Invalid git ref {git_ref:?}"));
        }
    }
    let mut command = tokio::process::Command::new("git");
    command
        .arg("-c")
        .arg("core.symlinks=false")
        .arg("clone")
        .arg("--depth")
        .arg("1")
        .arg("--no-tags");
    if let Some(git_ref) = git_ref {
        command.arg("--branch").arg(git_ref);
    }
    command
        .arg("--")
        .arg(clone_url)
        .arg(dest)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "echo")
        .env("SSH_ASKPASS", "")
        .kill_on_drop(true);
    suppress_console_window_async(&mut command);
    let output = tokio::time::timeout(timeout, command.output())
        .await
        .map_err(|_| format!("git clone timed out after {}s", timeout.as_secs()))?
        .map_err(|error| format!("Failed to run git (is git installed?): {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git clone failed: {}", stderr.trim()));
    }
    Ok(())
}

/// Reject a fetched tree that a copy step could smuggle out of the install
/// directory: any symlink, or more content than `max_bytes` (excluding the
/// repository's own `.git` metadata) (#2928 gatekeeper G1.1/G1.4).
fn validate_fetched_tree(dir: &Path, max_bytes: u64) -> Result<(), String> {
    for entry in WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| entry.file_name() != ".git")
    {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_type = entry.file_type();
        if file_type.is_symlink() {
            return Err(format!(
                "Install source contains a symlink ({}); symlinks are not allowed",
                entry.path().display()
            ));
        }
        if file_type.is_file() {
            let size = entry.metadata().map_err(|error| error.to_string())?.len();
            if size > max_bytes {
                return Err(format!(
                    "Install source file {} is larger than the {} MB install limit",
                    entry.path().display(),
                    max_bytes / 1024 / 1024
                ));
            }
        }
    }
    Ok(())
}

/// Fetch a generic git source into a staging temp dir and stage its skills.
/// The clone's `.git` metadata is removed before scanning so the installed
/// plugin copy carries no repository state and `git status`-style names
/// cannot leak into skill discovery.
pub(in crate::services::skill_service) async fn prepare_git_skill_import(
    source: String,
    clone_url: String,
    git_ref: Option<String>,
    temp_root_override: Option<&Path>,
) -> Result<PreparedSkillImport, String> {
    let temp_root = match temp_root_override {
        Some(root) => root.to_path_buf(),
        None => build_temp_root()?,
    };
    let temp_dir = tempfile::Builder::new()
        .prefix("git-skill-import-")
        .tempdir_in(&temp_root)
        .map_err(|error| error.to_string())?;
    let staging_dir = temp_dir.path().join(Uuid::new_v4().to_string());
    clone_git_repo(
        &clone_url,
        git_ref.as_deref(),
        &staging_dir,
        Duration::from_secs(GIT_CLONE_TIMEOUT_SECS),
    )
    .await?;

    let scan_root = staging_dir;
    let scan_dir = scan_root.clone();
    let discovered_skills = tokio::task::spawn_blocking(move || {
        validate_fetched_tree(&scan_dir, MAX_CLONE_TREE_BYTES)?;
        let git_dir = scan_dir.join(".git");
        if git_dir.exists() {
            std::fs::remove_dir_all(&git_dir).map_err(|error| error.to_string())?;
        }
        discover_skill_roots(&scan_dir)
    })
    .await
    .map_err(|error| format!("Task join error: {error}"))??;

    Ok(PreparedSkillImport {
        _temp_dir: temp_dir,
        scan_root,
        source_key: source.trim().to_string(),
        discovered_skills,
    })
}

/// Resolve a relative local install path against `workspace_dir` and store
/// the absolute result (#2928 gatekeeper G1.5): a bare `./plugins/x` used to
/// resolve against the application process directory, and the relative
/// string was stored as the source, so a later update resolved it elsewhere.
fn resolve_local_source(source: &str, workspace_dir: Option<&Path>) -> String {
    let path = Path::new(source);
    if path.is_absolute() {
        return source.to_string();
    }
    let base = workspace_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    // Drop `.` components so the stored path stays clean.
    let joined: PathBuf = base
        .join(path)
        .components()
        .filter(|component| !matches!(component, std::path::Component::CurDir))
        .collect();
    joined.to_string_lossy().to_string()
}

/// Fetch one install source by routing it to the GitHub zipball fast path,
/// the generic git clone path, or the existing local importer (#2928).
/// `workspace_dir` is the session workspace a relative local path resolves
/// against (`None`: resolve against the process directory, still stored
/// absolute).
pub(in crate::services::skill_service) async fn prepare_plugin_source_import(
    source: String,
    workspace_dir: Option<&Path>,
) -> Result<PreparedSkillImport, String> {
    match classify_install_source(&source)? {
        InstallSourceKind::GitHub => prepare_github_skill_import(source).await,
        InstallSourceKind::Git { url, git_ref } => {
            prepare_git_skill_import(source, url, git_ref, None).await
        }
        InstallSourceKind::Local => {
            prepare_local_skill_import(resolve_local_source(source.trim(), workspace_dir)).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn classifies_github_urls_as_github_fast_path() {
        assert_eq!(
            classify_install_source("https://github.com/obra/superpowers").unwrap(),
            InstallSourceKind::GitHub
        );
        assert_eq!(
            classify_install_source("https://www.github.com/o/r?ref=dev").unwrap(),
            InstallSourceKind::GitHub
        );
    }

    #[test]
    fn classifies_other_http_urls_as_generic_git_with_ref() {
        let kind = classify_install_source("https://git.example.com/team/plugin.git?ref=v2#frag")
            .expect("valid git URL");
        assert_eq!(
            kind,
            InstallSourceKind::Git {
                url: "https://git.example.com/team/plugin.git".to_string(),
                git_ref: Some("v2".to_string()),
            }
        );
    }

    #[test]
    fn classifies_local_paths_as_local() {
        assert_eq!(
            classify_install_source("/home/user/my-plugin").unwrap(),
            InstallSourceKind::Local
        );
        assert_eq!(
            classify_install_source("./relative/plugin").unwrap(),
            InstallSourceKind::Local
        );
    }

    #[test]
    fn file_urls_classify_as_generic_git() {
        let kind = classify_install_source("file:///tmp/plugin-repo").expect("valid file URL");
        assert_eq!(
            kind,
            InstallSourceKind::Git {
                url: "/tmp/plugin-repo".to_string(),
                git_ref: None,
            }
        );
    }

    #[test]
    fn file_url_ref_is_kept_not_dropped() {
        // G1.6: `?ref=` on a file:// URL used to be silently discarded.
        let kind =
            classify_install_source("file:///tmp/plugin-repo?ref=v2").expect("valid file URL");
        assert_eq!(
            kind,
            InstallSourceKind::Git {
                url: "/tmp/plugin-repo".to_string(),
                git_ref: Some("v2".to_string()),
            }
        );
    }

    #[test]
    fn rejects_unsupported_schemes() {
        let error = classify_install_source("ssh://git.example.com/o/r").unwrap_err();
        assert!(
            error.contains("Unsupported install source scheme"),
            "{error}"
        );
    }

    #[test]
    fn rejects_urls_with_embedded_credentials() {
        // G1.3: `https://user:token@host/repo` used to be accepted and stored
        // verbatim in the plugin record.
        for url in [
            "https://user:token@git.example.com/o/r",
            "https://token@git.example.com/o/r",
            "http://user:pw@127.0.0.1:3000/o/r",
        ] {
            let error = classify_install_source(url).unwrap_err();
            assert!(
                error.contains("must not carry credentials"),
                "{url}: {error}"
            );
        }
        assert!(classify_install_source("https://git.example.com/o/r").is_ok());
    }

    #[test]
    fn relative_local_sources_resolve_to_absolute_paths() {
        // G1.5: a relative path must resolve against the session workspace
        // and be stored absolute, never against the process directory.
        let workspace = Path::new("/workspace-root");
        assert_eq!(
            resolve_local_source("./plugins/x", Some(workspace)),
            "/workspace-root/plugins/x"
        );
        assert_eq!(
            resolve_local_source("plugins/x", Some(workspace)),
            "/workspace-root/plugins/x"
        );
        assert_eq!(
            resolve_local_source("/abs/plugin", Some(workspace)),
            "/abs/plugin"
        );
        // Without a workspace the process directory is the base, but the
        // stored path is still absolute.
        let resolved = resolve_local_source("plugins/x", None);
        assert!(Path::new(&resolved).is_absolute(), "{resolved}");
    }

    #[test]
    fn fetched_tree_rejects_symlinks_and_oversized_files() {
        // G1.1 + G1.4, exercised on the same check the clone path runs.
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::create_dir_all(root.join("skills")).unwrap();
        std::fs::write(root.join("skills/SKILL.md"), "hello").unwrap();
        // `.git` is excluded from the size total.
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join(".git/blob"), vec![b'x'; 4096]).unwrap();
        validate_fetched_tree(root, 1024).unwrap();

        std::fs::write(root.join("skills/SKILL.md"), vec![b'x'; 2048]).unwrap();
        let error = validate_fetched_tree(root, 1024).unwrap_err();
        assert!(error.contains("install limit"), "{error}");
        std::fs::write(root.join("skills/SKILL.md"), "hello").unwrap();

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc/passwd", root.join("skills/leak")).unwrap();
            let error = validate_fetched_tree(root, 1024).unwrap_err();
            assert!(error.contains("symlink"), "{error}");
        }
    }

    /// A PATH entry that is not a directory, so a `git` lookup can hit the
    /// fake script below without shadowing the real git for other tests.
    static FAKE_GIT_SEQ: AtomicUsize = AtomicUsize::new(0);

    /// Install a fake `git` that reports the environment it was spawned
    /// with, then hangs, and returns (dir, marker path, marker path). The
    /// test runs the clone with only that directory on `PATH`.
    fn fake_hanging_git() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let seq = FAKE_GIT_SEQ.fetch_add(1, Ordering::SeqCst);
        let env_marker = dir
            .path()
            .join(format!("git-env-{seq}.txt"))
            .to_string_lossy()
            .to_string();
        let pid_marker = dir
            .path()
            .join(format!("git-pid-{seq}.txt"))
            .to_string_lossy()
            .to_string();
        let script = dir.path().join("git");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$GIT_TERMINAL_PROMPT\" > '{env_marker}'\necho $$ > '{pid_marker}'\ntrap 'rm -f \"$0.pid\"; exit 143' TERM\nwhile true; do sleep 0.1; done\n"
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        (dir, PathBuf::from(env_marker), PathBuf::from(pid_marker))
    }

    fn git_available() -> bool {
        std::process::Command::new("git")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    /// A committed local git repository with one skill; the second call also
    /// creates a `feature` branch holding a different skill.
    fn init_local_repo(path: &Path, with_feature_branch: bool) {
        std::fs::create_dir_all(path).unwrap();
        let skill_dir = path.join("skills").join("main-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: main-skill\ndescription: test\n---\n",
        )
        .unwrap();

        let git = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .current_dir(path)
                .args(args)
                .output()
                .unwrap();
            assert!(output.status.success(), "git {args:?}: {output:?}");
        };
        git(&["init", "--initial-branch=main"]);
        for args in [
            ["config", "user.email", "t@t"].as_slice(),
            ["config", "user.name", "t"].as_slice(),
            ["add", "-A"].as_slice(),
            ["commit", "-m", "init"].as_slice(),
        ] {
            git(args);
        }

        if with_feature_branch {
            for args in [
                ["checkout", "-b", "feature"].as_slice(),
                ["rm", "-r", "skills"].as_slice(),
            ] {
                git(args);
            }
            let feature_dir = path.join("skills").join("feature-skill");
            std::fs::create_dir_all(&feature_dir).unwrap();
            std::fs::write(
                feature_dir.join("SKILL.md"),
                "---\nname: feature-skill\ndescription: test\n---\n",
            )
            .unwrap();
            for args in [
                ["add", "-A"].as_slice(),
                ["commit", "-m", "feature"].as_slice(),
            ] {
                git(args);
            }
            git(&["checkout", "main"]);
        }
    }

    #[tokio::test]
    async fn clone_shallow_clones_a_local_repo_and_refuses_bad_refs() {
        if !git_available() {
            return;
        }
        let temp = tempfile::tempdir().unwrap();
        let origin = temp.path().join("origin");
        init_local_repo(&origin, false);

        // A ref that reads back as a git option is refused before spawning.
        let error = clone_git_repo(
            &origin.to_string_lossy(),
            Some("--upload-pack=evil"),
            &temp.path().join("bad"),
            Duration::from_secs(GIT_CLONE_TIMEOUT_SECS),
        )
        .await
        .unwrap_err();
        assert!(error.contains("Invalid git ref"), "{error}");

        let dest = temp.path().join("clone");
        clone_git_repo(
            &origin.to_string_lossy(),
            None,
            &dest,
            Duration::from_secs(GIT_CLONE_TIMEOUT_SECS),
        )
        .await
        .unwrap();
        assert!(dest
            .join("skills")
            .join("main-skill")
            .join("SKILL.md")
            .is_file());
    }

    #[tokio::test]
    async fn clone_checks_out_the_requested_ref() {
        // G1.6 end-to-end: the ref from `?ref=` selects the branch.
        if !git_available() {
            return;
        }
        let temp = tempfile::tempdir().unwrap();
        let origin = temp.path().join("origin");
        init_local_repo(&origin, true);

        let dest = temp.path().join("clone");
        clone_git_repo(
            &origin.to_string_lossy(),
            Some("feature"),
            &dest,
            Duration::from_secs(GIT_CLONE_TIMEOUT_SECS),
        )
        .await
        .unwrap();
        assert!(dest
            .join("skills")
            .join("feature-skill")
            .join("SKILL.md")
            .is_file());
        assert!(!dest.join("skills").join("main-skill").exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn clone_kills_a_hanging_git_on_timeout_and_disables_prompts() {
        // G1.2: the timeout must terminate the child (kill_on_drop), and the
        // clone must disable credential prompts instead of waiting on one.
        let (dir, env_marker, pid_marker) = fake_hanging_git();
        let old_path = std::env::var("PATH").unwrap_or_default();
        std::env::set_var("PATH", dir.path());
        let result = clone_git_repo(
            "https://git.example.com/o/r",
            None,
            &dir.path().join("dest"),
            Duration::from_millis(300),
        )
        .await;
        std::env::set_var("PATH", old_path);

        let error = result.unwrap_err();
        assert!(error.contains("timed out"), "{error}");
        assert_eq!(
            std::fs::read_to_string(&env_marker).unwrap().trim(),
            "0",
            "GIT_TERMINAL_PROMPT=0 must reach the child"
        );

        let pid = std::fs::read_to_string(&pid_marker)
            .unwrap()
            .trim()
            .parse::<i32>()
            .unwrap();
        let mut killed = false;
        for _ in 0..100 {
            let probe = std::process::Command::new("kill")
                .args(["-0", &pid.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|status| status.success())
                .unwrap_or(false);
            if !probe {
                killed = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(killed, "clone pid {pid} survived the timeout");
    }
}
