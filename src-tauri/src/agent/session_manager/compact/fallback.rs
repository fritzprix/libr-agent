use super::summary::{extract_message_preview, COMPACTION_SUMMARY_TRUNCATION_SUFFIX};
use crate::agent::compaction_text::sanitize_compaction_semantic_text;
use crate::agent::llm::completion::{
    build_compaction_preservation_hints, summarize_recent_tool_calls,
};
use crate::agent::state::{CompactionPhase, CompactionRecoveryPhase, CompactionSnapshot};
use crate::mcp::types::MCPContent;
use crate::models::chat::Message;

const COMPACTION_FALLBACK_ARTIFACT_DIR: &str = ".libragent/tool-results/compaction";
const COMPACTION_FALLBACK_ARTIFACT_MESSAGE_LIMIT: usize = 12;
const COMPACTION_FALLBACK_ACTIVE_REQUEST_LIMIT: usize = 4;
const COMPACTION_FALLBACK_REFERENCE_LIMIT: usize = 5;
const COMPACTION_FALLBACK_MESSAGE_TEXT_LIMIT: usize = 1_200;
const COMPACTION_FALLBACK_NOTE: &str =
    "Auto-saved via fallback summary after compaction retries ran out.";

fn push_bullet_section(lines: &mut Vec<String>, heading: &str, bullets: &[String]) {
    lines.push(format!("### {}", heading));
    if bullets.is_empty() {
        lines.push("- None".to_string());
    } else {
        lines.extend(bullets.iter().map(|bullet| format!("- {}", bullet)));
    }
    lines.push(String::new());
}

fn truncate_for_fallback(text: &str, max_chars: usize) -> String {
    let normalized = sanitize_compaction_semantic_text(text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }

    format!(
        "{}{}",
        normalized
            .chars()
            .take(max_chars.saturating_sub(COMPACTION_SUMMARY_TRUNCATION_SUFFIX.chars().count()))
            .collect::<String>()
            .trim_end(),
        COMPACTION_SUMMARY_TRUNCATION_SUFFIX
    )
}

