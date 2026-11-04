use crate::mcp::builtin::tool_description::tool_description;
use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

/// Ask the user one or more questions and wait for all answers.
pub fn ask_user_tool() -> MCPTool {
    let question_schema = object_prop(
        vec![
            (
                "id".to_string(),
                string_prop_required("Unique key for this question in the call"),
            ),
            (
                "question".to_string(),
                string_prop_required("The question to show the user"),
            ),
            (
                "description".to_string(),
                string_prop(None, None, Some("Optional detail shown under the question")),
            ),
            (
                "options".to_string(),
                array_schema(
                    string_prop(None, None, None),
                    Some("Choices. Omit for a free-text question"),
                ),
            ),
            (
                "multiSelect".to_string(),
                boolean_prop_with_default(false, Some("Allow more than one choice; needs options")),
            ),
            (
                "allowCustom".to_string(),
                boolean_prop_with_default(
                    true,
                    Some("Add an \"Other\" free-text choice to options"),
                ),
            ),
        ],
        vec!["id".to_string(), "question".to_string()],
        None,
    );

    MCPTool {
        name: "askUser".to_string(),
        title: Some("Ask User".to_string()),
        description: tool_description(
            "Ask the user 1-5 questions in one dialog and wait for all answers.",
            &[
                "Use when you cannot continue without a decision or fact only the user has.",
                "Do not use it to show content: write Markdown in chat, or use ui__reportResult for the final result.",
            ],
            &[
                "Every question is required. With `options` it is a choice; without, free text.",
                "Never ask for passwords or secrets here: use workspace__runShell with requireUserInput=true, inputType=password, and storeAs.",
            ],
            &[
                "The call returns one JSON object keyed by question id: {\"<id>\": {\"question\": ..., \"answer\": ...}}.",
                "If the user cancels or does not answer, continue with your best judgment or stop; do not ask the same thing again at once.",
            ],
        ),
        input_schema: object_prop(
            vec![(
                "questions".to_string(),
                array_schema_with_max_items(
                    question_schema,
                    Some(5),
                    Some("1 to 5 questions"),
                ),
            )],
            vec!["questions".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
        libragent_wait: None,
    }
}

/// Explicit terminal deliverable: show the final result and stop tool use.
pub fn report_result_tool() -> MCPTool {
    MCPTool {
        name: "reportResult".to_string(),
        title: Some("Report Final Result".to_string()),
        description: tool_description(
            "Deliver the final task result to the user when there is nothing left to do. This is the explicit completion signal — not for mid-task updates.",
            &[
                "You are not waiting on another tool, process, or user clarification.",
                "Do NOT use this while still exploring, debugging, or planning — finish the work first, then call once.",
                "After compaction: do not treat empty Active Request or satisfied Next Actions alone as Done — confirm Working Intent / Recent User Requests first.",
            ],
            &[
                "Call this exactly once when the outcome is ready.",
                "Put the complete user-facing result in `result` (summary + key outputs/paths). Prefer Markdown.",
                "For long reports: write the full document with workspace__writeFile (e.g. `.libragent/work/report.md`), put a short summary in `result`, and attach the path via `export_paths`. Do not paste the full report into assistant chat.",
                "Never tell the user the deliverable was saved to scratchpad — scratchpad is private working memory, not a shareable output.",
                "When the request has checkable acceptance criteria, put them in `criteria` and verification evidence (or unmet gaps) in `proof`. Skip both when the outcome is not objectively verifiable.",
                "If a compact summary is in context and intent/criteria are unclear or unverified, read the latest `.libragent/pre_compaction_epoch_*.md` (context-recall) before `status=success`.",
                "If your task produced deliverable files, pass their workspace-relative paths to `export_paths` to attach them with Preview/Open/Download actions.",
                "For status=success, every `export_paths` entry must already exist on disk — write files first; missing paths reject success without settling the session.",
                "On status=success, session planning (goal, todos, scratchpad) is cleared so the next request starts clean — recreate a goal if follow-up work continues in the same session.",
                "After a successful return: stop. Do not call any more tools. End your turn with at most a one-sentence confirmation.",
            ],
            &[
                "If you still need user input, use ui__askUser instead.",
                "If work remains, continue with tools — do not call reportResult early.",
            ],
        ),
        // Keep large body last for model argument ordering.
        input_schema: object_prop(
            vec![
                (
                    "status".to_string(),
                    enum_prop(
                        vec!["success", "partial", "blocked"],
                        "success",
                        Some(
                            "Outcome status: success = fully done; partial = best-effort with known gaps; blocked = cannot finish without external help",
                        ),
                    ),
                ),
                (
                    "format".to_string(),
                    enum_prop(
                        vec!["markdown", "auto"],
                        "auto",
                        Some("Result format. 'auto' defaults to Markdown"),
                    ),
                ),
                (
                    "title".to_string(),
                    string_prop(
                        None,
                        None,
                        Some("Optional short title for the result panel"),
                    ),
                ),
                (
                    "criteria".to_string(),
                    string_prop(
                        None,
                        None,
                        Some(
                            "Optional. Checkable acceptance criteria from the user request when they exist; omit when the outcome is not objectively verifiable",
                        ),
                    ),
                ),
                (
                    "proof".to_string(),
                    string_prop(
                        None,
                        None,
                        Some(
                            "Optional. Evidence that criteria were checked, or unmet gaps for partial/blocked; omit when criteria is omitted",
                        ),
                    ),
                ),
                (
                    "export_paths".to_string(),
                    array_schema(
                        string_prop(Some(1), Some(1000), None),
                        Some("Optional workspace-relative paths of final deliverable files (e.g. outputs/report.pdf) to attach to the final result"),
                    ),
                ),
                (
                    "result".to_string(),
                    string_prop_required(
                        "Final user-facing result text (what was accomplished, key outputs, paths, and any caveats)",
                    ),
                ),
            ],
            vec!["result".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
        libragent_wait: None,
    }
}

/// Returns all UI tools intended for the AI agent
/// Note: Internal callback tools (circuitBreak, resumeCircuitBreak)
/// are NOT included here to prevent the AI from hallucinating calls to them.
pub fn all_tools() -> Vec<MCPTool> {
    vec![ask_user_tool(), report_result_tool()]
}
