use crate::mcp::builtin::tool_description::tool_description;
use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

/// Render arbitrary content (HTML or Markdown) with interactive elements
pub fn present_interactive_tool() -> MCPTool {
    let interaction_schema = object_prop(
        vec![
            (
                "type".to_string(),
                enum_prop_required(
                    vec!["text", "select", "multiselect"],
                    "Type of interaction UI to display",
                ),
            ),
            (
                "prompt".to_string(),
                string_prop_required("The question or instruction to show the user"),
            ),
            (
                "options".to_string(),
                array_schema(
                    string_prop(None, None, None),
                    Some("Options for select/multiselect (required for those types)"),
                ),
            ),
        ],
        vec!["type".to_string(), "prompt".to_string()],
        Some(
            "Optional user interaction after rendering content. Omit for display-only UI. Use type=text for free-form answers; select/multiselect require options.",
        ),
    );

    MCPTool {
        name: "presentInteractive".to_string(),
        title: Some("Present Interactive Content".to_string()),
        description: tool_description(
            "Render HTML or Markdown in the chat UI, optionally asking the user a follow-up question.",
            &[
                "Use when you still need the user to see content and/or answer something.",
                "If the task is fully done and you only need to deliver a final result, use ui__reportResult instead.",
            ],
            &[
                "For display-only content, omit `interaction`.",
                "For a user response after the content, include `interaction` (text / select / multiselect).",
                "HTML mode supports a safe subset only: basic text, tables, and links. JavaScript, CSS, event handlers, images, and arbitrary embeds are stripped.",
            ],
            &[
                "If the user must reply, wait for their UI response before continuing.",
                "When no further work remains, call ui__reportResult and then stop.",
            ],
        ),
        // Field order is intentional: format → title → interaction → content.
        // Models often emit arguments in schema order; keep large HTML/Markdown last.
        input_schema: object_prop(
            vec![
                (
                    "format".to_string(),
                    enum_prop(
                        vec!["html", "markdown", "auto"],
                        "auto",
                        Some("Content format. 'auto' defaults to Markdown; use 'html' for raw HTML rendering"),
                    ),
                ),
                (
                    "title".to_string(),
                    string_prop(None, None, Some("Optional title displayed above the content")),
                ),
                ("interaction".to_string(), interaction_schema),
                (
                    "content".to_string(),
                    string_prop_required("The content string to render (HTML or Markdown)"),
                ),
            ],
            vec!["content".to_string()],
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
            ],
            &[
                "Call this exactly once when the outcome is ready.",
                "Put the complete user-facing result in `result` (summary + key outputs/paths). Prefer Markdown.",
                "For long reports: write the full document with workspace__writeFile (e.g. `.libragent/work/report.md`), put a short summary in `result`, and attach the path via `export_paths`. Do not paste the full report into assistant chat.",
                "Never tell the user the deliverable was saved to scratchpad — scratchpad is private working memory, not a shareable output.",
                "When the request has checkable acceptance criteria, put them in `criteria` and verification evidence (or unmet gaps) in `proof`. Skip both when the outcome is not objectively verifiable.",
                "If your task produced deliverable files, pass their workspace-relative paths to `export_paths` to attach them with Preview/Open/Download actions.",
                "After this tool returns: stop. Do not call any more tools. End your turn with at most a one-sentence confirmation.",
            ],
            &[
                "If you still need user input, use ui__presentInteractive with `interaction` instead.",
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
                        vec!["html", "markdown", "auto"],
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
/// Note: Internal callback tools (getUserAnswer, circuitBreak, resumeCircuitBreak)
/// are NOT included here to prevent the AI from hallucinating calls to them.
pub fn all_tools() -> Vec<MCPTool> {
    vec![present_interactive_tool(), report_result_tool()]
}