fn sanitize_fallback_identifier(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|char| {
            if char.is_ascii_alphanumeric() || matches!(char, '-' | '_') {
                char
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if sanitized.is_empty() {
        "item".to_string()
    } else {
        sanitized.chars().take(48).collect()
    }
}

pub(super) fn compaction_fallback_artifact_relative_path(
    session_id: &str,
    to_id: &str,
    created_at: i64,
) -> String {
    format!(
        "{}/fallback-{}-{}-{}.md",
        COMPACTION_FALLBACK_ARTIFACT_DIR,
        created_at,
        sanitize_fallback_identifier(&session_id.chars().take(8).collect::<String>()),
        sanitize_fallback_identifier(&to_id.chars().take(12).collect::<String>())
    )
}

pub fn compaction_fallback_artifact_relative_path_for_testing(
    session_id: &str,
    to_id: &str,
    created_at: i64,
) -> String {
    compaction_fallback_artifact_relative_path(session_id, to_id, created_at)
}

fn fallback_message_excerpt(message: &Message) -> Option<String> {
    let mut fragments = Vec::new();

    for content in &message.content {
        match content {
            MCPContent::Text { text, .. } => {
                let snippet = truncate_for_fallback(text, COMPACTION_FALLBACK_MESSAGE_TEXT_LIMIT);
                if !snippet.is_empty() {
                    fragments.push(snippet);
                }
            }
            MCPContent::Thinking { thinking, .. } => {
                let snippet =
                    truncate_for_fallback(thinking, COMPACTION_FALLBACK_MESSAGE_TEXT_LIMIT);
                if !snippet.is_empty() {
                    fragments.push(format!("Thinking: {}", snippet));
                }
            }
            MCPContent::ToolCall { name, .. } => fragments.push(format!("Tool call: {}", name)),
            _ => {}
        }
    }

    if fragments.is_empty() {
        if let Some(tool_calls) = &message.tool_calls {
            for tool_call in tool_calls {
                fragments.push(format!("Tool call: {}", tool_call.function.name));
            }
        }
    }

    if fragments.is_empty() && message.role == "tool" {
        fragments.push("Tool result".to_string());
    }

    if fragments.is_empty() {
        None
    } else {
        Some(fragments.join("\n"))
    }
}

fn format_recovery_phase_label(recovery_phase: CompactionRecoveryPhase) -> &'static str {
    match recovery_phase {
        CompactionRecoveryPhase::CacheAligned => "cache-aligned",
        CompactionRecoveryPhase::OverflowRecovery => "overflow-recovery",
        CompactionRecoveryPhase::DegradedTools => "degraded-tools",
    }
}

pub(super) fn build_compaction_hard_fallback_summary(
    compacted_messages: &[Message],
    artifact_relative_path: Option<&str>,
    to_id: &str,
    compacted_delta_count: usize,
    snapshot: Option<&CompactionSnapshot>,
    failure_reason: &str,
) -> String {
    let hints = build_compaction_preservation_hints(compacted_messages);
    let active_request = hints
        .active_request
        .into_iter()
        .take(COMPACTION_FALLBACK_ACTIVE_REQUEST_LIMIT)
        .collect::<Vec<_>>();
    let required_references = hints
        .required_references
        .into_iter()
        .take(COMPACTION_FALLBACK_REFERENCE_LIMIT)
        .collect::<Vec<_>>();
    let latest_preview = compacted_messages
        .last()
        .and_then(extract_message_preview)
        .unwrap_or_else(|| "Newest compacted message".to_string());
    let mut current_state = vec![
        COMPACTION_FALLBACK_NOTE.to_string(),
        format!(
            "Compacted {} messages up to `{}`.",
            compacted_delta_count,
            to_id.chars().take(12).collect::<String>()
        ),
        format!("Latest included: {}", latest_preview),
    ];
    if let Some(snapshot) = snapshot {
        current_state.push(format!(
            "Recovery path stopped at `{}` after {} summary retries.",
            format_recovery_phase_label(snapshot.recovery_phase),
            snapshot.summary_retry_count
        ));
    }
    current_state.push(format!(
        "Fallback reason: {}",
        truncate_for_fallback(failure_reason, 180)
    ));

    let recent_tool_results = summarize_recent_tool_calls(compacted_messages);
    let mut next_actions = vec!["Resume from the latest active request above.".to_string()];
    let mut fallback_note = Vec::new();
    if let Some(artifact_relative_path) = artifact_relative_path {
        next_actions.push(format!(
            "Open `{}` if you need the saved excerpts and recent detail.",
            artifact_relative_path
        ));
        fallback_note.push(format!(
            "Detailed fallback context saved to `{}`.",
            artifact_relative_path
        ));
        fallback_note.push(
            "Use that file when the summary feels too compressed or you need exact recent excerpts."
                .to_string(),
        );
    } else {
        fallback_note.push(
            "Detailed fallback context could not be written, so this summary is the only saved handoff."
                .to_string(),
        );
    }

    let mut lines = Vec::new();
    push_bullet_section(&mut lines, "Active Request", &active_request);
    push_bullet_section(&mut lines, "Required References", &required_references);
    push_bullet_section(&mut lines, "Current State", &current_state);
    push_bullet_section(&mut lines, "Recent Tool Results", &recent_tool_results);
    push_bullet_section(&mut lines, "Next Actions", &next_actions);
    push_bullet_section(&mut lines, "Fallback Note", &fallback_note);
    lines.join("\n").trim().to_string()
}

pub fn build_compaction_hard_fallback_summary_for_testing(
    compacted_messages: &[Message],
    artifact_relative_path: &str,
    to_id: &str,
    compacted_delta_count: usize,
    recovery_phase: CompactionRecoveryPhase,
    summary_retry_count: u32,
    failure_reason: &str,
) -> String {
    build_compaction_hard_fallback_summary(
        compacted_messages,
        Some(artifact_relative_path),
        to_id,
        compacted_delta_count,
        Some(&CompactionSnapshot {
            phase: CompactionPhase::Idle,
            last_compacted_tail_id: None,
            retry_attempt: 0,
            recovery_phase,
            summary_retry_count,
        }),
        failure_reason,
    )
}

pub(super) fn build_compaction_hard_fallback_artifact(
    session_id: &str,
    to_id: &str,
    compacted_delta_count: usize,
    compacted_messages: &[Message],
    artifact_relative_path: &str,
    snapshot: Option<&CompactionSnapshot>,
    failure_reason: &str,
) -> String {
    let hints = build_compaction_preservation_hints(compacted_messages);
    let mut lines = vec![
        "# Compaction fallback artifact".to_string(),
        String::new(),
        format!("- Session: `{}`", session_id),
        format!("- Boundary message: `{}`", to_id),
        format!("- Condensed messages: {}", compacted_delta_count),
        format!("- Saved artifact: `{}`", artifact_relative_path),
        format!(
            "- Trigger: {}",
            truncate_for_fallback(failure_reason, COMPACTION_FALLBACK_MESSAGE_TEXT_LIMIT)
        ),
    ];

    if let Some(snapshot) = snapshot {
        lines.push(format!(
            "- Recovery phase: `{}`",
            format_recovery_phase_label(snapshot.recovery_phase)
        ));
        lines.push(format!(
            "- Summary retries used: {}",
            snapshot.summary_retry_count
        ));
        lines.push(format!(
            "- Budget retry attempt: {}",
            snapshot.retry_attempt
        ));
    }

    lines.push(String::new());
    push_bullet_section(
        &mut lines,
        "Active Request Seeds",
        &hints
            .active_request
            .into_iter()
            .take(COMPACTION_FALLBACK_ACTIVE_REQUEST_LIMIT)
            .collect::<Vec<_>>(),
    );
    push_bullet_section(
        &mut lines,
        "Required References",
        &hints
            .required_references
            .into_iter()
            .take(COMPACTION_FALLBACK_REFERENCE_LIMIT)
            .collect::<Vec<_>>(),
    );
    push_bullet_section(
        &mut lines,
        "Recent Tool Results",
        &summarize_recent_tool_calls(compacted_messages),
    );

    lines.push("## Recent Message Excerpts".to_string());
    let excerpt_messages = compacted_messages
        .iter()
        .rev()
        .take(COMPACTION_FALLBACK_ARTIFACT_MESSAGE_LIMIT)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();

    for message in excerpt_messages {
        let source = message
            .source
            .as_ref()
            .map(|source| source.as_str().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        lines.push(format!(
            "### {} `{}` ({})",
            message.role, message.id, source
        ));
        lines
            .push(fallback_message_excerpt(message).unwrap_or_else(|| {
                "No text excerpt was recoverable for this message.".to_string()
            }));
        lines.push(String::new());
    }

    lines.join("\n").trim().to_string()
}
