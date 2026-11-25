use super::super::contracts::{
    PreparedSkillImport, GITHUB_DOWNLOAD_CONNECT_TIMEOUT_SECS, GITHUB_DOWNLOAD_TIMEOUT_SECS,
    MAX_GITHUB_ARCHIVE_BYTES,
};
use super::super::github::parse_github_repo_url;
use super::super::scanning::{copy_dir_recursive, discover_skill_roots};
use crate::session::get_session_manager;
use reqwest::header::USER_AGENT;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

pub(super) fn build_temp_root() -> Result<PathBuf, String> {
    let session_manager = get_session_manager()?;
    let temp_root = session_manager
        .get_base_data_dir()
        .join("temp_skill_imports");
    fs::create_dir_all(&temp_root).map_err(|error| error.to_string())?;
    Ok(temp_root)
}

pub(super) async fn prepare_local_skill_import(
    file_path: String,
) -> Result<PreparedSkillImport, String> {
    let temp_root = build_temp_root()?;
    tokio::task::spawn_blocking(move || prepare_local_skill_import_blocking(&temp_root, &file_path))
        .await
        .map_err(|error| format!("Task join error: {}", error))?
}

pub(in crate::services::skill_service) fn prepare_local_skill_import_blocking(
    temp_root: &Path,
    file_path: &str,
) -> Result<PreparedSkillImport, String> {
    let temp_dir = tempfile::Builder::new()
        .prefix("skill-import-")
        .tempdir_in(temp_root)
        .map_err(|error| error.to_string())?;
    let staging_dir = temp_dir.path().join("staging");
    fs::create_dir_all(&staging_dir).map_err(|error| error.to_string())?;

    let src_path = PathBuf::from(file_path);
    if !src_path.exists() {
        return Err(format!("Source path does not exist: {}", file_path));
    }

    if src_path.is_file() {
        let extension = src_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase());
        match extension.as_deref() {
            Some("zip") | Some("skill") => {
                let file = fs::File::open(&src_path).map_err(|error| error.to_string())?;
                let mut archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
                crate::utils::fs::extract_zip_secure(&mut archive, &staging_dir)?;
            }
            _ => return Err("Only .skill, .zip files, or directories are supported".to_string()),
        }
    } else if src_path.is_dir() {
        copy_dir_recursive(&src_path, &staging_dir)?;
    } else {
        return Err("Invalid source path".to_string());
    }

    let discovered_skills = discover_skill_roots(&staging_dir)?;
    Ok(PreparedSkillImport {
        _temp_dir: temp_dir,
        scan_root: staging_dir,
        source_key: file_path.to_string(),
        discovered_skills,
    })
}

pub(in crate::services::skill_service) async fn prepare_github_skill_import(
    repo_url: String,
) -> Result<PreparedSkillImport, String> {
    let source_key = repo_url.clone();
    let repo_spec = parse_github_repo_url(&repo_url)?;
    let temp_root = build_temp_root()?;
    let temp_dir = tempfile::Builder::new()
        .prefix("github-skill-import-")
        .tempdir_in(&temp_root)
        .map_err(|error| error.to_string())?;
    let archive_path = temp_dir.path().join(format!("{}.zip", Uuid::new_v4()));
    let extract_dir = temp_dir.path().join("extract");
    tokio::fs::create_dir_all(&extract_dir)
        .await
        .map_err(|error| error.to_string())?;

    let archive_url = if let Some(branch) = &repo_spec.branch {
        format!(
            "https://codeload.github.com/{}/{}/zip/refs/heads/{}",
            repo_spec.owner, repo_spec.repo, branch
        )
    } else {
        format!(
            "https://api.github.com/repos/{}/{}/zipball",
            repo_spec.owner, repo_spec.repo
        )
    };

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(GITHUB_DOWNLOAD_CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(GITHUB_DOWNLOAD_TIMEOUT_SECS))
        .build()
        .map_err(|error| format!("Failed to build GitHub download client: {}", error))?;
    let mut response = client
        .get(archive_url)
        .header(USER_AGENT, "LibrAgent Skills Installer")
        .send()
        .await
        .map_err(|error| format!("Failed to download GitHub repository: {}", error))?
        .error_for_status()
        .map_err(|error| format!("GitHub repository download failed: {}", error))?;

    if let Some(content_length) = response.content_length() {
        if content_length > MAX_GITHUB_ARCHIVE_BYTES {
            return Err(format!(
                "GitHub repository archive is too large ({} bytes). Maximum allowed size is {} MB.",
                content_length,
                MAX_GITHUB_ARCHIVE_BYTES / 1024 / 1024
            ));
        }
    }

    let mut archive_file = tokio::fs::File::create(&archive_path)
        .await
        .map_err(|error| error.to_string())?;
    let mut downloaded_bytes = 0_u64;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("Failed to read GitHub archive: {}", error))?
    {
        downloaded_bytes = downloaded_bytes
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| "GitHub archive size overflowed the download counter".to_string())?;
        if downloaded_bytes > MAX_GITHUB_ARCHIVE_BYTES {
            return Err(format!(
                "GitHub repository archive exceeded the {} MB download limit.",
                MAX_GITHUB_ARCHIVE_BYTES / 1024 / 1024
            ));
        }
        archive_file
            .write_all(&chunk)
            .await
            .map_err(|error| format!("Failed to write GitHub archive to disk: {}", error))?;
    }
    archive_file
        .flush()
        .await
        .map_err(|error| format!("Failed to finalize GitHub archive on disk: {}", error))?;

    tokio::task::spawn_blocking(move || {
        let file = fs::File::open(&archive_path).map_err(|error| error.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
        crate::utils::fs::extract_zip_secure(&mut archive, &extract_dir)?;

        let repo_root = find_archive_root(&extract_dir)?;
        let scan_root = if let Some(subpath) = &repo_spec.subpath {
            let candidate = repo_root.join(subpath);
            if !candidate.exists() {
                return Err(format!(
                    "GitHub path '{}' was not found in the downloaded repository",
                    subpath.display()
                ));
            }
            candidate
        } else {
            repo_root
        };

        let discovered_skills = discover_skill_roots(&scan_root)?;
        Ok(PreparedSkillImport {
            _temp_dir: temp_dir,
            scan_root,
            source_key,
            discovered_skills,
        })
    })
    .await
    .map_err(|error| format!("Task join error: {}", error))?
}

fn find_archive_root(extract_dir: &Path) -> Result<PathBuf, String> {
    let mut child_dirs = fs::read_dir(extract_dir)
        .map_err(|error| error.to_string())?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();

    child_dirs.sort();
    if child_dirs.len() == 1 {
        Ok(child_dirs.remove(0))
    } else {
        Ok(extract_dir.to_path_buf())
    }
}

/// Hook config files for a plugin root, in load order: the auto-loaded
/// `hooks/hooks.json` / `hooks-cursor.json` plus the files the manifest's
/// `hooks` field declares (spec plugins-reference add-semantics). A manifest
/// entry duplicating the default or missing on disk is an error (S1.1).
/// Install persists these paths (relative to the root) so dispatch and the
pub(super) fn commit_from_archive_root(scan_root: &Path) -> Option<String> {
    for ancestor in scan_root.ancestors().take(3) {
        let Some(name) = ancestor.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some((_, sha)) = name.rsplit_once('-') else {
            continue;
        };
        if sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(sha.to_ascii_lowercase());
        }
    }
    None
}
