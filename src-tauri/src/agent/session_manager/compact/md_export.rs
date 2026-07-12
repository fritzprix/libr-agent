// src-tauri/src/agent/session_manager/compact/md_export.rs

use crate::models::chat::Message;
use crate::services::WorkspaceService;
use std::path::PathBuf;
use tokio::fs;

fn truncate_text(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_string();
    }
    let mut boundary = limit.min(text.len());
    while boundary > 0 && !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    let truncated = &text[..boundary];
    match truncated.rfind('\n') {
        Some(pos) if pos > 0 => format!(
            "{}\n\n[... truncated {} bytes, use session context for details ...]",
            &truncated[..pos],
            text.len() - boundary
        ),
        _ => format!(
            "{}\n\n[... truncated {} bytes, use session context for details ...]",
            truncated,
            text.len() - boundary
        ),
    }
}

pub fn build_compaction_markdown(session_id: &str, messages: &[Message]) -> String {
    let mut md = String::new();
    md.push_str(&format!(
        "# Pre-compaction Transcript\nSession: {}\n\n",
        session_id
    ));

    for msg in messages {
        let role = msg.role.to_uppercase();
        md.push_str(&format!("---\n[{}] {}\n", msg.created_at, role));

        if let Some(thinking) = &msg.thinking {
            if !thinking.is_empty() {
                md.push_str(&format!("Thinking:\n{}\n\n", truncate_text(thinking, 1500)));
            }
        }

        for content in &msg.content {
            match content {
                crate::mcp::types::MCPContent::Text { text, .. } => {
                    md.push_str(&format!("{}\n", truncate_text(text, 1500)));
                }
                crate::mcp::types::MCPContent::Thinking { thinking, .. }
                    if msg.thinking.is_none() =>
                {
                    md.push_str(&format!("Thinking:\n{}\n\n", truncate_text(thinking, 1500)));
                }
                // Intentionally skip other content variants (such as ToolResult) to prevent token explosion.
                // Detailed tool results remain accessible via the main workspace or session logs.
                _ => {}
            }
        }

        if let Some(tool_calls) = &msg.tool_calls {
            for tc in tool_calls {
                md.push_str(&format!(
                    "\n[TOOL CALL] {}({})\n",
                    tc.function.name, tc.function.arguments
                ));
            }
        }
    }

    md
}

pub async fn write_compaction_markdown(
    session_id: &str,
    messages: &[Message],
    max_epochs: usize,
) -> Result<(String, u32), String> {
    let markdown = build_compaction_markdown(session_id, messages);

    let session_manager = crate::session::get_session_manager()
        .map_err(|e| format!("Session manager error: {}", e))?;
    let workspace_dir =
        crate::session::resolve_session_workspace_dir(session_manager, session_id).await?;
    let libragent_dir = workspace_dir.join(".libragent");

    // TODO: fs::create_dir_all bypass - Safe because workspace_dir is session-scoped.
    fs::create_dir_all(&libragent_dir)
        .await
        .map_err(|e| format!("Failed to create .libragent dir: {}", e))?;

    let mut existing: Vec<(u32, PathBuf)> = Vec::new();
    let mut entries = fs::read_dir(&libragent_dir)
        .await
        .map_err(|e| format!("Failed to read .libragent dir: {}", e))?;

    while let Some(entry) = entries.next_entry().await.ok().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(rest) = name.strip_prefix("pre_compaction_epoch_") {
            if let Some(num_str) = rest.strip_suffix(".md") {
                if let Ok(n) = num_str.parse::<u32>() {
                    existing.push((n, entry.path()));
                }
            }
        }
    }

    existing.sort_by_key(|k| k.0);
    let next_epoch = existing.last().map(|(n, _)| n + 1).unwrap_or(1);

    let file_name = format!("pre_compaction_epoch_{}.md", next_epoch);
    let relative_path = format!(".libragent/{}", file_name);

    WorkspaceService::workspace_write_file(
        &relative_path,
        markdown.as_bytes(),
        Some(session_id.to_string()),
    )
    .await
    .map_err(|e| format!("Failed to write via WorkspaceService: {}", e))?;

    existing.push((next_epoch, libragent_dir.join(&file_name)));
    existing.sort_by_key(|k| k.0);

    if existing.len() > max_epochs {
        let to_remove = existing.len() - max_epochs;
        for (_, path) in existing.iter().take(to_remove) {
            let _ = fs::remove_file(path).await;
        }
    }

    Ok((relative_path, next_epoch))
}

pub fn format_transcript_recovery_suffix(relative_path: &str) -> String {
    format!(
        "\n\n---\n### Context recovery\n\
A full pre-compaction transcript was saved to `{path}`.\n\
It contains the user/assistant messages and tool calls from before this compaction.\n\
If you are unsure what to do next, need missing details, or this summary feels incomplete, \
read `{path}` first (via `workspace__readFile` or `tail {path}` in the session workspace).",
        path = relative_path
    )
}

#[cfg(test)]
mod tests {
    use super::{build_compaction_markdown, format_transcript_recovery_suffix, truncate_text};
    use crate::agent::types::{ToolCall, ToolCallFunction};
    use crate::models::chat::Message;

    fn message_with_role(role: &str, text: &str) -> Message {
        let mut message =
            Message::new_user_message("session-test".to_string(), text.to_string(), None, None);
        message.role = role.to_string();
        message
    }

    #[test]
    fn transcript_recovery_suffix_explains_file_purpose_and_how_to_read_it() {
        let suffix = format_transcript_recovery_suffix(".libragent/pre_compaction_epoch_4.md");

        assert!(suffix.contains(".libragent/pre_compaction_epoch_4.md"));
        assert!(suffix.contains("pre-compaction transcript"));
        assert!(suffix.contains("user/assistant messages and tool calls"));
        assert!(suffix.contains("workspace__readFile"));
        assert!(suffix.contains("tail .libragent/pre_compaction_epoch_4.md"));
    }

    #[test]
    fn truncate_text_respects_utf8_char_boundaries() {
        let text = "🎉".repeat(500);
        let truncated = truncate_text(&text, 20);

        assert!(truncated.contains("truncated"));
        assert!(std::str::from_utf8(truncated.as_bytes()).is_ok());
    }

    #[test]
    fn build_compaction_markdown_handles_empty_message_list() {
        let markdown = build_compaction_markdown("session-empty", &[]);

        assert!(markdown.starts_with("# Pre-compaction Transcript\nSession: session-empty"));
        assert!(!markdown.contains("---\n["));
    }

    #[test]
    fn build_compaction_markdown_preserves_shell_special_characters() {
        let messages = vec![message_with_role(
            "user",
            "Gradium raised $100M. Prime: `code` & \"quotes\".",
        )];
        let markdown = build_compaction_markdown("session-special", &messages);

        assert!(markdown.contains("$100M"));
        assert!(markdown.contains("`code`"));
        assert!(markdown.contains("\"quotes\""));
    }

    #[test]
    fn build_compaction_markdown_includes_tool_calls_and_truncates_long_thinking() {
        let mut message = message_with_role("assistant", "");
        message.content.clear();
        message.thinking = Some("x".repeat(2_000));
        message.tool_calls = Some(vec![ToolCall {
            id: "call-1".to_string(),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: "workspace__readFile".to_string(),
                arguments: r#"{"path":"src/main.rs"}"#.to_string(),
            },
        }]);

        let markdown = build_compaction_markdown("session-tools", &[message]);

        assert!(markdown.contains("[TOOL CALL] workspace__readFile"));
        assert!(markdown.contains("truncated"));
        assert!(!markdown.contains(&"x".repeat(2_000)));
    }
}
