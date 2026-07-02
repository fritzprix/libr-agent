use tauri_mcp_agent_lib::agent::session_manager::validate_compact_summary_for_testing;

#[test]
fn accepts_plain_markdown_compaction_summary() {
    let summary = r#"## Active Request
- Continue the Korea market analysis for today.

## Required References
- src/context/llm/compact-listener.ts
- src-tauri/src/agent/session_manager/compact/summary.rs

## Next Actions
- Re-run compaction with stricter output constraints.
"#;

    assert!(validate_compact_summary_for_testing(summary, 4).is_ok());
}

#[test]
fn rejects_tool_call_markup_in_compaction_summary() {
    let summary = r#"## Active Request
- Continue the Korea market analysis for today.

<tool_call>
<function=yahoo-finance__get_quotes>
"#;

    let error = validate_compact_summary_for_testing(summary, 4)
        .expect_err("tool-call markup must invalidate compaction output");
    assert!(error.contains("tool-call markup or execution payload"));
}

#[test]
fn rejects_json_tool_call_payload_in_compaction_summary() {
    let summary = r#"## Active Request
- Continue the Korea market analysis for today.

{"tool_calls":[{"id":"call_1","function":{"name":"readFile","arguments":"{}"}}]}
"#;

    let error = validate_compact_summary_for_testing(summary, 4)
        .expect_err("JSON tool-call payload must invalidate compaction output");
    assert!(error.contains("tool-call markup or execution payload"));
}
