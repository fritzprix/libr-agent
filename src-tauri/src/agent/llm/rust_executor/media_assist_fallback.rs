//! MediaAssist fallback for multimodal capability rejection on the Rust LLM path (#3090).
//!
//! Mirrors `src/lib/ai-service/media-assist-fallback.ts`:
//! - Detects multimodal capability rejections (400 / 415 / 422 with multimodal hint,
//!   or 5xx with explicit capability-reject hint). Never triggers on context-limit,
//!   429, or opaque 5xx.
//! - Prefers the host MediaAssist plugin (`media_assist::run_plugin`) unless the
//!   session is Docker/Harbor-isolated; otherwise strips media and appends the
//!   `@skill:libragent-plugin` placeholder.
//! - Remembers the last `(provider, model)` route that required fallback in process
//!   memory so subsequent turns pre-convert/strip outbound payloads.

use crate::mcp::builtin::workspace::utils::is_session_docker_isolated;
use crate::mcp::types::MCPContent;
use crate::media_assist::{self, RunRequest};
use crate::models::chat::Message;
use crate::session::get_session_manager;
use llm_client::{ErrorKind, LlmError};
use regex::Regex;
use std::path::Path;
use std::sync::{LazyLock, Mutex};

pub const MEDIA_ASSIST_SKILL_NAME: &str = "libragent-plugin";

static MULTIMODAL_ERROR_HINT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(audio|image|video|modality|modalities|input_audio|multimodal|unsupported\s+(?:media|content|modality)|invalid.*(?:image|audio|video)|does not support\s+(?:audio|image|video|media|modalit\w*))\b",
    )
    .expect("valid multimodal error regex")
});

static MULTIMODAL_CAPABILITY_REJECT_HINT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:audio|image|video|input_audio)\s+input\s+is\s+not\s+supported\b|\bdoes\s+not\s+support\s+(?:audio|image|video|media|modalit\w*)\b|\bunsupported\s+(?:media|content|modality|audio|image|video)\b|\bmmproj\b|\bmodalit(?:y|ies)\s+(?:are\s+)?(?:not\s+supported|unsupported)\b",
    )
    .expect("valid multimodal capability reject regex")
});

static CONTEXT_LIMIT_HINT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bcontext\b.*\b(length|window|exceed)").expect("valid context limit regex")
});

static RATE_LIMIT_HINT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(429|rate\s*limit|quota\s*exceeded)\b").expect("valid rate limit regex")
});

static SERVER_ERROR_STATUS_HINT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b5\d{2}\b").expect("valid 5xx regex"));

static REMOTE_OR_DATA_URI: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(https?|data|blob):").expect("valid remote uri regex"));

static REMEMBERED_STRIP_ROUTE_KEY: LazyLock<Mutex<Option<String>>> =
    LazyLock::new(|| Mutex::new(None));

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StrippedModalities {
    pub image: bool,
    pub audio: bool,
    pub video: bool,
}

impl StrippedModalities {
    pub fn any(self) -> bool {
        self.image || self.audio || self.video
    }
}

pub fn media_assist_capability_route_key(provider: &str, model: &str) -> String {
    format!("{}\0{}", provider.trim(), model.trim())
}

pub fn remember_strip_for_route(provider: &str, model: &str) {
    if let Ok(mut guard) = REMEMBERED_STRIP_ROUTE_KEY.lock() {
        *guard = Some(media_assist_capability_route_key(provider, model));
    }
}

/// True when outbound messages for this route should be pre-converted/stripped.
/// Changing provider or model clears the remembered route.
pub fn should_strip_for_route(provider: &str, model: &str) -> bool {
    let Ok(mut guard) = REMEMBERED_STRIP_ROUTE_KEY.lock() else {
        return false;
    };
    let Some(ref remembered) = *guard else {
        return false;
    };
    let key = media_assist_capability_route_key(provider, model);
    if key != *remembered {
        *guard = None;
        return false;
    }
    true
}

#[cfg(test)]
pub fn clear_strip_route_memory() {
    if let Ok(mut guard) = REMEMBERED_STRIP_ROUTE_KEY.lock() {
        *guard = None;
    }
}

pub fn is_multimodal_part(part: &MCPContent) -> bool {
    matches!(part, MCPContent::Image { .. } | MCPContent::Audio { .. })
}

pub fn messages_have_multimodal_parts(messages: &[Message]) -> bool {
    messages
        .iter()
        .any(|m| m.content.iter().any(is_multimodal_part))
}

pub fn collect_stripped_modalities(content: &[MCPContent]) -> StrippedModalities {
    let mut result = StrippedModalities::default();
    for part in content {
        match part {
            MCPContent::Image { .. } => result.image = true,
            MCPContent::Audio { .. } => result.audio = true,
            _ => {}
        }
    }
    result
}

