//! HTTP and workspace-local media byte loading.

use std::path::Path;

use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::types::MCPResult;

/// Maximum allowed download size (20 MB).
pub(super) const MAX_BYTES: usize = 20 * 1024 * 1024;

/// Fetch bytes from an HTTP/HTTPS URL.
/// Returns `(bytes, content_type_header_value)`.
pub(super) async fn fetch_http_bytes(url: &str) -> Result<(Vec<u8>, Option<String>), String> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; LibrAgent/1.0)")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {} for URL: {}", response.status(), url));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Check Content-Length before downloading to avoid excessive memory use.
    if let Some(len) = response.content_length() {
        if len as usize > MAX_BYTES {
            return Err(format!(
                "Remote file is too large ({} bytes). Maximum allowed size is {} MB.",
                len,
                MAX_BYTES / 1024 / 1024
            ));
        }
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response body: {e}"))?;

    if bytes.len() > MAX_BYTES {
        return Err(format!(
            "Downloaded content is too large ({} bytes). Maximum allowed size is {} MB.",
            bytes.len(),
            MAX_BYTES / 1024 / 1024
        ));
    }

    Ok((bytes.to_vec(), content_type))
}

/// Read bytes from a local file, enforcing the size cap.
pub(super) async fn read_local_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|e| format!("Cannot access file '{}': {e}", path.display()))?;

    if metadata.len() as usize > MAX_BYTES {
        return Err(format!(
            "File is too large ({} bytes). Maximum allowed size is {} MB.",
            metadata.len(),
            MAX_BYTES / 1024 / 1024
        ));
    }

    tokio::fs::read(path)
        .await
        .map_err(|e| format!("Failed to read file '{}': {e}", path.display()))
}

/// Resolve a local path against the session workspace.
///
/// Relative paths join `workspace_dir`. Absolute Docker workdir paths (e.g. `/app/…`)
/// are remapped to the host workspace when the session is Docker-isolated.
async fn resolve_media_local_path(
    path: &Path,
    workspace_dir: &Path,
    session_id: &str,
) -> Result<std::path::PathBuf, String> {
    let path_str = path.to_string_lossy();
    if let Some(mapped) =
        crate::session_isolation::map_docker_container_file_tool_path(session_id, &path_str).await?
    {
        return Ok(mapped);
    }
    Ok(resolve_local_path(path, workspace_dir))
}

/// Resolve, (for attach mode) sync, and workspace-bound a local media file path.
async fn prepare_local_media_path(
    path: &Path,
    workspace_dir: &Path,
    session_id: &str,
) -> Result<std::path::PathBuf, String> {
    let resolved = resolve_media_local_path(path, workspace_dir, session_id).await?;
    // Pull before canonicalize so attach-mode files that exist only in the
    // container become visible on the host staging workspace.
    if let Some(session) = crate::services::container_attach_fs::load_session(session_id).await? {
        crate::services::container_attach_fs::pull_container_file_to_host(&session, &resolved)
            .await?;
    }
    ensure_within_workspace(&resolved, workspace_dir)?;
    Ok(resolved)
}

/// Resolve a local path against the session workspace when it is relative.
pub(super) fn resolve_local_path(path: &Path, workspace_dir: &Path) -> std::path::PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_dir.join(path)
    }
}

/// Validate that a resolved local path does not escape the workspace.
fn ensure_within_workspace(path: &Path, workspace_dir: &Path) -> Result<(), String> {
    let canonical_path = path
        .canonicalize()
        .map_err(|e| format!("Cannot resolve path '{}': {e}", path.display()))?;
    let canonical_workspace = workspace_dir
        .canonicalize()
        .map_err(|e| format!("Cannot resolve workspace directory: {e}"))?;

    if canonical_path.starts_with(&canonical_workspace) {
        Ok(())
    } else {
        Err(format!(
            "Access denied: '{}' is outside the session workspace.",
            path.display()
        ))
    }
}

pub(super) fn media_local_path_error_category(error: &str) -> ErrorCategory {
    if error.contains(crate::session_isolation::OUTSIDE_DOCKER_WORKDIR_FILE_TOOL_MARKER)
        || error.contains("outside the session workspace")
    {
        ErrorCategory::PermissionDenied
    } else if error.contains("Cannot resolve path") || error.contains("Failed to read file") {
        ErrorCategory::ResourceNotFound
    } else {
        ErrorCategory::OperationFailed
    }
}

/// Map/sync a workspace-local media path and read its bytes.
pub(super) async fn read_workspace_media_bytes(
    raw_path: &Path,
    workspace_dir: &Path,
    session_id: &str,
) -> Result<Vec<u8>, MCPResult> {
    let resolved = match prepare_local_media_path(raw_path, workspace_dir, session_id).await {
        Ok(path) => path,
        Err(e) => {
            return Err(
                guided_error(media_local_path_error_category(&e), e, ToolGroup::Media)
                    .to_mcp_result(),
            );
        }
    };
    match read_local_bytes(&resolved).await {
        Ok(data) => Ok(data),
        Err(e) => {
            Err(guided_error(ErrorCategory::ResourceNotFound, e, ToolGroup::Media).to_mcp_result())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::builtin::error_guidance::ErrorCategory;
    use crate::session_isolation::PathMappingLayer;
    use std::path::PathBuf;

    #[test]
    fn relative_local_path_joins_workspace() {
        let workspace = PathBuf::from("/tmp/ws");
        let resolved = resolve_local_path(Path::new("image.gif"), &workspace);
        assert_eq!(resolved, workspace.join("image.gif"));
    }

    #[test]
    fn docker_workdir_absolute_maps_like_workspace_tools() {
        let host = PathBuf::from("/tmp/staging");
        let mapper = PathMappingLayer::with_container_root(host.clone(), "/app");
        assert_eq!(
            mapper.container_to_host("/app/image.gif"),
            Some(host.join("image.gif"))
        );
        assert_eq!(mapper.container_to_host("/logs/artifacts/x"), None);
    }

    #[test]
    fn outside_workdir_error_is_permission_denied() {
        let err = format!(
            "Docker container path '/logs/x' is outside /app. Shell commands may access it, but {} /app paths to the host workspace.",
            crate::session_isolation::OUTSIDE_DOCKER_WORKDIR_FILE_TOOL_MARKER
        );
        assert_eq!(
            media_local_path_error_category(&err),
            ErrorCategory::PermissionDenied
        );
    }
}
