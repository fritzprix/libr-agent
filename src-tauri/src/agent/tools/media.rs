use crate::mcp::types::MCPContent;
use crate::services::WorkspaceService;
use base64::engine::general_purpose;
use base64::Engine;
use sha2::{Digest, Sha256};
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

pub async fn externalize_media_content_for_storage(
    session_id: &str,
    content: Vec<MCPContent>,
) -> Result<Vec<MCPContent>, String> {
    let mut next_content = Vec::with_capacity(content.len());

    for item in content {
        match item {
            MCPContent::Image {
                data: Some(data),
                uri: _uri,
                mime_type,
            } => {
                let bytes = general_purpose::STANDARD.decode(&data).map_err(|error| {
                    format!(
                        "Failed to decode image payload for session {}: {}",
                        session_id, error
                    )
                })?;
                let file_url = persist_tool_result_media(session_id, &bytes, &mime_type).await?;
                next_content.push(MCPContent::Image {
                    data: None,
                    uri: Some(file_url),
                    mime_type,
                });
            }
            MCPContent::Audio {
                data: Some(data),
                uri: _uri,
                mime_type,
            } => {
                let bytes = general_purpose::STANDARD.decode(&data).map_err(|error| {
                    format!(
                        "Failed to decode audio payload for session {}: {}",
                        session_id, error
                    )
                })?;
                let file_url = persist_tool_result_media(session_id, &bytes, &mime_type).await?;
                next_content.push(MCPContent::Audio {
                    data: None,
                    uri: Some(file_url),
                    mime_type,
                });
            }
            other => next_content.push(other),
        }
    }

    Ok(next_content)
}
