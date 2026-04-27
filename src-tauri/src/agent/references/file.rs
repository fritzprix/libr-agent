use super::ReferenceResolver;
use crate::session::get_session_manager;
use async_trait::async_trait;
use std::path::Path;
use tracing::warn;
use walkdir::WalkDir;

/// Resolves `@file:relative/path` references by pointing the agent at a workspace file
/// without inlining the full file contents into prompt context.
pub struct FileReferenceResolver {
    session_id: String,
}

impl FileReferenceResolver {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
        }
    }
}

#[async_trait]
impl ReferenceResolver for FileReferenceResolver {
    fn type_name(&self) -> &'static str {
        "file"
    }

    /// Resolves `arg` as a relative path within the session workspace.
    /// Returns a compact metadata block with targeted read guidance instead of
    /// inlining the entire file body.
    async fn resolve(&self, arg: &str) -> Option<String> {
        let session_manager = get_session_manager()
            .map_err(|e| warn!("FileResolver: {e}"))
            .ok()?;
        let workspace = session_manager.get_session_workspace_dir_by_id(&self.session_id);

        // Sanitize: strip leading slashes / dots to stay inside workspace
        let rel = arg.trim_start_matches('/').trim_start_matches("./");
        let target = workspace.join(rel);

        // Reject path traversal attempts
        let canonical_workspace = workspace.canonicalize().ok()?;
        let canonical_target = target.canonicalize().ok()?;
        if !canonical_target.starts_with(&canonical_workspace) {
            warn!("FileResolver: path traversal attempt blocked: {}", arg);
            return None;
        }

        if !canonical_target.is_file() {
            return None;
        }

        let metadata = tokio::fs::metadata(&canonical_target).await.ok()?;
        let rel_path = {
            let p = rel.to_string();
            #[cfg(target_os = "windows")]
            let p = p.replace('\\', "/");
            p
        };
        let file_size_bytes = metadata.len();
        let extension = canonical_target
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        Some(format!(
            "# File Reference `{}`\n\n\
             The file content was not inlined to avoid unnecessary context usage.\n\n\
             - Relative path: `{}`\n\
             - File size: {} bytes\n\
             - Extension: `{}`\n\
             - To inspect it, call: `workspace__readFile(path: \"{}\")`\n\
             - Prefer reading only the relevant line range or searching before loading more content.",
            rel_path,
            rel_path,
            file_size_bytes,
            if extension.is_empty() { "(none)" } else { extension },
            rel_path
        ))
    }
}

fn collect_relative_file_paths(root: &Path, max_depth: usize) -> Vec<String> {
    let walker = WalkDir::new(root)
        .max_depth(max_depth)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file());

    let mut paths: Vec<String> = Vec::new();
    for entry in walker {
        if let Ok(rel) = entry.path().strip_prefix(root) {
            paths.push({
                let p = rel.to_string_lossy().to_string();
                #[cfg(target_os = "windows")]
                let p = p.replace('\\', "/");
                p
            });
        }
    }

    paths.sort();
    paths
}

/// Returns a flat list of relative file paths for an arbitrary workspace root.
pub async fn list_relative_paths_in_root(
    root: &Path,
    max_depth: usize,
) -> Result<Vec<String>, String> {
    let metadata = tokio::fs::metadata(root)
        .await
        .map_err(|error| format!("Workspace path is not accessible: {error}"))?;

    if !metadata.is_dir() {
        return Err("Workspace path must be a directory".to_string());
    }

    Ok(collect_relative_file_paths(root, max_depth))
}

/// Returns a flat list of relative file paths in the workspace (non-recursive directories excluded).
/// Used by the Tauri command for frontend autocomplete candidates.
pub async fn list_workspace_relative_paths(
    session_id: &str,
    max_depth: usize,
) -> Result<Vec<String>, String> {
    let session_manager =
        get_session_manager().map_err(|e| format!("Session manager error: {e}"))?;
    let workspace = session_manager.get_session_workspace_dir_by_id(session_id);

    list_relative_paths_in_root(&workspace, max_depth).await
}
