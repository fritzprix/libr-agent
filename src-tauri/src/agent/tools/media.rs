use crate::mcp::types::MCPContent;
use crate::services::WorkspaceService;
use base64::engine::general_purpose;
use base64::Engine;
use sha2::{Digest, Sha256};
use std::path::Path;
use url::Url;

const TOOL_RESULT_MEDIA_DIR: &str = ".libragent/tool-results/media";

fn media_extension_for_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        "image/svg+xml" => "svg",
        "audio/mpeg" => "mp3",
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/ogg" => "ogg",
        "audio/aac" => "aac",
        "audio/flac" => "flac",
        "audio/webm" => "webm",
        _ => "bin",
    }
}

fn media_relative_path_from_bytes(bytes: &[u8], mime_type: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = format!("{:x}", hasher.finalize());
    format!(
        "{}/{}.{}",
        TOOL_RESULT_MEDIA_DIR,
        digest,
        media_extension_for_mime(mime_type)
    )
}

async fn persist_tool_result_media(
    session_id: &str,
    bytes: &[u8],
    mime_type: &str,
) -> Result<String, String> {
    let relative_path = media_relative_path_from_bytes(bytes, mime_type);
    WorkspaceService::workspace_write_file(&relative_path, bytes, Some(session_id.to_string()))
        .await
        .map_err(|error| {
            format!(
                "Failed to persist tool-result media '{}' for session {}: {}",
                relative_path, session_id, error
            )
        })?;

    let session_manager = crate::session::get_session_manager()?;
    let absolute_path = session_manager
        .get_session_workspace_dir_by_id(session_id)
        .join(&relative_path);

    Url::from_file_path(&absolute_path)
        .map(|url| url.to_string())
        .map_err(|_| {
            format!(
                "Failed to convert persisted media path '{}' into file URL",
                absolute_path.display()
            )
        })
}

/// Reuse a tool-provided `file://` URI when it resolves inside the session workspace.
///
/// Builtin tools may already persist bytes (e.g. screenshots) and attach a workspace-scoped
/// URI so download can avoid re-shipping base64. Without this reuse path,
/// `externalize_media_content_for_storage` would discard that URI, rewrite under
/// `.libragent/tool-results/media/`, and leave an orphaned copy behind.
pub(crate) fn reuse_workspace_file_uri_if_valid(uri: &str, workspace_dir: &Path) -> Option<String> {
    let parsed = Url::parse(uri).ok()?;
    if parsed.scheme() != "file" {
        return None;
    }
    let path = parsed.to_file_path().ok()?;
    let canonical_workspace = std::fs::canonicalize(workspace_dir).ok()?;
    let canonical_file = std::fs::canonicalize(&path).ok()?;
    if !canonical_file.starts_with(&canonical_workspace) {
        return None;
    }
    Url::from_file_path(&canonical_file)
        .ok()
        .map(|url| url.to_string())
}

async fn externalize_inline_media(
    session_id: &str,
    data: String,
    uri: Option<String>,
    mime_type: String,
    kind: MediaKind,
) -> Result<MCPContent, String> {
    if let Some(existing_uri) = uri.as_deref() {
        if let Ok(session_manager) = crate::session::get_session_manager() {
            let workspace_dir = session_manager.get_session_workspace_dir_by_id(session_id);
            if let Some(reused) = reuse_workspace_file_uri_if_valid(existing_uri, &workspace_dir) {
                return Ok(kind.with_uri(reused, mime_type));
            }
        }
    }

    let bytes = general_purpose::STANDARD.decode(&data).map_err(|error| {
        format!(
            "Failed to decode {} payload for session {}: {}",
            kind.label(),
            session_id,
            error
        )
    })?;
    let file_url = persist_tool_result_media(session_id, &bytes, &mime_type).await?;
    Ok(kind.with_uri(file_url, mime_type))
}

enum MediaKind {
    Image,
    Audio,
}

impl MediaKind {
    fn label(&self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Audio => "audio",
        }
    }

    fn with_uri(&self, uri: String, mime_type: String) -> MCPContent {
        match self {
            Self::Image => MCPContent::Image {
                data: None,
                uri: Some(uri),
                mime_type,
            },
            Self::Audio => MCPContent::Audio {
                data: None,
                uri: Some(uri),
                mime_type,
            },
        }
    }
}

pub async fn externalize_media_content_for_storage(
    session_id: &str,
    content: Vec<MCPContent>,
) -> Result<Vec<MCPContent>, String> {
    let mut next_content = Vec::with_capacity(content.len());

    for item in content {
        match item {
            MCPContent::Image {
                data: Some(data),
                uri,
                mime_type,
            } => {
                next_content.push(
                    externalize_inline_media(session_id, data, uri, mime_type, MediaKind::Image)
                        .await?,
                );
            }
            MCPContent::Audio {
                data: Some(data),
                uri,
                mime_type,
            } => {
                next_content.push(
                    externalize_inline_media(session_id, data, uri, mime_type, MediaKind::Audio)
                        .await?,
                );
            }
            other => next_content.push(other),
        }
    }

    Ok(next_content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn reuse_workspace_file_uri_accepts_in_workspace_file() {
        let temp = tempfile::tempdir().unwrap();
        let media_dir = temp.path().join(TOOL_RESULT_MEDIA_DIR);
        fs::create_dir_all(&media_dir).unwrap();
        let file_path = media_dir.join("shot.png");
        let mut file = fs::File::create(&file_path).unwrap();
        file.write_all(b"png-bytes").unwrap();

        let uri = Url::from_file_path(&file_path).unwrap().to_string();
        let reused = reuse_workspace_file_uri_if_valid(&uri, temp.path()).expect("should reuse");
        let reused_path = Url::parse(&reused).unwrap().to_file_path().unwrap();
        assert_eq!(
            fs::canonicalize(&reused_path).unwrap(),
            fs::canonicalize(&file_path).unwrap()
        );
    }

    #[test]
    fn reuse_workspace_file_uri_rejects_outside_workspace() {
        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let file_path = outside.path().join("escape.png");
        fs::write(&file_path, b"png-bytes").unwrap();

        let uri = Url::from_file_path(&file_path).unwrap().to_string();
        assert!(reuse_workspace_file_uri_if_valid(&uri, workspace.path()).is_none());
    }

    #[test]
    fn reuse_workspace_file_uri_rejects_non_file_schemes() {
        let temp = tempfile::tempdir().unwrap();
        assert!(
            reuse_workspace_file_uri_if_valid("https://example.com/a.png", temp.path()).is_none()
        );
    }

    #[test]
    fn reuse_workspace_file_uri_rejects_missing_file() {
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("missing.png");
        let uri = Url::from_file_path(&missing).unwrap().to_string();
        assert!(reuse_workspace_file_uri_if_valid(&uri, temp.path()).is_none());
    }
}