pub fn build_media_assist_placeholder(stripped: StrippedModalities) -> String {
    let mut kinds = Vec::new();
    if stripped.audio {
        kinds.push("audio");
    }
    if stripped.image {
        kinds.push("image");
    }
    if stripped.video {
        kinds.push("video");
    }
    let kind_list = if kinds.is_empty() {
        "media".to_string()
    } else {
        kinds.join("/")
    };

    [
        format!(
            "[media-assist] Multimodal {kind_list} was stripped because this model/runtime rejected it (capability reject after media content; may arrive as HTTP 400 or mislabeled 5xx)."
        ),
        "Continue the task with external commands or sparse samples if needed (e.g. ffmpeg keyframes, OCR CLI, media__seeContent on a few frames). Do not spend the session apt/pip-installing ASR/OCR stacks.".to_string(),
        format!(
            "For a durable fix on this machine: load @skill:{MEDIA_ASSIST_SKILL_NAME}, implement a MediaAssist plugin (audio|image|video → text), run its verify script, then deploy to the harness plugin path. Once deployed, LibrAgent will auto-convert media on future modality failures."
        ),
    ]
    .join("\n")
}

pub fn strip_multimodal_content(content: &[MCPContent]) -> Vec<MCPContent> {
    let stripped = collect_stripped_modalities(content);
    if !stripped.any() {
        return content.to_vec();
    }
    let mut kept: Vec<MCPContent> = content
        .iter()
        .filter(|part| !is_multimodal_part(part))
        .cloned()
        .collect();
    kept.push(MCPContent::Text {
        text: build_media_assist_placeholder(stripped),
    });
    kept
}

pub fn strip_multimodal_from_messages(messages: &[Message]) -> Vec<Message> {
    messages
        .iter()
        .map(|message| {
            if !message.content.iter().any(is_multimodal_part) {
                return message.clone();
            }
            let mut next = message.clone();
            next.content = strip_multimodal_content(&message.content);
            next
        })
        .collect()
}

pub fn should_attempt_media_assist_fallback(error: &LlmError, messages: &[Message]) -> bool {
    if !messages_have_multimodal_parts(messages) {
        return false;
    }

    let blob = &error.message;
    if CONTEXT_LIMIT_HINT.is_match(blob) {
        return false;
    }

    if error.kind == ErrorKind::RateLimit
        || error.status == Some(429)
        || RATE_LIMIT_HINT.is_match(blob)
    {
        return false;
    }

    let is_server_failure =
        error.status.is_some_and(|s| s >= 500) || SERVER_ERROR_STATUS_HINT.is_match(blob);
    if is_server_failure {
        return MULTIMODAL_CAPABILITY_REJECT_HINT.is_match(blob);
    }

    match error.status {
        Some(415) => true,
        Some(400 | 422) => MULTIMODAL_ERROR_HINT.is_match(blob),
        _ => false,
    }
}

pub fn local_media_path_for_plugin(uri: Option<&str>) -> Option<String> {
    let trimmed = uri?.trim();
    if trimmed.is_empty() || REMOTE_OR_DATA_URI.is_match(trimmed) {
        return None;
    }
    Some(trimmed.to_string())
}

async fn convert_part_with_plugin(base_data_dir: &Path, part: &MCPContent) -> Option<String> {
    let (modality, mime_type, uri, data) = match part {
        MCPContent::Image {
            data,
            uri,
            mime_type,
        } => (
            "image",
            if mime_type.is_empty() {
                "image/png"
            } else {
                mime_type.as_str()
            },
            uri.as_deref(),
            data.as_deref(),
        ),
        MCPContent::Audio {
            data,
            uri,
            mime_type,
        } => (
            "audio",
            if mime_type.is_empty() {
                "audio/wav"
            } else {
                mime_type.as_str()
            },
            uri.as_deref(),
            data.as_deref(),
        ),
        _ => return None,
    };

    let path = local_media_path_for_plugin(uri);
    let data_base64 = data
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if path.is_none() && data_base64.is_none() {
        return None;
    }

    let req = RunRequest {
        modality: modality.to_string(),
        mime_type: mime_type.to_string(),
        path,
        data_base64,
        max_output_chars: None,
    };
    let resp = media_assist::run_plugin(base_data_dir, req).await.ok()?;
    let text = resp
        .text
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    if !resp.ok {
        return None;
    }
    let note = resp
        .notes
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|n| format!(" ({n})"))
        .unwrap_or_default();
    Some(format!("[media-assist:{modality}]{note}\n{text}"))
}

pub async fn prepare_messages_for_media_assist_retry_with_dir(
    messages: &[Message],
    base_data_dir: Option<&Path>,
    docker_isolated: bool,
) -> (Vec<Message>, bool) {
    if docker_isolated {
        return (strip_multimodal_from_messages(messages), false);
    }
    let Some(base_dir) = base_data_dir else {
        return (strip_multimodal_from_messages(messages), false);
    };
    let status = media_assist::load_status(base_dir);
    if !status.installed {
        return (strip_multimodal_from_messages(messages), false);
    }

    let mut used_plugin = false;
    let mut next_messages = Vec::with_capacity(messages.len());

    for message in messages {
        if !message.content.iter().any(is_multimodal_part) {
            next_messages.push(message.clone());
            continue;
        }
        let mut next_content = Vec::with_capacity(message.content.len());
        let mut failed_strip = StrippedModalities::default();

        for part in &message.content {
            if !is_multimodal_part(part) {
                next_content.push(part.clone());
                continue;
            }
            if let Some(converted) = convert_part_with_plugin(base_dir, part).await {
                used_plugin = true;
                next_content.push(MCPContent::Text { text: converted });
            } else {
                match part {
                    MCPContent::Image { .. } => failed_strip.image = true,
                    MCPContent::Audio { .. } => failed_strip.audio = true,
                    _ => {}
                }
            }
        }

        if failed_strip.any() {
            next_content.push(MCPContent::Text {
                text: build_media_assist_placeholder(failed_strip),
            });
        }

        let mut next = message.clone();
        next.content = next_content;
        next_messages.push(next);
    }

    (next_messages, used_plugin)
}

