use crate::repositories::session_repository::SessionRepository;
use crate::session::get_session_manager;
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::{write::FileOptions, ZipWriter};

#[derive(Debug, Clone)]
pub struct SessionExportRoots {
    pub workspace_canon: PathBuf,
    pub teamwork_canon: Option<PathBuf>,
    pub skill_alias_roots: Vec<(&'static str, PathBuf)>,
}

impl SessionExportRoots {
    pub async fn resolve_for_session(
        session_manager: &crate::session::SessionManager,
        session_id: &str,
    ) -> Result<Self, String> {
        let workspace_dir =
            crate::session::resolve_session_workspace_dir(session_manager, session_id).await?;
        let workspace_canon = tokio::fs::canonicalize(&workspace_dir)
            .await
            .unwrap_or_else(|_| workspace_dir.clone());

        let teamwork_canon = match crate::session::resolve_teamwork_artifact_dir(
            session_manager,
            session_id,
        )
        .await
        {
            Ok(tw) => tokio::fs::canonicalize(&tw).await.ok().or(Some(tw)),
            Err(_) => None,
        };

        let assistant_id = if let Some(repo) = crate::state::try_get_session_repository() {
            repo.get_session(session_id)
                .await
                .ok()
                .flatten()
                .and_then(|s| crate::agent::extract_assistant_id_from_session(&s))
        } else {
            None
        };

        let skill_alias_roots = if let Ok((sys, usr, ast, wsk)) =
            crate::services::skill_service::resolve_skill_directories(
                assistant_id.as_deref(),
                Some(session_id),
                Some(&workspace_dir),
            )
            .await
        {
            let mut roots = Vec::new();
            for root in
                crate::services::skill_service::collect_skill_alias_roots(sys, usr, ast, wsk)
            {
                let canon = tokio::fs::canonicalize(&root.root)
                    .await
                    .unwrap_or_else(|_| root.root.clone());
                roots.push((root.prefix, canon));
            }
            roots
        } else {
            Vec::new()
        };

        Ok(Self {
            workspace_canon,
            teamwork_canon,
            skill_alias_roots,
        })
    }

    pub fn determine_archive_path(
        &self,
        abs_canon: &Path,
        original_path_hint: Option<&str>,
    ) -> Option<String> {
        // 1. If path hint explicitly references teamwork alias, check teamwork root first
        let is_teamwork_hint = original_path_hint
            .map(|h| {
                let trimmed = h.trim().trim_start_matches('/');
                trimmed.starts_with("@teamwork") || trimmed.starts_with(".libragent/teamwork")
            })
            .unwrap_or(false);

        if is_teamwork_hint {
            if let Some(tw_canon) = &self.teamwork_canon {
                if let Some(rel) =
                    crate::mcp::builtin::utils::relative_path_under_base(abs_canon, tw_canon)
                {
                    let prefix = if original_path_hint
                        .map(|h| {
                            h.trim()
                                .trim_start_matches('/')
                                .starts_with(".libragent/teamwork")
                        })
                        .unwrap_or(false)
                    {
                        ".libragent/teamwork"
                    } else {
                        "@teamwork"
                    };
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    let rel_clean = rel_str.trim_start_matches('/');
                    return Some(if rel_clean.is_empty() {
                        prefix.to_string()
                    } else {
                        format!("{}/{}", prefix, rel_clean)
                    });
                }
            }
        }

        // 2. Check skill alias hint
        if let Some(hint) = original_path_hint {
            if let Some((alias_prefix, _)) =
                crate::services::skill_service::extract_skill_alias_relative_path(hint)
            {
                for (prefix, root_canon) in &self.skill_alias_roots {
                    if *prefix == alias_prefix {
                        if let Some(rel) = crate::mcp::builtin::utils::relative_path_under_base(
                            abs_canon, root_canon,
                        ) {
                            let rel_str = rel.to_string_lossy().replace('\\', "/");
                            let rel_clean = rel_str.trim_start_matches('/');
                            return Some(if rel_clean.is_empty() {
                                prefix.to_string()
                            } else {
                                format!("{}/{}", prefix, rel_clean)
                            });
                        }
                    }
                }
            }
        }

        // 3. Check workspace root
        if let Some(rel) =
            crate::mcp::builtin::utils::relative_path_under_base(abs_canon, &self.workspace_canon)
        {
            if crate::mcp::builtin::workspace::utils::is_internal_workspace_artifact_path(
                &self.workspace_canon,
                abs_canon,
            ) {
                return None;
            }
            return Some(rel.to_string_lossy().replace('\\', "/"));
        }

        // 4. Check teamwork root (even without explicit hint)
        if let Some(tw_canon) = &self.teamwork_canon {
            if let Some(rel) =
                crate::mcp::builtin::utils::relative_path_under_base(abs_canon, tw_canon)
            {
                let prefix = if original_path_hint
                    .map(|h| {
                        h.trim()
                            .trim_start_matches('/')
                            .starts_with(".libragent/teamwork")
                    })
                    .unwrap_or(false)
                {
                    ".libragent/teamwork"
                } else {
                    "@teamwork"
                };
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                let rel_clean = rel_str.trim_start_matches('/');
                return Some(if rel_clean.is_empty() {
                    prefix.to_string()
                } else {
                    format!("{}/{}", prefix, rel_clean)
                });
            }
        }

        // 5. Check skill alias roots
        for (prefix, root_canon) in &self.skill_alias_roots {
            if let Some(rel) =
                crate::mcp::builtin::utils::relative_path_under_base(abs_canon, root_canon)
            {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                let rel_clean = rel_str.trim_start_matches('/');
                return Some(if rel_clean.is_empty() {
                    prefix.to_string()
                } else {
                    format!("{}/{}", prefix, rel_clean)
                });
            }
        }

        None
    }
}

pub struct FileExportService;

pub struct ExportedFile {
    pub filename: String,
    pub content: Vec<u8>,
    pub file_count: Option<usize>,
}

impl FileExportService {
    /// Reads a file from the workspace for export/download.
    pub async fn read_file_content(
        session_id: &str,
        file_path: &str,
    ) -> Result<ExportedFile, String> {
        // Resolve and validate path securely (supports @teamwork, @skills, workspace)
        let full_path =
            crate::services::WorkspaceService::resolve_path_for_session(session_id, file_path)
                .await
                .map_err(|e| format!("Access denied or file not found: {}", e))?;

        // Extract filename
        let filename = full_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("download")
            .to_string();

        // Read file content
        let max_size = crate::config::max_file_size() as u64;
        let content = crate::utils::fs::read_file_with_limit(&full_path, max_size)
            .await
            .map_err(|e| format!("Failed to read file: {e}"))?;

        Ok(ExportedFile {
            filename,
            content,
            file_count: None,
        })
    }

