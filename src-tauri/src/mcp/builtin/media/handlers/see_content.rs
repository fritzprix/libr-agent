//! `seeContent` tool handler and per-session success budget.

use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::fetch::{fetch_http_bytes, read_workspace_media_bytes};
use super::mime::resolve_image_mime;
use super::source::{parse_source, ContentSource};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, ToolGroup,
};
use crate::mcp::types::{MCPContent, MCPResult};

/// Hard cap on successful `seeContent` image loads per media-server session.
///
/// Soft tool-description advice alone does not stop all-frame multimodal dumps;
/// the handler enforces this bound and returns guided recovery.
pub const MAX_SEE_CONTENT_SUCCESSES_PER_SESSION: usize = 8;

/// RAII reservation for one `seeContent` success slot.
///
/// Drop releases the slot unless [`SeeContentSlot::commit`] was called (load
/// succeeded and multimodal content will be returned).
struct SeeContentSlot<'a> {
    counter: &'a AtomicUsize,
    committed: bool,
}

impl<'a> SeeContentSlot<'a> {
    fn try_reserve(counter: &'a AtomicUsize) -> Result<Self, usize> {
        let prev = counter.fetch_add(1, Ordering::SeqCst);
        if prev >= MAX_SEE_CONTENT_SUCCESSES_PER_SESSION {
            counter.fetch_sub(1, Ordering::SeqCst);
            return Err(MAX_SEE_CONTENT_SUCCESSES_PER_SESSION);
        }
        Ok(Self {
            counter,
            committed: false,
        })
    }

    fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for SeeContentSlot<'_> {
    fn drop(&mut self) {
        if !self.committed {
            self.counter.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

fn see_content_cap_recovery() -> Vec<String> {
    vec![
        format!(
            "This session already used the maximum of {MAX_SEE_CONTENT_SUCCESSES_PER_SESSION} successful seeContent loads."
        ),
        "Do not dump every video frame or large image set into context — sample a few key frames only."
            .to_string(),
        "For speech-in-video: extract audio (wav/mp3) and call media__listenContent on that file instead of more seeContent."
            .to_string(),
        "If scripting tools (ffmpeg/python) are available, filter or OCR offline, then use seeContent only for final verification samples."
            .to_string(),
    ]
}

fn see_content_cap_exceeded_result() -> MCPResult {
    guided_error(
        ErrorCategory::InvalidState,
        format!(
            "seeContent session limit reached ({MAX_SEE_CONTENT_SUCCESSES_PER_SESSION} successful image loads). Further seeContent calls are blocked for this session."
        ),
        ToolGroup::Media,
    )
    .with_guidance(see_content_cap_recovery())
    .to_mcp_result()
}

/// Handle the `seeContent` tool.
pub async fn handle_see_content(
    args: Value,
    workspace_dir: std::path::PathBuf,
    session_id: String,
    see_content_successes: &AtomicUsize,
) -> Result<MCPResult, String> {
    let url_str = match args.get("url").and_then(|v| v.as_str()) {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => return Ok(missing_param_error("url", ToolGroup::Media)),
    };

    let slot = match SeeContentSlot::try_reserve(see_content_successes) {
        Ok(slot) => slot,
        Err(_) => return Ok(see_content_cap_exceeded_result()),
    };

    let (bytes, content_type_header) = {
        let source = match parse_source(&url_str) {
            Ok(s) => s,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!("Invalid source '{url_str}': {e}"),
                    ToolGroup::Media,
                )
                .to_mcp_result())
            }
        };
        match source {
            ContentSource::Http(url) => match fetch_http_bytes(&url).await {
                Ok(pair) => pair,
                Err(e) => {
                    return Ok(guided_error(
                        ErrorCategory::OperationFailed,
                        format!("Failed to fetch from '{url_str}': {e}"),
                        ToolGroup::Media,
                    )
                    .to_mcp_result())
                }
            },
            ContentSource::LocalFile(raw_path) => {
                match read_workspace_media_bytes(&raw_path, &workspace_dir, &session_id).await {
                    Ok(data) => (data, None),
                    Err(result) => return Ok(result),
                }
            }
        }
    };

    let mime_type = match resolve_image_mime(&url_str, content_type_header.as_deref()) {
        Some(m) => m,
        None => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Could not determine image MIME type for '{url_str}'. \
                     Ensure the URL/path points to a supported image format \
                     (JPEG, PNG, GIF, WebP, BMP, SVG)."
                ),
                ToolGroup::Media,
            )
            .to_mcp_result());
        }
    };

    if !mime_type.starts_with("image/") {
        return Ok(guided_error(
            ErrorCategory::InvalidInput,
            format!("URL does not point to an image (detected MIME type: {mime_type})."),
            ToolGroup::Media,
        )
        .to_mcp_result());
    }

    let data = general_purpose::STANDARD.encode(&bytes);
    let size_kb = bytes.len() / 1024;
    let remaining = MAX_SEE_CONTENT_SUCCESSES_PER_SESSION
        .saturating_sub(see_content_successes.load(Ordering::SeqCst));

    slot.commit();

    Ok(MCPResult {
        content: Some(vec![
            MCPContent::Text {
                text: format!(
                    "✓ Image loaded ({size_kb} KB, {mime_type})\n\nSource: {url_str}\n\n\
                     seeContent session budget: {remaining} successful load(s) remaining \
                     (max {MAX_SEE_CONTENT_SUCCESSES_PER_SESSION}). Sample sparsely; \
                     for speech-in-video prefer media__listenContent on extracted audio."
                ),
            },
            MCPContent::Image {
                data: Some(data),
                uri: None,
                mime_type,
            },
        ]),
        structured_content: None,
        is_error: Some(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn see_content_slot_allows_up_to_max_then_rejects() {
        let counter = AtomicUsize::new(0);
        let mut committed = Vec::new();
        for _ in 0..MAX_SEE_CONTENT_SUCCESSES_PER_SESSION {
            let slot = SeeContentSlot::try_reserve(&counter).expect("slot available");
            slot.commit();
            committed.push(());
        }
        assert_eq!(
            counter.load(Ordering::SeqCst),
            MAX_SEE_CONTENT_SUCCESSES_PER_SESSION
        );
        assert!(SeeContentSlot::try_reserve(&counter).is_err());
        assert_eq!(
            counter.load(Ordering::SeqCst),
            MAX_SEE_CONTENT_SUCCESSES_PER_SESSION
        );
        let _ = committed;
    }

    #[test]
    fn see_content_slot_releases_on_drop_without_commit() {
        let counter = AtomicUsize::new(0);
        {
            let _slot = SeeContentSlot::try_reserve(&counter).expect("slot available");
            assert_eq!(counter.load(Ordering::SeqCst), 1);
        }
        assert_eq!(counter.load(Ordering::SeqCst), 0);
        let slot = SeeContentSlot::try_reserve(&counter).expect("slot available again");
        slot.commit();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn see_content_cap_recovery_mentions_listen_and_sample() {
        let joined = see_content_cap_recovery().join("\n");
        assert!(joined.contains(&MAX_SEE_CONTENT_SUCCESSES_PER_SESSION.to_string()));
        assert!(joined.contains("listenContent"));
        assert!(joined.to_lowercase().contains("sample"));
    }
}
