//! `listenContent` tool handler and recovery guidance.

use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;

use super::fetch::{fetch_http_bytes, read_workspace_media_bytes};
use super::mime::{looks_like_video_path, resolve_audio_mime};
use super::source::{parse_source, ContentSource};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::{MCPContent, MCPResult};

/// Post-load guidance after a successful `listenContent`.
///
/// Soft load-ack alone led agents to treat the tool as done and stop; push
/// analysis / sparse visual sample / verify-before-stop without task clues.
fn listen_content_success_follow_ups() -> Vec<String> {
    vec![
        "Audio is attached in this tool result — analyze speech/content from it now; the load acknowledgment alone is not the task answer.".to_string(),
        "If on-screen text also matters, sample a few frames with media__seeContent (session budget applies); do not dump every frame.".to_string(),
        "Persist required workspace outputs and verify them before stopping.".to_string(),
    ]
}

fn listen_content_recovery(url: &str) -> Vec<String> {
    if looks_like_video_path(url) {
        vec![
            "This path looks like a video container; listenContent accepts audio only."
                .to_string(),
            "If ffmpeg is available: extract audio (e.g. `ffmpeg -y -i <video> -vn -acodec pcm_s16le /app/audio.wav`), then call media__listenContent on that wav/mp3."
                .to_string(),
            "If ffmpeg is missing, do not apt-install for long stretches — sample a few keyframes and use media__seeContent on those images instead."
                .to_string(),
            "Prefer media__listenContent / media__seeContent over installing OCR/ASR stacks when the goal is transcription or on-screen text."
                .to_string(),
        ]
    } else {
        vec![
            "Provide a URL/path to a supported audio format (MP3, WAV, OGG, AAC, FLAC, WEBM, M4A)."
                .to_string(),
            "If the source is video, extract an audio track first, then retry listenContent on the extracted file."
                .to_string(),
        ]
    }
}

/// Handle the `listenContent` tool.
pub async fn handle_listen_content(
    args: Value,
    workspace_dir: std::path::PathBuf,
    session_id: String,
) -> Result<MCPResult, String> {
    let url_str = match args.get("url").and_then(|v| v.as_str()) {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => return Ok(missing_param_error("url", ToolGroup::Media)),
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

    let mime_type = match resolve_audio_mime(&url_str, content_type_header.as_deref()) {
        Some(m) => m,
        None => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Could not determine audio MIME type for '{url_str}'. \
                     Ensure the URL/path points to a supported audio format \
                     (MP3, WAV, OGG, AAC, FLAC, WEBM, M4A)."
                ),
                ToolGroup::Media,
            )
            .with_guidance(listen_content_recovery(&url_str))
            .to_mcp_result());
        }
    };

    if !mime_type.starts_with("audio/") {
        return Ok(guided_error(
            ErrorCategory::InvalidInput,
            format!("URL does not point to an audio file (detected MIME type: {mime_type})."),
            ToolGroup::Media,
        )
        .with_guidance(listen_content_recovery(&url_str))
        .to_mcp_result());
    }

    let data = general_purpose::STANDARD.encode(&bytes);
    let size_kb = bytes.len() / 1024;

    let mut result = SuccessHint::new(
        format!("Audio loaded ({size_kb} KB, {mime_type})\n\nSource: {url_str}"),
        listen_content_success_follow_ups(),
    )
    .to_mcp_result();

    // SuccessHint → MCPResult::success always sets content; keep attach resilient.
    result
        .content
        .get_or_insert_with(Vec::new)
        .push(MCPContent::Audio {
            data: Some(data),
            uri: None,
            mime_type,
        });

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listen_content_recovery_for_video_mentions_extract_not_apt() {
        let tips = listen_content_recovery("/app/video.mp4");
        let joined = tips.join("\n");
        assert!(joined.contains("ffmpeg"));
        assert!(joined.contains("listenContent"));
        assert!(joined.contains("seeContent"));
        assert!(joined.to_lowercase().contains("do not apt-install"));
    }

    #[test]
    fn listen_content_success_follow_ups_push_continue_after_load() {
        let joined = listen_content_success_follow_ups().join("\n");
        assert!(joined.to_lowercase().contains("attached"));
        assert!(joined.to_lowercase().contains("not the task answer"));
        assert!(joined.contains("seeContent"));
        assert!(joined.to_lowercase().contains("before stopping"));

        let text = SuccessHint::new(
            "Audio loaded (1 KB, audio/wav)\n\nSource: /app/a.wav",
            listen_content_success_follow_ups(),
        )
        .to_mcp_result();
        let body = match text.content.as_ref().and_then(|c| c.first()) {
            Some(MCPContent::Text { text }) => text.clone(),
            _ => panic!("expected text content"),
        };
        assert!(body.starts_with("✓ Audio loaded"));
        assert!(body.contains("💡 Suggested Follow-ups:"));
        assert!(body.contains("seeContent"));
    }

    #[test]
    fn listen_success_mcp_result_keeps_text_and_audio_parts() {
        let mut result = SuccessHint::new(
            "Audio loaded (1 KB, audio/wav)\n\nSource: /app/a.wav",
            listen_content_success_follow_ups(),
        )
        .to_mcp_result();
        assert!(
            result.content.is_some(),
            "SuccessHint::to_mcp_result must populate content"
        );
        result
            .content
            .get_or_insert_with(Vec::new)
            .push(MCPContent::Audio {
                data: Some("YQ==".to_string()),
                uri: None,
                mime_type: "audio/wav".to_string(),
            });
        let content = result.content.as_ref().expect("content");
        assert_eq!(content.len(), 2);
        assert!(matches!(&content[0], MCPContent::Text { text } if text.starts_with('✓')));
        assert!(matches!(
            &content[1],
            MCPContent::Audio {
                mime_type,
                data: Some(_),
                ..
            } if mime_type == "audio/wav"
        ));
    }
}