    /// Creates a ZIP archive from selected workspace files.
    pub async fn create_zip_export(
        session_id: &str,
        files: Vec<String>,
        package_name: &str,
    ) -> Result<ExportedFile, String> {
        let session_manager =
            get_session_manager().map_err(|e| format!("Session manager error: {e}"))?;
        let export_roots =
            SessionExportRoots::resolve_for_session(session_manager, session_id).await?;

        if files.is_empty() {
            return Err("Files array cannot be empty".to_string());
        }

        // Create a temporary ZIP file
        let temp_dir =
            tempfile::tempdir().map_err(|e| format!("Failed to create temp dir: {e}"))?;

        // Sanitize package_name to prevent path traversal via malicious characters
        let safe_package_name = Self::sanitize_package_name(package_name);

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let zip_filename = format!("{safe_package_name}_{timestamp}.zip");
        let temp_zip_path = temp_dir.path().join(&zip_filename);

        // Create the ZIP archive
        let zip_file = std::fs::File::create(&temp_zip_path)
            .map_err(|e| format!("Failed to create ZIP file: {e}"))?;

        let mut zip = ZipWriter::new(zip_file);
        let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        // Compute max size once for all file operations
        let max_size = crate::config::max_file_size() as u64;

        // Add files to the ZIP
        let mut processed_files = Vec::new();
        let mut added_archive_paths = HashSet::<String>::new();
        for file_path in &files {
            // Resolve path securely (supports @teamwork, @skills, workspace)
            let source_path = match crate::services::WorkspaceService::resolve_path_for_session(
                session_id, file_path,
            )
            .await
            {
                Ok(p) => p,
                Err(e) => {
                    log::warn!("Skipping invalid path {}: {}", file_path, e);
                    continue;
                }
            };

            let roots: Vec<PathBuf> = if source_path.is_file() {
                vec![source_path]
            } else if source_path.is_dir() {
                WalkDir::new(&source_path)
                    .into_iter()
                    .filter_map(Result::ok)
                    .filter(|e| {
                        !crate::mcp::builtin::workspace::utils::is_internal_workspace_artifact_path(
                            &export_roots.workspace_canon,
                            e.path(),
                        )
                    })
                    .filter(|e| e.file_type().is_file())
                    .map(|e| e.into_path())
                    .collect()
            } else {
                continue;
            };

            for abs_path in roots {
                let abs_canon = match std::fs::canonicalize(&abs_path) {
                    Ok(p) => p,
                    Err(_) => continue,
                };

                let archive_path =
                    match export_roots.determine_archive_path(&abs_canon, Some(file_path)) {
                        Some(p) => p,
                        None => continue,
                    };

                if !added_archive_paths.insert(archive_path.clone()) {
                    continue;
                }

                // Read file content first, before adding to ZIP
                let content =
                    match crate::utils::fs::read_file_with_limit(&abs_canon, max_size).await {
                        Ok(c) => c,
                        Err(e) => {
                            log::error!("Failed to read file {}: {e}", abs_canon.display());
                            continue;
                        }
                    };

                // Only add to ZIP after successful read
                if zip.start_file(&archive_path, options).is_err() {
                    continue;
                }

                if zip.write_all(&content).is_err() {
                    continue;
                }
                processed_files.push(archive_path);
            }
        }

        // Finalize the ZIP file
        zip.finish()
            .map_err(|e| format!("Failed to finalize ZIP: {e}"))?;

        if processed_files.is_empty() {
            return Err("No files were successfully added to ZIP".to_string());
        }

        // Read ZIP content
        let zip_content = crate::utils::fs::read_file_with_limit(&temp_zip_path, max_size)
            .await
            .map_err(|e| format!("Failed to read ZIP file: {e}"))?;

        Ok(ExportedFile {
            filename: zip_filename,
            content: zip_content,
            file_count: Some(processed_files.len()),
        })
    }

    /// Sanitizes package_name to prevent path traversal via malicious characters.
    pub fn sanitize_package_name(package_name: &str) -> String {
        let sanitized: String = package_name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        // If the resulting name is empty or consists purely of underscores,
        // fall back to a safe default "workspace_export".
        let has_content = sanitized.chars().any(|c| c != '_');
        if has_content {
            sanitized
        } else {
            "workspace_export".to_string()
        }
    }
}
