//! Install-source routing for plugin/skill installs (#2928).
//!
//! One source string can be a github.com URL, any other http(s) git URL, a
//! `file://` git URL, or a local path (`.skill`, `.zip`, or directory).
//! `prepare_plugin_source_import` classifies the string and reuses the
//! matching fetcher; the github.com fast path and the local-import path are
//! the existing ones, untouched.

use super::super::contracts::{PreparedSkillImport, GIT_CLONE_TIMEOUT_SECS};
use super::super::scanning::discover_skill_roots;
use super::archive::{build_temp_root, prepare_github_skill_import, prepare_local_skill_import};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use url::Url;
use uuid::Uuid;

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

    let parsed = Url::parse(trimmed).map_err(|error| format!("Invalid install URL: {error}"))?;
    match parsed.scheme() {
        "http" | "https" => {
            let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
            if host == "github.com" || host == "www.github.com" {
                return Ok(InstallSourceKind::GitHub);
            }
            let git_ref = parsed
                .query_pairs()
                .into_owned()
                .find(|(key, value)| key == "ref" && !value.is_empty())
                .map(|(_, value)| value);
            let mut clone_url = parsed.clone();
            clone_url.set_query(None);
            clone_url.set_fragment(None);
            Ok(InstallSourceKind::Git {
                url: clone_url.to_string(),
                git_ref,
            })
        }
        "file" => {
            let path = parsed
                .to_file_path()
                .map_err(|_| "file:// install source must point to a local path".to_string())?;
            Ok(InstallSourceKind::Git {
                url: path.to_string_lossy().to_string(),
                git_ref: None,
            })
        }
        scheme => Err(format!("Unsupported install source scheme '{scheme}'")),
    }
}

/// Shallow-clone one git URL into `dest`. Arguments are passed as a list
/// (never through a shell), `--` ends option parsing, and a `ref` starting
/// with `-` is rejected so neither can be read back as a git option.
pub(super) async fn clone_git_repo(
    clone_url: &str,
    git_ref: Option<&str>,
    dest: &Path,
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
        .stderr(Stdio::piped());
    let output = tokio::time::timeout(
        Duration::from_secs(GIT_CLONE_TIMEOUT_SECS),
        command.output(),
    )
    .await
    .map_err(|_| format!("git clone timed out after {GIT_CLONE_TIMEOUT_SECS}s"))?
    .map_err(|error| format!("Failed to run git (is git installed?): {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git clone failed: {}", stderr.trim()));
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
) -> Result<PreparedSkillImport, String> {
    let temp_root = build_temp_root()?;
    let temp_dir = tempfile::Builder::new()
        .prefix("git-skill-import-")
        .tempdir_in(&temp_root)
        .map_err(|error| error.to_string())?;
    let staging_dir = temp_dir.path().join(Uuid::new_v4().to_string());
    clone_git_repo(&clone_url, git_ref.as_deref(), &staging_dir).await?;

    let scan_root = staging_dir;
    let scan_dir = scan_root.clone();
    let discovered_skills = tokio::task::spawn_blocking(move || {
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

/// Fetch one install source by routing it to the GitHub zipball fast path,
/// the generic git clone path, or the existing local importer (#2928).
pub(in crate::services::skill_service) async fn prepare_plugin_source_import(
    source: String,
) -> Result<PreparedSkillImport, String> {
    match classify_install_source(&source)? {
        InstallSourceKind::GitHub => prepare_github_skill_import(source).await,
        InstallSourceKind::Git { url, git_ref } => {
            prepare_git_skill_import(source, url, git_ref).await
        }
        InstallSourceKind::Local => prepare_local_skill_import(source).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn rejects_unsupported_schemes() {
        let error = classify_install_source("ssh://git.example.com/o/r").unwrap_err();
        assert!(
            error.contains("Unsupported install source scheme"),
            "{error}"
        );
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

    #[tokio::test]
    async fn clone_shallow_clones_a_local_repo_and_refuses_bad_refs() {
        if !git_available() {
            return;
        }
        let temp = tempfile::tempdir().unwrap();
        let origin = temp.path().join("origin");
        let skill_dir = origin.join("skills").join("cloned-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: cloned-skill\ndescription: test\n---\n",
        )
        .unwrap();

        let git_output = std::process::Command::new("git")
            .current_dir(&origin)
            .args(["init", "--initial-branch=main"])
            .output()
            .unwrap();
        assert!(git_output.status.success());
        for args in [
            vec!["-c", "user.email=t@t", "-c", "user.name=t", "add", "-A"],
            vec![
                "-c",
                "user.email=t@t",
                "-c",
                "user.name=t",
                "commit",
                "-m",
                "init",
            ],
        ] {
            let output = std::process::Command::new("git")
                .current_dir(&origin)
                .args(&args)
                .output()
                .unwrap();
            assert!(output.status.success(), "git {args:?}: {output:?}");
        }

        // A ref that reads back as a git option is refused before spawning.
        let error = clone_git_repo(
            &origin.to_string_lossy(),
            Some("--upload-pack=evil"),
            &temp.path().join("bad"),
        )
        .await
        .unwrap_err();
        assert!(error.contains("Invalid git ref"), "{error}");

        let dest = temp.path().join("clone");
        clone_git_repo(&origin.to_string_lossy(), None, &dest)
            .await
            .unwrap();
        assert!(dest
            .join("skills")
            .join("cloned-skill")
            .join("SKILL.md")
            .is_file());
    }
}
