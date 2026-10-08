// helpers.rs - Utility functions
use crate::mcp::builtin::utils::relative_path_under_base;
use std::path::Path;

/// Extract file path from file:// URL
///
/// This function properly handles file URLs across Windows and Unix platforms,
/// including URL encoding (spaces, Unicode characters), drive letters (Windows),
/// and UNC paths. It uses the url crate to ensure proper parsing and conversion.
///
/// # Examples
/// - `file:///C:/Users/Me/file.txt` -> `C:\Users\Me\file.txt` (Windows)
/// - `file:///home/user/file.txt` -> `/home/user/file.txt` (Unix)
/// - `file:///C:/My%20Files/doc.txt` -> `C:\My Files\doc.txt` (URL decoding)
/// - `file://localhost/C:/file.txt` -> `C:\file.txt` (with host)
pub fn extract_file_path_from_url(file_url: &str) -> Result<String, String> {
    let url = url::Url::parse(file_url).map_err(|e| format!("Invalid file URL format: {e}"))?;

    // Ensure it's a file:// URL
    if url.scheme() != "file" {
        return Err(format!(
            "URL must use file:// scheme, got: {}",
            url.scheme()
        ));
    }

    // Convert to OS-specific file path (handles Windows drive letters, UNC, URL decoding, etc.)
    url.to_file_path()
        .map_err(|_| "URL cannot be converted to a local file path".to_string())?
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Failed to convert path to UTF-8 string".to_string())
}

/// True when `path` canonicalizes inside `workspace`. Fail closed if either side
/// cannot be resolved.
pub fn path_is_within_workspace(path: &Path, workspace: &Path) -> bool {
    let Ok(canonical_workspace) = workspace.canonicalize() else {
        return false;
    };
    let Ok(canonical_path) = path.canonicalize() else {
        return false;
    };
    relative_path_under_base(&canonical_path, &canonical_workspace).is_some()
}

/// Derive a session-relative workspace path from an in-workspace `file://` URL.
///
/// Returns forward-slash relative paths agents can pass to workspace tools
/// (e.g. `attachments/123_doc.pdf`). Host absolute paths are never returned.
pub fn exposed_workspace_path_from_src_url(src_url: &str, workspace: &Path) -> Option<String> {
    if !is_file_url(src_url) {
        return None;
    }
    let absolute = extract_file_path_from_url(src_url).ok()?;
    let Ok(canonical_path) = Path::new(&absolute).canonicalize() else {
        return None;
    };
    let Ok(canonical_workspace) = workspace.canonicalize() else {
        return None;
    };
    let relative = relative_path_under_base(&canonical_path, &canonical_workspace)?;
    if relative.as_os_str().is_empty() {
        return None;
    }
    Some(relative.to_string_lossy().replace('\\', "/"))
}

fn is_file_url(url_str: &str) -> bool {
    url::Url::parse(url_str)
        .map(|u| u.scheme().eq_ignore_ascii_case("file"))
        .unwrap_or(false)
}

fn is_remote_provenance_url(url_str: &str) -> bool {
    url::Url::parse(url_str)
        .map(|u| matches!(u.scheme(), "http" | "https"))
        .unwrap_or(false)
}

/// Agent-facing `srcUrl`: keep remote http(s) provenance only. Never echo
/// `file://`, raw host paths, or browser pseudo-URLs — agents should use
/// `workspacePath` for local copies.
pub fn exposed_src_url(src_url: Option<&str>) -> Option<String> {
    let src = src_url?;
    if is_remote_provenance_url(src) {
        Some(src.to_string())
    } else {
        None
    }
}

/// Choose a persistable `src_url` from optional MCP `srcUrl` / UI `fileUrl`.
///
/// Prefers in-workspace `file://` so list/read can derive a relative
/// `workspacePath`. Otherwise keeps a remote http(s) provenance URL.
/// Out-of-workspace `file://`, raw paths, and blob/data URLs are dropped
/// (UI may still read a host `fileUrl` for indexing).
pub fn persistable_attachment_src_url(
    src_url: Option<&str>,
    file_url: Option<&str>,
    workspace: &Path,
) -> Option<String> {
    for candidate in [file_url, src_url].into_iter().flatten() {
        if is_file_url(candidate)
            && exposed_workspace_path_from_src_url(candidate, workspace).is_some()
        {
            return Some(candidate.to_string());
        }
    }
    for candidate in [src_url, file_url].into_iter().flatten() {
        if is_remote_provenance_url(candidate) {
            return Some(candidate.to_string());
        }
    }
    None
}

/// Determine MIME type from file extension
pub(crate) fn mime_type_from_extension(path: &Path) -> &'static str {
    match path.extension() {
        Some(ext) => match ext.to_str().unwrap_or("").to_lowercase().as_str() {
            "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "pdf" => "application/pdf",
            "txt" => "text/plain",
            "md" => "text/markdown",
            "csv" => "text/csv",
            _ => "text/plain",
        },
        None => "text/plain",
    }
}

/// Create text chunks from content lines
pub(crate) fn create_text_chunks(lines: &[&str], chunk_size: usize) -> Vec<String> {
    lines
        .chunks(chunk_size)
        .map(|chunk| chunk.join("\n"))
        .collect()
}
