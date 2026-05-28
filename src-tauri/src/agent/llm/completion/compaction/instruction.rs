use crate::mcp::types::MCPContent;
use crate::models::chat::{Message, MessageSource};

use super::hints::build_compaction_preservation_hints;

const COMPACTION_SECTION_SCHEMA: &str = "Use EXACTLY these sections in this order:\n\
1. Stable Context\n\
2. Key Decisions & Constraints\n\
3. Active Request\n\
4. Required References\n\
5. Current State\n\
6. Recent Tool Results\n\
7. Next Actions";

const COMPACTION_RULES: &[&str] = &[
    "Use terse bullet points, not prose paragraphs.",
    "Prefer noun phrases and short action statements.",
    "Minimize adjectives, adverbs, filler, and repetition.",
    "Do not restate obvious chronology or narration.",
    "Preserve durable facts, decisions, constraints, user preferences, and unresolved work.",
    "Active Request: Record only the currently unresolved user request. If a prior request is resolved, clear it from this section.",
    "Required References: Record minimum file paths, symbols, or identifiers needed for the active request.",
    "Keep volatile details in Current State, Recent Tool Results, or Next Actions.",
    "Do not paraphrase away concrete targets, technology choices, or exact file paths.",
];

const COMPACTION_SECTION_LIMITS: &[&str] = &[
    "Stable Context: at most 6 bullets",
    "Key Decisions & Constraints: at most 6 bullets",
    "Active Request: at most 4 bullets",
    "Required References: at most 5 bullets",
    "Current State: at most 6 bullets",
    "Recent Tool Results: at most 5 bullets",
    "Next Actions: at most 5 bullets",
    "Each bullet should be one short sentence or fragment.",
];

const COMPACTION_OUTPUT_CONSTRAINT: &str =
    "IMPORTANT: Do NOT attempt to use tools in this response. Just output plain text.";

const INCREMENTAL_COMPACTION_RESIDUAL_PREFIX: &str = "The first message is a previously accumulated compact summary representing ALL earlier history.\n\n\
CRITICAL RESIDUAL RULE: Every fact, decision, and constraint in the prior summary MUST be preserved verbatim in your new summary. Do NOT drop durable information. You may clean wording or move items to appropriate sections, but do not lose meaning.\n\
ACTIVE REQUEST UPDATE RULE: Rewrite Active Request only if new messages show it is resolved, refined, or completed. In that case, clear resolved bullets and move completed outcomes to Stable Context or Current State.";

pub(super) const ACTIVE_REQUEST_BULLET_LIMIT: usize = 4;
pub(super) const REQUIRED_REFERENCE_BULLET_LIMIT: usize = 5;
pub(super) const REFERENCE_CONTEXT_WINDOW_MESSAGES: usize = 8;
// Character cap for individual hint bullets. This keeps the instruction block
// bounded and biased toward terse operational seeds rather than copying large
// raw request paragraphs into the compaction prompt.
pub(super) const INSTRUCTION_HINT_TEXT_LIMIT: usize = 320;

fn render_bulleted_lines(lines: &[&str]) -> String {
    lines
        .iter()
        .map(|line| format!("- {}", line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_base_compaction_instruction() -> String {
    format!(
        "Summarise the previous conversation history using strict compact Markdown.\n\n{}\n\nCompression rules:\n{}\n\nSection limits:\n{}\n\n{}",
        COMPACTION_SECTION_SCHEMA,
        render_bulleted_lines(COMPACTION_RULES),
        render_bulleted_lines(COMPACTION_SECTION_LIMITS),
        COMPACTION_OUTPUT_CONSTRAINT
    )
}

fn build_compaction_hint_block(messages: &[Message]) -> Option<String> {
    let hints = build_compaction_preservation_hints(messages);
    if hints.active_request.is_empty() && hints.required_references.is_empty() {
        return None;
    }

    let mut parts = Vec::new();
    if !hints.active_request.is_empty() {
        parts.push(format!(
            "Active Request distillation seed:\n{}",
            hints
                .active_request
                .iter()
                .map(|line| format!("- {}", line))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    if !hints.required_references.is_empty() {
        parts.push(format!(
            "Required References candidates:\n{}",
            hints
                .required_references
                .iter()
                .map(|line| format!("- {}", line))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    Some(format!(
        "Preservation hints for this compaction input:\n{}",
        parts.join("\n\n")
    ))
}

pub(super) fn build_compaction_instruction(messages: &[Message]) -> String {
    let mut instruction = build_base_compaction_instruction();
    if let Some(hint_block) = build_compaction_hint_block(messages) {
        instruction = format!("{}\n\n{}", instruction, hint_block);
    }

    if messages.first().map(Message::is_compact_summary) == Some(true) {
        return format!(
            "{}\n\n{}",
            INCREMENTAL_COMPACTION_RESIDUAL_PREFIX, instruction
        );
    }

    instruction
}

pub(super) fn build_compaction_instruction_message(
    session_id: &str,
    instruction: String,
    created_at: i64,
) -> Message {
    Message {
        id: format!("compaction-instruction-{}", created_at),
        session_id: session_id.to_string(),
        role: "user".to_string(),
        content: vec![MCPContent::Text {
            text: instruction,
            is_error: None,
        }],
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
        created_at,
        updated_at: created_at,
        source: Some(MessageSource::CompactionInstruction),
        error: None,
        metadata: None,
    }
}

fn normalize_instruction_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn truncate_instruction_text(text: &str, limit: usize) -> String {
    let normalized = normalize_instruction_text(text);
    if normalized.chars().count() <= limit {
        return normalized;
    }

    let truncated = normalized
        .chars()
        .take(limit.saturating_sub(1))
        .collect::<String>();
    format!("{}…", truncated.trim_end())
}
