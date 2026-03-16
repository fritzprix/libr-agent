use super::ReferenceResolver;
use crate::session::get_session_manager;
use async_trait::async_trait;
use tracing::warn;
use walkdir::WalkDir;

/// Resolves `@file:relative/path` references by reading the file from the session workspace.
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

    /// Reads `arg` as a relative path within the session workspace.
    /// Returns `None` if the file does not exist or cannot be read.
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

        // Guard: reject files that are too large to inject into context
        const MAX_INLINE_BYTES: u64 = 100 * 1024; // 100 KB
        let metadata = tokio::fs::metadata(&canonical_target).await.ok()?;
        let rel_path = rel.replace('\\', "/");
        if metadata.len() > MAX_INLINE_BYTES {
            return Some(format!(
                "# File `{}`\n\n⚠️ File is too large to inline ({} KB). Use `workspace__readFile` tool to read it.",
                rel_path,
                metadata.len() / 1024
            ));
        }

        // Guard: read raw bytes first to detect binary files
        let raw = tokio::fs::read(&canonical_target).await.ok()?;
        match String::from_utf8(raw) {
            Ok(content) => Some(format!(
                "# File `{}`\n\n```\n{}\n```",
                rel_path, content
            )),
            Err(_) => Some(format!(
                "# File `{}`\n\n⚠️ Binary file — cannot be inlined as text. Use `workspace__readFile` tool if needed.",
                rel_path
            )),
        }
    }
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

    let walker = WalkDir::new(&workspace)
        .max_depth(max_depth)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file());

    let mut paths: Vec<String> = Vec::new();
    for entry in walker {
        if let Ok(rel) = entry.path().strip_prefix(&workspace) {
            #[cfg(target_os = "windows")]
            paths.push(rel.to_string_lossy().replace('\\', "/"));
            #[cfg(not(target_os = "windows"))]
            paths.push(rel.to_string_lossy().to_string());
        }
    }

    Ok(paths)
}
