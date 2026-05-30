use crate::mcp::types::MCPContent;
use crate::models::chat::{Message, MessageSource};

use super::hints::build_compaction_preservation_hints_from_parts;

const COMPACTION_SECTION_SCHEMA: &str =
    "Use Markdown headings only when helpful. Do not force all sections.\n\
Keep these section titles unchanged when you use them so later compaction can recognize them:\n\
- Active Request\n\
- Required References\n\
Optional supporting section titles:\n\
- Stable Context\n\
- Key Decisions & Constraints\n\
- Current State\n\
- Recent Tool Results\n\
- Next Actions";

const COMPACTION_RULES: &[&str] = &[
    "Write a dense handoff for the next coding turn. Brief bullets or short note fragments are both fine.",
    "Preserve enough detail to resume safely: durable facts, decisions, constraints, user preferences, unresolved work, and exact file paths or identifiers.",
    "You do not need to emit every possible section. Omit empty or low-value sections.",
    "Active Request: keep only the current unresolved user ask. Remove resolved asks.",
    "Required References: keep only the minimum paths, symbols, or IDs needed for the active request.",
    "Put volatile details in Current State, Recent Tool Results, or Next Actions.",
    "If a section has little to say, keep it brief instead of adding filler.",
    "Never call tools, suggest tool use, or write meta commentary about what you will do.",
];

const COMPACTION_SECTION_LIMITS: &[&str] = &[
    "Keep the summary compact, but completeness matters more than rigid symmetry.",
    "Usually 1-5 bullets or note fragments per section.",
    "Active Request: at most 4 items.",
    "Required References: at most 5 items.",
];

const COMPACTION_OUTPUT_CONSTRAINT: &str =
    "IMPORTANT: Output only the compact summary. Do not call tools, propose tool calls, ask for verification, or describe your process.";

const INCREMENTAL_COMPACTION_RESIDUAL_PREFIX: &str =
    "The first message is the prior compact summary for all earlier history.\n\
Preserve its durable facts, decisions, and constraints when merging the newer messages.\n\
Update Active Request only if the newer messages clearly refine or resolve it.";

pub(super) const ACTIVE_REQUEST_BULLET_LIMIT: usize = 4;
pub(super) const REQUIRED_REFERENCE_BULLET_LIMIT: usize = 5;
pub(super) const REFERENCE_CONTEXT_WINDOW_MESSAGES: usize = 8;
// Character cap for individual hint bullets. This keeps the instruction block
// bounded and biased toward terse operational seeds rather than copying large
// raw request paragraphs into the compaction prompt.
pub(super) const INSTRUCTION_HINT_TEXT_LIMIT: usize = 320;

#[derive(Clone, Copy)]
pub(super) struct CompactionInstructionTemplateInput<'a> {
    pub has_prior_summary: bool,
    pub prior_summary: Option<&'a Message>,
    pub latest_external_request_messages: &'a [Message],
    pub reference_context_messages: &'a [Message],
}

fn render_bulleted_lines(lines: &[&str]) -> String {
    lines
        .iter()
        .map(|line| format!("- {}", line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_base_compaction_instruction() -> String {
    format!(
        "Summarise the previous conversation history into a compact technical handoff for later resume.\n\n{}\n\nRules:\n{}\n\nLimits:\n{}\n\n{}",
        COMPACTION_SECTION_SCHEMA,
        render_bulleted_lines(COMPACTION_RULES),
        render_bulleted_lines(COMPACTION_SECTION_LIMITS),
        COMPACTION_OUTPUT_CONSTRAINT
    )
}

fn build_compaction_hint_block(input: CompactionInstructionTemplateInput<'_>) -> Option<String> {
    let hints = build_compaction_preservation_hints_from_parts(
        input.prior_summary,
        input.latest_external_request_messages,
        input.reference_context_messages,
    );
    if hints.active_request.is_empty() && hints.required_references.is_empty() {
        return None;
    }

    let mut parts = Vec::new();
    if !hints.active_request.is_empty() {
        parts.push(format!(
            "Active Request seed:\n{}",
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
            "Required References seed:\n{}",
            hints
                .required_references
                .iter()
                .map(|line| format!("- {}", line))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    Some(format!(
        "Use these seeds if helpful:\n{}",
        parts.join("\n\n")
    ))
}

pub(super) fn build_compaction_instruction(
    input: CompactionInstructionTemplateInput<'_>,
) -> String {
    let mut instruction = build_base_compaction_instruction();
    if let Some(hint_block) = build_compaction_hint_block(input) {
        instruction = format!("{}\n\n{}", instruction, hint_block);
    }

    if input.has_prior_summary {
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
