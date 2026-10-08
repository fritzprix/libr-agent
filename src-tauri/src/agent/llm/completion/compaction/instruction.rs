use crate::mcp::types::MCPContent;
use crate::models::chat::{Message, MessageSource};

use super::hints::build_compaction_preservation_hints_from_parts;

const COMPACTION_SECTION_SCHEMA: &str = "Write plain Markdown summary text for a later resume.\n\
 Use headings only when helpful. Do not force all sections.\n\
Keep these section titles unchanged when you use them so later compaction can recognize them:\n\
- Target Deliverable\n\
- Progress\n\
- Completion Criteria\n\
- Working Intent\n\
- Active Request\n\
- Required References\n\
- Next Actions\n\
Optional supporting section titles:\n\
- Stable Context\n\
- Key Decisions & Constraints\n\
- Current State\n\
- Recent Tool Results\n\
Do not invent a Recent User Requests section — the runtime appends that ledger deterministically.";

const COMPACTION_RULES: &[&str] = &[
    "Pause first. You are not continuing the workflow; you are only compressing it into a handoff.",
    "Write a dense handoff for later resume. Brief bullets or short note fragments are fine.",
    "Keep only the details needed to resume safely: durable facts, decisions, constraints, user preferences, unresolved work, and exact file paths or identifiers.",
    "Lead with the end-state: Target Deliverable, Working Intent, and Completion Criteria before local next steps.",
    "You do not need to emit every possible section. Omit empty or low-value sections, and keep short sections brief.",
    "Target Deliverable: list the CONCRETE final outputs the user wants. Not steps — the actual things the user cares about. Prefer paths and names already stated in the workspace or user request.",
    "Progress: if the workflow has clear phases or milestones, include a Markdown table with columns Phase, Status, Notes. Use status values Done, In Progress, Not Started, or Blocked. If the workflow is too fluid, omit this section.",
    "Progress / criteria status: when a phase or criterion requires a named on-disk file deliverable, mark Done or check it off only if conversation or tool results confirm that path was written or already exists. Collecting values or preparing content is not enough — keep In Progress / unchecked and put the missing write in Active Request. For non-file milestones, mark Done/checked from clear conversation evidence of completion.",
    "Completion Criteria: list verifiable checkpoints that define done. Prefer checkbox lines like `- [ ] ...`. Each criterion must be objectively testable. Check off criteria that are already satisfied; remove only criteria that are obsolete or superseded.",
    "Working Intent: one short bullet stating what the user still wants, derived from recent external user requests. Next Actions must never outrank Working Intent.",
    "Active Request: keep only the current unresolved operational residue of that intent. Remove resolved asks. Empty Active Request alone never means Done.",
    "Required References: keep only the minimum paths, symbols, or IDs needed for the active request.",
    "Put fast-changing details in Current State, Recent Tool Results, or Next Actions.",
    // Lossless grounding: keep values; do not invent deliverable contracts.
    "Structured snippets (including fenced JSON) may preserve exact collected values, paths, or identifiers that already appeared in the conversation or workspace.",
    "Never invent a deliverable file schema. Do not turn object/array shape, property names, or example file bodies into Completion Criteria or Active Request contracts unless that exact schema already appears in a workspace instruction or an existing on-disk file.",
    "If a required output file already exists, treat that file as source of truth: record its path and status; do not replace it with a redesigned example body.",
    "When values are collected but the file is not written yet, label any data snippet as working notes — not as the required file format. Deliverable format follows the original workspace/user instructions.",
    "Do not call tools. Even if tool definitions are visible, ignore them for this request.",
    "Do not emit XML, pseudo tool-call markup, shell command blocks, or meta commentary about the summarization process.",
    "Do not tell the agent to continue from later messages or otherwise prime continuation. Empty Active Request alone is never enough to mark Done — treat work as complete only when Working Intent / recent user requests are clearly fulfilled AND Completion Criteria are checked (or there are none). If intent is unclear or unverified, keep Active Request or note Needs context-recall.",
];

const COMPACTION_SECTION_LIMITS: &[&str] = &[
    "Keep the summary compact, but completeness matters more than rigid symmetry.",
    "Usually 1-5 bullets or short note fragments per section.",
    "Target Deliverable: at most 6 items.",
    "Progress: at most 10 phases.",
    "Completion Criteria: at most 8 items.",
    "Working Intent: at most 2 items.",
    "Active Request: at most 4 items.",
    "Required References: at most 5 items.",
];

const COMPACTION_OUTPUT_CONSTRAINT: &str =
    "IMPORTANT: Output only the compact summary. Stay in summary mode. Do not call tools, propose tool calls, ask for verification, or describe your process.";

const INCREMENTAL_COMPACTION_RESIDUAL_PREFIX: &str =
    "The first message is the prior compact summary for all earlier history.\n\
Keep its Target Deliverable, Completion Criteria, Working Intent, Active Request, and Required References unless newer messages clearly replace or resolve them.\n\
Merge deliverables and criteria across rounds; check off satisfied criteria, remove only obsolete/superseded ones, and refresh Progress status from newer messages.\n\
Do not promote a Progress row to Done or check off a file-deliverable criterion unless newer messages or tool results confirm the named path was written or already exists.\n\
Preserve its durable facts, decisions, and constraints when merging the newer messages.\n\
If the prior summary invented a deliverable schema that is not grounded in workspace/user instructions or an existing file, drop that schema while keeping grounded facts and paths.";
pub(super) const ACTIVE_REQUEST_BULLET_LIMIT: usize = 4;
pub(super) const REQUIRED_REFERENCE_BULLET_LIMIT: usize = 5;
pub(super) const REFERENCE_CONTEXT_WINDOW_MESSAGES: usize = 8;
// Character cap for individual hint bullets. This keeps the instruction block
// bounded and biased toward terse operational seeds rather than copying large
// raw request paragraphs into the compaction prompt.
pub(super) const INSTRUCTION_HINT_TEXT_LIMIT: usize = 160;

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
    // Active Request seeds already capture the latest external-request block;
    // reuse them as Working Intent grounding so Next Actions cannot outrank intent.
    if hints.active_request.is_empty() && hints.required_references.is_empty() {
        return None;
    }

    let mut parts = Vec::new();
    if !hints.active_request.is_empty() {
        parts.push(format!(
            "Working Intent / recent user-request seed (derive Working Intent from these; do not invent conflicting intent):\n{}",
            hints
                .active_request
                .iter()
                .map(|line| format!("- {}", line))
                .collect::<Vec<_>>()
                .join("\n")
        ));
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
        content: vec![MCPContent::Text { text: instruction }],
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