/// Prefer host plugin conversion; fall back to strip + skill placeholder.
/// Never runs host plugins for Docker or Harbor sessions.
pub async fn prepare_messages_for_media_assist_retry(
    messages: &[Message],
    session_id: &str,
) -> (Vec<Message>, bool) {
    let docker_isolated =
        !session_id.trim().is_empty() && is_session_docker_isolated(session_id).await;
    let base_dir = get_session_manager()
        .ok()
        .map(|mgr| mgr.get_base_data_dir().clone());
    prepare_messages_for_media_assist_retry_with_dir(messages, base_dir.as_deref(), docker_isolated)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg_with_content(content: Vec<MCPContent>) -> Message {
        Message {
            id: "m1".to_string(),
            session_id: "s1".to_string(),
            role: "user".to_string(),
            content,
            tool_calls: None,
            tool_call_id: None,
            is_streaming: None,
            thinking: None,
            thinking_signature: None,
            assistant_id: None,
            attachments: None,
            tool_use: None,
            usage: None,
            prompt_tokens: None,
            created_at: 0,
            updated_at: 0,
            source: None,
            error: None,
            metadata: None,
        }
    }

    fn image_msg() -> Message {
        msg_with_content(vec![
            MCPContent::Text {
                text: "describe".to_string(),
            },
            MCPContent::Image {
                data: Some("AAAA".to_string()),
                uri: None,
                mime_type: "image/png".to_string(),
            },
        ])
    }

    #[test]
    fn fallback_decision_matches_multimodal_rejects_and_ignores_others() {
        let msgs = vec![image_msg()];
        let text_only = vec![msg_with_content(vec![MCPContent::Text {
            text: "hi".to_string(),
        }])];

        // 400 with multimodal hint -> true
        assert!(should_attempt_media_assist_fallback(
            &LlmError::from_http_status(400, "model does not support image input"),
            &msgs
        ));
        // 415 -> always true when media present
        assert!(should_attempt_media_assist_fallback(
            &LlmError::from_http_status(415, "unsupported media type"),
            &msgs
        ));
        // 500 with explicit capability reject -> true
        assert!(should_attempt_media_assist_fallback(
            &LlmError::from_http_status(500, "audio input is not supported; missing mmproj"),
            &msgs
        ));
        // Opaque 500 -> false
        assert!(!should_attempt_media_assist_fallback(
            &LlmError::from_http_status(500, "internal server error while processing image"),
            &msgs
        ));
        // 429 rate limit -> false
        assert!(!should_attempt_media_assist_fallback(
            &LlmError::from_http_status(429, "rate limit exceeded for image"),
            &msgs
        ));
        // Context limit -> false
        assert!(!should_attempt_media_assist_fallback(
            &LlmError::from_http_status(400, "maximum context length exceeded with image"),
            &msgs
        ));
        // No multimodal parts -> false
        assert!(!should_attempt_media_assist_fallback(
            &LlmError::from_http_status(400, "model does not support image input"),
            &text_only
        ));
    }

    #[test]
    fn route_memory_persists_for_same_route_and_clears_on_change() {
        clear_strip_route_memory();
        assert!(!should_strip_for_route("openai", "gpt-4o-mini"));

        remember_strip_for_route("openai", "gpt-4o-mini");
        assert!(should_strip_for_route("openai", "gpt-4o-mini"));
        assert!(!should_strip_for_route("openai", "gpt-4o"));
        // Changing route cleared memory
        assert!(!should_strip_for_route("openai", "gpt-4o-mini"));
    }

    #[test]
    fn strip_replaces_media_with_placeholder_and_keeps_text() {
        let stripped = strip_multimodal_from_messages(&[image_msg()]);
        assert_eq!(stripped.len(), 1);
        assert_eq!(stripped[0].content.len(), 2);
        match &stripped[0].content[1] {
            MCPContent::Text { text } => {
                assert!(text.contains("[media-assist] Multimodal image was stripped"));
                assert!(text.contains("@skill:libragent-plugin"));
            }
            other => panic!("expected placeholder text, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn docker_isolated_session_never_runs_plugin() {
        let tmp = tempfile::tempdir().unwrap();
        let (prepared, used_plugin) = prepare_messages_for_media_assist_retry_with_dir(
            &[image_msg()],
            Some(tmp.path()),
            true,
        )
        .await;
        assert!(!used_plugin);
        assert!(!messages_have_multimodal_parts(&prepared));
    }
}
