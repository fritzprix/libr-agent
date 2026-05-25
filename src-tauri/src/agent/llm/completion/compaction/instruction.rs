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
    "If the messages include an unresolved external request, you MUST record it in Active Request.",
    "Active Request is semantic residual state, not a raw transcript dump; preserve the user's operative intent, constraints, and deliverable without copying every request line verbatim unless exact wording is operationally required.",
    "If a previously recorded Active Request is now resolved, superseded, or no longer actionable, you MUST clear it from Active Request and move any durable outcome to Stable Context, Key Decisions & Constraints, Current State, or Next Actions instead of preserving stale request bullets.",
    "If the unresolved request depends on earlier discovered context, you MUST record the minimum file paths, symbol names, entities, or identifiers needed to execute it in Required References.",
    "Keep volatile/recent details in Current State, Recent Tool Results, or Next Actions.",
    "Do not paraphrase away concrete requirements such as technology choices, limits, file targets, or requested deliverables when they are still relevant to pending work.",
    "Do not replace exact file paths, symbol names, or user-named targets with vague descriptions when they are still operationally relevant.",
    "If a detail is recoverable from recent tool results, do not duplicate it in stable sections.",
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

const INCREMENTAL_COMPACTION_RESIDUAL_PREFIX: &str = "The first message is a previously accumulated compact summary that represents ALL earlier conversation history.\n\n\
CRITICAL RESIDUAL RULE: Every durable fact, decision, constraint, reference, and unresolved operationally useful context item recorded in that prior summary MUST be preserved verbatim or re-stated with equivalent fidelity in your new summary. \
Do NOT drop durable information from the prior summary. \
You may tighten wording, remove duplication, and relocate items into the required sections, but you must preserve the same meaning and operational usefulness. \
EXCEPTION FOR ACTIVE REQUEST: Active Request is allowed to change when the new delta shows that the prior request was resolved, superseded, or refined. In that case, rewrite Active Request to reflect only the still-unresolved operative request, and move completed outcomes to the appropriate non-request sections instead of preserving stale request bullets. \
Your new summary = (prior summary, preserved faithfully and reorganized if needed) + (new messages, summarised under the same schema).";

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
            "Active Request distillation seed (distill the operative unresolved request; do not copy raw wording unless exact phrasing is required):\n{}",
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
