use crate::session::get_session_manager;
use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use walkdir::WalkDir;
use zip::{write::FileOptions, ZipWriter};

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
        // Get workspace directory via SessionManager
        let session_manager =
            get_session_manager().map_err(|e| format!("Session manager error: {e}"))?;
        let workspace_dir =
            crate::session::resolve_session_workspace_dir(session_manager, session_id).await?;

        // Resolve and validate path securely
        let full_path = crate::utils::security::resolve_secure_path(&workspace_dir, file_path)
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
        let workspace_dir =
            crate::session::resolve_session_workspace_dir(session_manager, session_id).await?;

        // Canonicalize base for stripping prefixes later
        let workspace_dir_canon = tokio::fs::canonicalize(&workspace_dir)
            .await
            .map_err(|e| format!("Failed to canonicalize workspace: {}", e))?;

        if files.is_empty() {
            return Err("Files array cannot be empty".to_string());
        }

        // Create a temporary ZIP file
        let temp_dir =
            tempfile::tempdir().map_err(|e| format!("Failed to create temp dir: {e}"))?;

        // Sanitize package_name to prevent path traversal via malicious characters
        let safe_package_name = package_name
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect::<String>();

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
            // Resolve path securely
            let source_path = match crate::utils::security::resolve_secure_path(
                &workspace_dir,
                file_path,
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

                if !abs_canon.starts_with(&workspace_dir_canon) {
                    continue;
                }

                let rel_path = match abs_canon.strip_prefix(&workspace_dir_canon) {
                    Ok(p) => p,
                    Err(_) => continue,
                };

                let archive_path = {
                    let p = rel_path.to_string_lossy().to_string();
                    #[cfg(target_os = "windows")]
                    let p = p.replace('\\', "/");
                    p
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
}
