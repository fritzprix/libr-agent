use serde_json::json;
use std::collections::HashMap;
use tauri_mcp_agent_lib::agent::compaction_text::{
    is_compaction_artifact_line, sanitize_compaction_semantic_text,
};
use tauri_mcp_agent_lib::agent::llm::completion::{
    advance_compaction_overflow_recovery_step_for_testing, apply_compaction_retry_budget,
    build_checkpoint_backoff_split_candidates_for_testing, build_compact_context_selection_options,
    build_compact_summary_message_for_messages, build_compact_summary_text,
    build_compaction_preservation_hints, build_compaction_request_payload_for_testing,
    build_overflow_recovery_compaction_messages,
    derive_tail_recompaction_recovery_plan_for_testing,
    find_preflight_compactable_end_exclusive_for_testing, fit_compaction_request_messages_to_limit,
    has_prompt_checkpoint_compaction_target, inspect_compaction_payload,
    merge_consecutive_user_messages, normalize_request_messages,
    preview_preflight_compaction_selection, resolve_context_management_settings,
    resolve_preserved_calibration_ratio, should_skip_same_tail_compaction,
    uses_compaction_strategy,
};
use tauri_mcp_agent_lib::agent::llm::context_selector::*;
use tauri_mcp_agent_lib::agent::llm::token_utils::*;
use tauri_mcp_agent_lib::agent::llm::types::CompactionParentRequest;
use tauri_mcp_agent_lib::agent::session_manager::{
    build_compaction_hard_fallback_summary_for_testing, clamp_compact_summary_to_context_limit,
    clear_message_prompt_token_checkpoint_for_testing,
    compaction_fallback_artifact_relative_path_for_testing, validate_compact_summary_for_testing,
};
use tauri_mcp_agent_lib::agent::state::CompactionRecoveryPhase;
use tauri_mcp_agent_lib::agent::types::{ToolCall as AgentToolCall, ToolCallFunction};
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::{Message, MessageSource};
use tauri_mcp_agent_lib::repositories::CompactContextRecord;

const TEST_SESSION_ID: &str = "test-session";

struct TestMessageBuilder {
    message: Message,
}

impl TestMessageBuilder {
    fn new(id: &str, role: &str) -> Self {
        Self {
            message: Message {
                id: id.to_string(),
                session_id: TEST_SESSION_ID.to_string(),
                role: role.to_string(),
                content: Vec::new(),
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
            },
        }
    }

    fn text(mut self, text: &str) -> Self {
        self.message.content = vec![MCPContent::Text {
            text: text.to_string(),
            is_error: None,
        }];
        self
    }

    fn source(mut self, source: MessageSource) -> Self {
        self.message.source = Some(source);
        self
    }

    fn prompt_tokens(mut self, prompt_tokens: i64) -> Self {
        self.message.prompt_tokens = Some(prompt_tokens);
        self
    }

    fn completion_tokens(mut self, completion_tokens: usize) -> Self {
        let mut usage = self.message.usage.unwrap_or_else(|| json!({}));
        if let Some(object) = usage.as_object_mut() {
            object.insert("completionTokens".to_string(), json!(completion_tokens));
        }
        self.message.usage = Some(usage);
        self
    }

    fn build(self) -> Message {
        self.message
    }
}

fn make_message(id: &str, role: &str, text: &str) -> Message {
    TestMessageBuilder::new(id, role).text(text).build()
}

fn make_message_simple(role: &str, text: &str) -> Message {
    make_message(&format!("msg-{}", text.len()), role, text)
}

fn text_content_parts(message: &Message) -> Vec<&str> {
    message
        .content
        .iter()
        .filter_map(|part| match part {
            MCPContent::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn make_compact_summary_message(id: &str, role: &str, text: &str) -> Message {
    TestMessageBuilder::new(id, role)
        .text(text)
        .source(MessageSource::CompactSummary)
        .build()
}

fn make_compaction_instruction_message(id: &str, text: &str) -> Message {
    TestMessageBuilder::new(id, "user")
        .text(text)
        .source(MessageSource::CompactionInstruction)
        .build()
}

#[test]
fn test_find_compaction_split_index() {
    let mut msgs = vec![];
    for i in 0..20 {
        msgs.push(make_message(&format!("msg{}", i), "user", "Short message"));
    }
    // Function always returns messages.len() — compact everything
    let idx = find_compaction_split_index(&msgs);
    assert_eq!(idx, 20);
}

#[test]
fn test_find_compaction_split_index_with_calibration() {
    let mut msgs = vec![];
    for i in 0..10 {
        let mut msg = make_message(&format!("msg{}", i), "assistant", "Test content");
        if i == 5 {
            msg.usage = Some(json!({ "totalTokens": 20000 }));
        }
        msgs.push(msg);
    }
    // Function always returns messages.len() — compact everything
    let idx = find_compaction_split_index(&msgs);
    assert_eq!(idx, 10);
}

#[test]
fn test_find_compaction_split_index_stops_before_unresolved_tool_chain() {
    let intro = make_message("m0", "user", "Need analysis");

    let mut assistant = make_message("m1", "assistant", "Calling tools");
    assistant.tool_calls = Some(vec![
        AgentToolCall {
            id: "call_A".to_string(),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: "toolA".to_string(),
                arguments: "{}".to_string(),
            },
        },
        AgentToolCall {
            id: "call_B".to_string(),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: "toolB".to_string(),
                arguments: "{}".to_string(),
            },
        },
    ]);

    let mut tool_result = make_message("m2", "tool", "result A");
    tool_result.tool_call_id = Some("call_A".to_string());

    let idx = find_compaction_split_index(&[intro, assistant, tool_result]);
    assert_eq!(idx, 1);
}

#[test]
fn test_find_preflight_compaction_split_index_absorbs_latest_user_turn_in_normal_path() {
    let earlier = make_message("m0", "assistant", "Earlier context");
    let latest_user = make_message("m1", "user", &"latest request ".repeat(200));

    let preview = preview_preflight_compaction_selection(&[earlier, latest_user]);
    assert_eq!(preview.compacted_ids, vec!["m0", "m1"]);
    assert!(preview.preserved_ids.is_empty());
}

#[test]
fn test_find_preflight_compaction_split_index_absorbs_latest_non_tool_turn_in_normal_path() {
    let earlier = make_message("m0", "user", "Earlier user context");
    let latest_assistant = make_message("m1", "assistant", "Latest non-tool turn");

    let preview = preview_preflight_compaction_selection(&[earlier, latest_assistant]);
    assert_eq!(preview.compacted_ids, vec!["m0", "m1"]);
    assert!(preview.preserved_ids.is_empty());
}

#[test]
fn test_find_preflight_compaction_split_index_allows_compacting_latest_tool_result() {
    let intro = make_message("m0", "user", "Need analysis");

    let mut assistant = make_message("m1", "assistant", "Calling tool");
    assistant.tool_calls = Some(vec![AgentToolCall {
        id: "call_A".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "toolA".to_string(),
            arguments: "{}".to_string(),
        },
    }]);

    let mut tool_result = make_message("m2", "tool", &"very large tool result ".repeat(200));
    tool_result.tool_call_id = Some("call_A".to_string());

    let preview = preview_preflight_compaction_selection(&[intro, assistant, tool_result]);
    assert_eq!(preview.compacted_ids, vec!["m0", "m1", "m2"]);
    assert!(preview.preserved_ids.is_empty());
}

#[test]
fn test_find_preflight_compaction_split_index_keeps_unresolved_tool_chain_tail() {
    let intro = make_message("m0", "user", "Need analysis");

    let mut assistant = make_message("m1", "assistant", "Calling tools");
    assistant.tool_calls = Some(vec![
        AgentToolCall {
            id: "call_A".to_string(),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: "toolA".to_string(),
                arguments: "{}".to_string(),
            },
        },
        AgentToolCall {
            id: "call_B".to_string(),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: "toolB".to_string(),
                arguments: "{}".to_string(),
            },
        },
    ]);

    let mut tool_result = make_message("m2", "tool", "result A");
    tool_result.tool_call_id = Some("call_A".to_string());

    let preview = preview_preflight_compaction_selection(&[intro, assistant, tool_result]);
    assert_eq!(preview.compacted_ids, vec!["m0"]);
    assert_eq!(preview.preserved_ids, vec!["m1", "m2"]);
}

#[test]
fn test_preview_preflight_compaction_selection_handles_empty_messages() {
    let preview = preview_preflight_compaction_selection(&[]);
    assert!(preview.compacted_ids.is_empty());
    assert!(preview.preserved_ids.is_empty());
}

#[test]
fn test_build_compaction_request_payload_returns_none_for_zero_split_idx() {
    let compact_record = CompactContextRecord {
        id: "compact-1".to_string(),
        session_id: TEST_SESSION_ID.to_string(),
        to_id: "m1".to_string(),
        condensed_count: Some(2),
        summary: "Existing summary".to_string(),
        created_at: 123,
    };

    let payload = build_compaction_request_payload_for_testing(
        TEST_SESSION_ID,
        &[],
        0,
        Some(&compact_record),
        456,
    );

    assert!(payload.is_none());
}

#[test]
fn test_build_compaction_request_payload_falls_back_when_compact_record_to_id_is_missing() {
    let messages = vec![
        make_message("m0", "user", "Earlier context"),
        make_message("m1", "assistant", "Latest context"),
    ];
    let compact_record = CompactContextRecord {
        id: "compact-1".to_string(),
        session_id: TEST_SESSION_ID.to_string(),
        to_id: "missing-message".to_string(),
        condensed_count: Some(2),
        summary: "Stale summary".to_string(),
        created_at: 123,
    };

    let payload = build_compaction_request_payload_for_testing(
        TEST_SESSION_ID,
        &messages,
        2,
        Some(&compact_record),
        456,
    )
    .expect("payload should fall back to raw-prefix compaction");

    assert_eq!(payload.message_count, 3);
    assert_eq!(payload.to_id, "m1");
    assert_eq!(payload.compacted_delta_count, 2);
    assert!(!payload.reused_prior_summary);
}

#[test]
fn test_build_compaction_request_payload_injects_latest_external_request_outside_body_window() {
    let earlier_context = make_message("m0", "assistant", "Earlier compaction body");
    let older_request = make_message("m1", "user", "Compact the older context.");
    let mut latest_request = make_message(
        "m2",
        "user",
        "After compaction, keep `src/agent/instruction.rs` and `Active Request` visible.",
    );
    latest_request.source = Some(MessageSource::Ui);

    let payload = build_compaction_request_payload_for_testing(
        TEST_SESSION_ID,
        &[earlier_context, older_request, latest_request],
        2,
        None,
        456,
    )
    .expect("payload should be built");

    assert!(
        payload
            .instruction_text
            .contains("src/agent/instruction.rs"),
        "instruction should preserve the latest external request even when it is outside the compacted body"
    );
    assert!(
        payload.instruction_text.contains("After compaction"),
        "instruction should still include the latest external request seed text outside the body window"
    );
}

#[test]
fn test_build_compaction_request_payload_incremental_path_injects_latest_external_request_outside_delta(
) {
    let messages = vec![
        make_message("m0", "user", "Already compacted user turn"),
        make_message("m1", "assistant", "Already compacted assistant turn"),
        make_message("m2", "assistant", "Delta body context before the latest request"),
        make_message("m3", "user", "Compact the active delta state first."),
        make_message("m4", "assistant", "Delta response before the latest external request"),
        TestMessageBuilder::new("m5", "user")
            .text(
                "After compaction, keep `src-tauri/src/agent/llm/completion/compaction/payload.rs` visible for the next step.",
            )
            .source(MessageSource::Ui)
            .build(),
    ];
    let compact_record = CompactContextRecord {
        id: "compact-1".to_string(),
        session_id: TEST_SESSION_ID.to_string(),
        to_id: "m1".to_string(),
        condensed_count: Some(2),
        summary: "Prior compact summary".to_string(),
        created_at: 123,
    };

    let payload = build_compaction_request_payload_for_testing(
        TEST_SESSION_ID,
        &messages,
        5,
        Some(&compact_record),
        456,
    )
    .expect("payload should be built");

    assert!(
        payload.reused_prior_summary,
        "incremental compaction path should reuse the prior summary"
    );
    assert!(
        payload
            .instruction_text
            .contains("Keep its Active Request and Required References unless newer messages clearly replace or resolve them."),
        "incremental compaction should explicitly preserve prior active-request and reference anchors"
    );
    assert!(
        payload
            .instruction_text
            .contains("src-tauri/src/agent/llm/completion/compaction/payload.rs"),
        "instruction should preserve the latest external request even when it is outside the incremental compacted delta"
    );
    assert!(
        payload.instruction_text.contains("After compaction"),
        "instruction should still include the latest external request seed text outside the incremental delta"
    );
}

#[test]
fn test_build_compaction_request_payload_uses_simplified_instruction_template() {
    let latest_request = TestMessageBuilder::new("m0", "user")
        .text("Keep `src-tauri/src/agent/llm/completion/compaction/instruction.rs` visible.")
        .source(MessageSource::Ui)
        .build();
    let assistant = make_message("m1", "assistant", "Reviewing the compaction template.");

    let payload = build_compaction_request_payload_for_testing(
        TEST_SESSION_ID,
        &[latest_request, assistant],
        2,
        None,
        456,
    )
    .expect("payload should be built");

    assert!(
        payload
            .instruction_text
            .contains("Write plain Markdown summary text for a later resume."),
        "instruction should open with a simpler plain-summary directive"
    );
    assert!(
        payload
            .instruction_text
            .contains("Use headings only when helpful."),
        "instruction should stop forcing every summary into a rigid heading template"
    );
    assert!(
        payload
            .instruction_text
            .contains("Keep these section titles unchanged"),
        "instruction should still preserve parser-critical section anchors"
    );
    assert!(
        payload.instruction_text.contains("- Active Request"),
        "instruction should keep the Active Request anchor visible for downstream parsing"
    );
    assert!(
        payload
            .instruction_text
            .contains("You do not need to emit every possible section."),
        "instruction should explicitly allow omitting low-value sections"
    );
    assert!(
        payload
            .instruction_text
            .contains("Pause first. You are not continuing the workflow"),
        "instruction should nudge the model to stop and summarize instead of continuing execution"
    );
    assert!(
        payload
            .instruction_text
            .contains("Even if tool definitions are visible, ignore them"),
        "instruction should explicitly forbid tool use even when schemas are visible"
    );
    assert!(
        payload
            .instruction_text
            .contains("Do not emit XML, JSON, pseudo tool-call markup"),
        "instruction should block fake tool-call markup from leaking into the summary"
    );
    assert!(
        payload
            .instruction_text
            .contains("Use these seeds if helpful:"),
        "instruction should keep the preservation seeds in a compact form"
    );
    assert!(
        !payload.instruction_text.contains("Compression rules:"),
        "instruction should drop the older verbose compression block label"
    );
    assert!(
        !payload
            .instruction_text
            .contains("Preservation hints for this compaction input:"),
        "instruction should drop the older verbose hint wrapper"
    );
    assert!(
        payload
            .instruction_text
            .contains("Omit empty or low-value sections, and keep short sections brief."),
        "instruction should prefer sufficient information over rigid section padding"
    );
}

#[test]
fn test_preflight_compactable_end_ignores_unresolved_tool_chain_before_to_id() {
    let mut older_assistant = make_message("m0", "assistant", "Older unresolved tool call");
    older_assistant.tool_calls = Some(vec![AgentToolCall {
        id: "call_old".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "workspace__readFile".to_string(),
            arguments: "{\"path\":\"old.txt\"}".to_string(),
        },
    }]);

    let messages = vec![
        older_assistant,
        make_message("m1", "user", "Already compacted user turn"),
        make_message("m2", "assistant", "Already compacted assistant turn"),
        make_message("m3", "assistant", "Fresh post-summary delta"),
        make_message("m4", "user", "Latest request"),
    ];
    let compact_record = CompactContextRecord {
        id: "compact-1".to_string(),
        session_id: TEST_SESSION_ID.to_string(),
        to_id: "m2".to_string(),
        condensed_count: Some(2),
        summary: "Existing summary".to_string(),
        created_at: 123,
    };

    let compactable_end_exclusive = find_preflight_compactable_end_exclusive_for_testing(
        &messages,
        Some(&compact_record),
        None,
    );

    assert_eq!(compactable_end_exclusive, messages.len());
}

#[test]
fn test_preflight_compactable_end_uses_prompt_token_checkpoint_window() {
    let messages = vec![
        TestMessageBuilder::new("m0", "user")
            .text("checkpoint 0")
            .prompt_tokens(12_000)
            .build(),
        TestMessageBuilder::new("m1", "assistant")
            .text("checkpoint 1")
            .prompt_tokens(24_000)
            .build(),
        TestMessageBuilder::new("m2", "user")
            .text("checkpoint 2")
            .prompt_tokens(36_000)
            .build(),
        TestMessageBuilder::new("m3", "assistant")
            .text("checkpoint 3")
            .prompt_tokens(48_000)
            .build(),
    ];

    let compactable_end_exclusive =
        find_preflight_compactable_end_exclusive_for_testing(&messages, None, Some(30_000));

    assert_eq!(compactable_end_exclusive, 1);
}

#[test]
fn test_preflight_compactable_end_respects_effective_budget_after_output_reserve() {
    let messages = vec![
        TestMessageBuilder::new("m0", "user")
            .text("checkpoint 0")
            .prompt_tokens(12_000)
            .build(),
        TestMessageBuilder::new("m1", "assistant")
            .text("checkpoint 1")
            .prompt_tokens(24_000)
            .build(),
        TestMessageBuilder::new("m2", "user")
            .text("checkpoint 2")
            .prompt_tokens(36_000)
            .build(),
        TestMessageBuilder::new("m3", "assistant")
            .text("checkpoint 3")
            .prompt_tokens(48_000)
            .build(),
    ];

    let full_budget_end =
        find_preflight_compactable_end_exclusive_for_testing(&messages, None, Some(48_000));
    let reserve_adjusted_end =
        find_preflight_compactable_end_exclusive_for_testing(&messages, None, Some(36_000));

    assert_eq!(full_budget_end, 4);
    assert_eq!(reserve_adjusted_end, 1);
}

#[test]
fn test_preflight_compactable_end_falls_back_to_latest_checkpoint_when_overflow_window_starts_before_first_checkpoint(
) {
    let messages = vec![
        TestMessageBuilder::new("m0", "user")
            .text("checkpoint 0")
            .prompt_tokens(27_195)
            .build(),
        TestMessageBuilder::new("m1", "assistant")
            .text("checkpoint 1")
            .prompt_tokens(34_492)
            .build(),
        TestMessageBuilder::new("m2", "user")
            .text("checkpoint 2")
            .prompt_tokens(35_271)
            .build(),
        TestMessageBuilder::new("m3", "tool")
            .text("latest checkpoint")
            .prompt_tokens(127_783)
            .build(),
        TestMessageBuilder::new("m4", "assistant")
            .text("post-checkpoint assistant")
            .build(),
        TestMessageBuilder::new("m5", "user")
            .text("latest request")
            .source(MessageSource::Ui)
            .build(),
    ];

    let compactable_end_exclusive =
        find_preflight_compactable_end_exclusive_for_testing(&messages, None, Some(127_165));

    assert_eq!(compactable_end_exclusive, 4);
}

#[test]
fn test_prompt_checkpoint_compaction_target_survives_degenerate_near_overflow_window() {
    let messages = vec![
        TestMessageBuilder::new("m0", "user")
            .text("checkpoint 0")
            .prompt_tokens(27_195)
            .build(),
        TestMessageBuilder::new("m1", "assistant")
            .text("checkpoint 1")
            .prompt_tokens(34_492)
            .build(),
        TestMessageBuilder::new("m2", "user")
            .text("checkpoint 2")
            .prompt_tokens(35_271)
            .build(),
        TestMessageBuilder::new("m3", "tool")
            .text("latest checkpoint")
            .prompt_tokens(127_783)
            .build(),
        TestMessageBuilder::new("m4", "assistant")
            .text("post-checkpoint assistant")
            .build(),
        TestMessageBuilder::new("m5", "user")
            .text("latest request")
            .source(MessageSource::Ui)
            .build(),
    ];

    assert!(has_prompt_checkpoint_compaction_target(
        &messages, None, 127_165
    ));
}

#[test]
fn test_preflight_compactable_end_advances_past_tool_result_when_checkpoint_fallback_would_orphan_tail(
) {
    let mut latest_checkpoint_owner = TestMessageBuilder::new("m1", "assistant")
        .text("assistant tool owner checkpoint")
        .prompt_tokens(127_783)
        .build();
    latest_checkpoint_owner.tool_calls = Some(vec![AgentToolCall {
        id: "call_compaction_1".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "workspace__readFile".to_string(),
            arguments: "{\"path\":\"notes.txt\"}".to_string(),
        },
    }]);
    let mut tool_result = TestMessageBuilder::new("m2", "tool")
        .text("tool result")
        .build();
    tool_result.tool_call_id = Some("call_compaction_1".to_string());

    let messages = vec![
        TestMessageBuilder::new("m0", "assistant")
            .text("older compact summary")
            .source(MessageSource::CompactSummary)
            .build(),
        latest_checkpoint_owner,
        tool_result,
        TestMessageBuilder::new("m3", "user")
            .text("latest request")
            .source(MessageSource::Ui)
            .build(),
    ];
    let compact_record = CompactContextRecord {
        id: "compact-1".to_string(),
        session_id: TEST_SESSION_ID.to_string(),
        to_id: "m0".to_string(),
        condensed_count: Some(1),
        summary: "Existing summary".to_string(),
        created_at: 123,
    };

    let compactable_end_exclusive = find_preflight_compactable_end_exclusive_for_testing(
        &messages,
        Some(&compact_record),
        Some(127_165),
    );

    assert_eq!(compactable_end_exclusive, 3);
    assert!(has_prompt_checkpoint_compaction_target(
        &messages,
        Some(&compact_record),
        127_165
    ));
}

#[test]
fn test_checkpoint_backoff_candidates_skip_orphan_tool_tail_splits() {
    let mut assistant = make_message("m0", "assistant", "Tool call owner");
    assistant.tool_calls = Some(vec![AgentToolCall {
        id: "call_1".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "workspace__readFile".to_string(),
            arguments: "{\"path\":\"src/lib.rs\"}".to_string(),
        },
    }]);
    assistant.prompt_tokens = Some(2_000);
    let mut tool = make_message("m1", "tool", "Tool result");
    tool.tool_call_id = Some("call_1".to_string());

    let messages = vec![
        TestMessageBuilder::new("m-1", "user")
            .text("older checkpoint")
            .prompt_tokens(1_000)
            .build(),
        assistant,
        tool,
        TestMessageBuilder::new("m2", "user")
            .text("latest checkpoint")
            .prompt_tokens(3_000)
            .build(),
    ];

    let candidates =
        build_checkpoint_backoff_split_candidates_for_testing(&messages, None, messages.len());

    assert_eq!(candidates, vec![4, 1]);
}

#[test]
fn test_derive_measured_output_tokens_reserve_prefers_observed_completion_tokens() {
    let messages = vec![
        TestMessageBuilder::new("m0", "assistant")
            .text("Earlier output")
            .completion_tokens(320)
            .build(),
        TestMessageBuilder::new("m1", "assistant")
            .text("Latest output")
            .completion_tokens(2048)
            .build(),
    ];

    assert_eq!(
        derive_measured_output_tokens_reserve(&messages, Some(4096)),
        2048
    );
    assert_eq!(
        derive_measured_output_tokens_reserve(&[], Some(16_384)),
        8192
    );
    assert_eq!(derive_measured_output_tokens_reserve(&[], None), 0);
}

#[test]
fn test_derive_measured_output_tokens_reserve_prefers_latest_external_cycle_max() {
    let messages = vec![
        TestMessageBuilder::new("m0", "user")
            .text("older request")
            .source(MessageSource::Ui)
            .build(),
        TestMessageBuilder::new("m1", "assistant")
            .text("older answer")
            .completion_tokens(4096)
            .build(),
        TestMessageBuilder::new("m2", "user")
            .text("latest request")
            .source(MessageSource::Ui)
            .build(),
        TestMessageBuilder::new("m3", "assistant")
            .text("main answer in latest cycle")
            .completion_tokens(1800)
            .build(),
        TestMessageBuilder::new("m4", "assistant")
            .text("small follow-up in latest cycle")
            .completion_tokens(120)
            .build(),
    ];

    assert_eq!(
        derive_measured_output_tokens_reserve(&messages, Some(4096)),
        1800
    );
}

#[test]
fn test_tail_recompaction_recovery_plan_targets_latest_request_block_after_incremental_noop() {
    let messages = vec![
        make_message("m0", "assistant", "older 0"),
        make_message("m1", "assistant", "older 1"),
        make_message("m2", "assistant", "older 2"),
        make_message("m3", "assistant", "older 3"),
        make_message("m4", "assistant", "post-summary delta"),
        TestMessageBuilder::new("m5", "user")
            .text("latest user request")
            .source(MessageSource::Ui)
            .build(),
    ];
    let compact_record = CompactContextRecord {
        id: "compact-1".to_string(),
        session_id: TEST_SESSION_ID.to_string(),
        to_id: "m3".to_string(),
        condensed_count: Some(4),
        summary: "Existing summary".to_string(),
        created_at: 123,
    };

    let plan =
        derive_tail_recompaction_recovery_plan_for_testing(&messages, Some(&compact_record), 2)
            .expect("oversize no-op should force tail re-compaction before latest request");

    assert_eq!(plan.compacted_to_idx, 3);
    assert_eq!(plan.first_delta_message_idx, 4);
    assert_eq!(plan.latest_request_start_idx, 5);
    assert_eq!(plan.fallback_split_idx, 5);
}

#[test]
fn test_tail_recompaction_recovery_plan_returns_none_when_latest_request_is_already_only_tail() {
    let messages = vec![
        make_message("m0", "assistant", "older 0"),
        TestMessageBuilder::new("m1", "user")
            .text("latest user request")
            .source(MessageSource::Ui)
            .build(),
    ];
    let compact_record = CompactContextRecord {
        id: "compact-1".to_string(),
        session_id: TEST_SESSION_ID.to_string(),
        to_id: "m0".to_string(),
        condensed_count: Some(1),
        summary: "Existing summary".to_string(),
        created_at: 123,
    };

    let plan =
        derive_tail_recompaction_recovery_plan_for_testing(&messages, Some(&compact_record), 1);

    assert!(plan.is_none());
}

#[test]
fn test_compaction_parent_request_does_not_serialize_internal_session_context() {
    let request = CompactionParentRequest {
        model: "gpt-4o".to_string(),
        provider: "openai".to_string(),
        system_prompt: Some("Stable prompt".to_string()),
        session_context: Some("volatile session context".to_string()),
        available_tools: None,
    };

    let serialized = serde_json::to_value(&request).expect("request should serialize");

    assert_eq!(serialized["model"], "gpt-4o");
    assert_eq!(serialized["provider"], "openai");
    assert_eq!(serialized["systemPrompt"], "Stable prompt");
    assert!(serialized.get("sessionContext").is_none());
}

#[test]
fn test_apply_compaction_retry_budget_progressively_reduces_limit() {
    assert_eq!(apply_compaction_retry_budget(128_000, 0), 128_000);
    assert_eq!(apply_compaction_retry_budget(128_000, 1), 108_800);
    assert_eq!(apply_compaction_retry_budget(128_000, 2), 89_600);
    assert_eq!(apply_compaction_retry_budget(128_000, 3), 70_400);
    assert_eq!(apply_compaction_retry_budget(512, 3), 512);
}

#[test]
fn test_retry_budget_applies_after_output_reserve_budgeting() {
    let safe_input_limit = 128_000;
    let output_reserve = 24_000;

    let effective_budget = calculate_effective_input_budget(safe_input_limit, output_reserve);
    let retry_budget = apply_compaction_retry_budget(effective_budget, 1);

    assert_eq!(effective_budget, 104_000);
    assert_eq!(retry_budget, 88_400);
}

#[test]
fn test_compaction_overflow_recovery_ladder_progresses_from_budget_to_recovery_to_degraded_tools() {
    assert_eq!(
        advance_compaction_overflow_recovery_step_for_testing(
            tauri_mcp_agent_lib::agent::state::CompactionRecoveryPhase::CacheAligned,
            0,
        ),
        Some((
            tauri_mcp_agent_lib::agent::state::CompactionRecoveryPhase::CacheAligned,
            1,
        ))
    );
    assert_eq!(
        advance_compaction_overflow_recovery_step_for_testing(
            tauri_mcp_agent_lib::agent::state::CompactionRecoveryPhase::CacheAligned,
            3,
        ),
        Some((
            tauri_mcp_agent_lib::agent::state::CompactionRecoveryPhase::OverflowRecovery,
            0,
        ))
    );
    assert_eq!(
        advance_compaction_overflow_recovery_step_for_testing(
            tauri_mcp_agent_lib::agent::state::CompactionRecoveryPhase::OverflowRecovery,
            0,
        ),
        Some((
            tauri_mcp_agent_lib::agent::state::CompactionRecoveryPhase::DegradedTools,
            0,
        ))
    );
    assert_eq!(
        advance_compaction_overflow_recovery_step_for_testing(
            tauri_mcp_agent_lib::agent::state::CompactionRecoveryPhase::DegradedTools,
            0,
        ),
        None
    );
}

#[test]
fn test_overflow_recovery_preserves_latest_real_ui_request_summary_and_full_surviving_body() {
    let summary = make_compact_summary_message("m0", "assistant", "previous summary");
    let mut synthetic = make_message("m1", "user", "synthetic session context");
    synthetic.source = Some(MessageSource::SessionContext);
    let older = make_message("m2", "assistant", "older assistant context");
    let mut latest_request = make_message("m3", "user", "latest real request from ui");
    latest_request.source = Some(MessageSource::Ui);
    let fresh_assistant = make_message("m4", "assistant", "fresh assistant context");
    let instruction = make_compaction_instruction_message("m5", "compact this safely");

    let selected = build_overflow_recovery_compaction_messages(
        &[
            summary.clone(),
            synthetic,
            older,
            latest_request.clone(),
            fresh_assistant.clone(),
            instruction.clone(),
        ],
        "openai",
        10_000,
        0,
        0,
    )
    .expect("overflow recovery payload should fit");

    let selected_ids = selected
        .iter()
        .map(|message| message.id.as_str())
        .collect::<Vec<_>>();
    assert!(selected_ids.contains(&summary.id.as_str()));
    assert!(selected_ids.contains(&latest_request.id.as_str()));
    assert!(selected_ids.contains(&fresh_assistant.id.as_str()));
    assert!(selected_ids.contains(&instruction.id.as_str()));
    assert!(!selected_ids.contains(&"m1"));
}

#[test]
fn test_overflow_recovery_filters_scaffolding_and_preserves_tail_instruction_shape() {
    let summary = make_compact_summary_message("m0", "assistant", "previous summary");

    let mut session_context = make_message("m1", "user", "volatile session context");
    session_context.source = Some(MessageSource::SessionContext);

    let old_instruction = make_compaction_instruction_message("m2", "stale compaction overlay");

    let mut latest_request = make_message("m3", "user", "latest real request from ui");
    latest_request.source = Some(MessageSource::Ui);

    let fresh_assistant = make_message("m4", "assistant", "fresh assistant context");
    let tail_instruction = make_compaction_instruction_message("m5", "current compaction overlay");

    let selected = build_overflow_recovery_compaction_messages(
        &[
            summary.clone(),
            session_context,
            old_instruction,
            latest_request.clone(),
            fresh_assistant.clone(),
            tail_instruction.clone(),
        ],
        "openai",
        10_000,
        0,
        0,
    )
    .expect("overflow recovery payload should preserve compaction shape");

    let selected_ids = selected
        .iter()
        .map(|message| message.id.as_str())
        .collect::<Vec<_>>();

    assert!(selected_ids.contains(&summary.id.as_str()));
    assert!(selected_ids.contains(&latest_request.id.as_str()));
    assert!(selected_ids.contains(&fresh_assistant.id.as_str()));
    assert!(!selected_ids.contains(&"m1"));
    assert!(!selected_ids.contains(&"m2"));
    assert_eq!(
        selected.last().map(|message| message.id.as_str()),
        Some("m5")
    );
}

#[test]
fn test_compaction_payload_diagnostics_capture_final_input_shape() {
    let summary = make_compact_summary_message("m0", "assistant", "previous summary");

    let mut latest_request = make_message("m1", "user", "latest real request from ui");
    latest_request.source = Some(MessageSource::Ui);

    let fresh_assistant = make_message("m2", "assistant", "fresh assistant context");
    let tail_instruction = make_compaction_instruction_message("m3", "current compaction overlay");

    let selected = build_overflow_recovery_compaction_messages(
        &[
            summary.clone(),
            latest_request.clone(),
            fresh_assistant.clone(),
            tail_instruction.clone(),
        ],
        "openai",
        10_000,
        0,
        0,
    )
    .expect("overflow recovery payload should fit");

    let diagnostics = inspect_compaction_payload(&selected);

    assert_eq!(diagnostics.total_messages, 4);
    assert_eq!(diagnostics.body_message_count, 3);
    assert_eq!(diagnostics.raw_delta_message_count, 2);
    assert_eq!(diagnostics.compact_summary_count, 1);
    assert_eq!(diagnostics.compaction_instruction_count, 1);
    assert_eq!(diagnostics.scaffolding_count, 0);
    assert_eq!(diagnostics.external_request_count, 1);
    assert_eq!(diagnostics.latest_external_request_message_ids, vec!["m1"]);

    let latest_request_entry = diagnostics
        .messages
        .iter()
        .find(|message| message.id == "m1")
        .expect("latest request entry should exist");
    assert_eq!(latest_request_entry.source, "ui");
    assert!(latest_request_entry
        .flags
        .contains(&"external_request".to_string()));

    let instruction_entry = diagnostics
        .messages
        .iter()
        .find(|message| message.id == "m3")
        .expect("instruction entry should exist");
    assert!(instruction_entry
        .flags
        .contains(&"compaction_instruction".to_string()));
}

#[test]
fn test_overflow_recovery_reduces_active_body_with_fifo_semantics() {
    let summary = make_compact_summary_message("m0", "assistant", &"summary context ".repeat(600));
    let older = make_message("m1", "assistant", &"older assistant context ".repeat(300));
    let mut latest_request = make_message("m2", "user", "latest real request from ui");
    latest_request.source = Some(MessageSource::Ui);
    let fresh_assistant = make_message("m3", "assistant", "fresh assistant context");
    let tail_instruction = make_compaction_instruction_message("m4", "current compaction overlay");

    let limit = calculate_conservative_preflight_prompt_tokens(
        &[
            summary.clone(),
            latest_request.clone(),
            fresh_assistant.clone(),
            tail_instruction.clone(),
        ],
        10,
        5,
        None,
    ) + 1;

    let selected = build_overflow_recovery_compaction_messages(
        &[
            summary.clone(),
            older,
            latest_request.clone(),
            fresh_assistant.clone(),
            tail_instruction.clone(),
        ],
        "openai",
        limit,
        10,
        5,
    )
    .expect("overflow recovery should keep the latest request and freshest active suffix");

    let selected_ids = selected
        .iter()
        .map(|message| message.id.as_str())
        .collect::<Vec<_>>();
    assert!(selected_ids.contains(&summary.id.as_str()));
    assert!(selected_ids.contains(&latest_request.id.as_str()));
    assert!(selected_ids.contains(&fresh_assistant.id.as_str()));
    assert!(selected_ids.contains(&tail_instruction.id.as_str()));
    assert!(!selected_ids.contains(&"m1"));
}

#[test]
fn test_overflow_recovery_allows_no_external_user_anchor_when_body_fits() {
    let summary = make_compact_summary_message("m0", "assistant", "previous summary");
    let mut assistant = make_message("m1", "assistant", "assistant-only continuation");
    assistant.tool_calls = Some(vec![AgentToolCall {
        id: "call-1".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "read_file".to_string(),
            arguments: "{}".to_string(),
        },
    }]);
    let mut tool = make_message("m2", "tool", "tool output");
    tool.tool_call_id = Some("call-1".to_string());
    let tail_instruction = make_compaction_instruction_message("m3", "current compaction overlay");

    let selected = build_overflow_recovery_compaction_messages(
        &[
            summary.clone(),
            assistant.clone(),
            tool.clone(),
            tail_instruction.clone(),
        ],
        "openai",
        10_000,
        0,
        0,
    )
    .expect("overflow recovery should support sessions without external user anchor");

    let selected_ids = selected
        .iter()
        .map(|message| message.id.as_str())
        .collect::<Vec<_>>();
    assert!(selected_ids.contains(&summary.id.as_str()));
    assert!(selected_ids.contains(&assistant.id.as_str()));
    assert!(selected_ids.contains(&tool.id.as_str()));
    assert_eq!(
        selected.last().map(|message| message.id.as_str()),
        Some(tail_instruction.id.as_str())
    );
}

#[test]
fn test_overflow_recovery_uses_compact_summary_active_request_as_workflow_anchor() {
    let summary = make_compact_summary_message(
        "m0",
        "assistant",
        "### Stable Context\n- Existing project context\n\n### Active Request\n- Fix SDL2 compatibility in doom-engine build\n\n### Next Actions\n- Rebuild and verify runtime",
    );
    let mut assistant = make_message("m1", "assistant", "assistant-only continuation");
    assistant.tool_calls = Some(vec![AgentToolCall {
        id: "call-1".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "read_file".to_string(),
            arguments: "{}".to_string(),
        },
    }]);
    let mut tool = make_message("m2", "tool", "tool output");
    tool.tool_call_id = Some("call-1".to_string());
    let tail_instruction = make_compaction_instruction_message("m3", "current compaction overlay");

    let selected = build_overflow_recovery_compaction_messages(
        &[
            summary.clone(),
            assistant.clone(),
            tool.clone(),
            tail_instruction.clone(),
        ],
        "openai",
        10_000,
        0,
        0,
    )
    .expect("overflow recovery should use compact summary Active Request as workflow anchor");

    let selected_ids = selected
        .iter()
        .map(|message| message.id.as_str())
        .collect::<Vec<_>>();
    assert!(selected_ids.contains(&summary.id.as_str()));
    assert!(selected_ids.contains(&assistant.id.as_str()));
    assert!(selected_ids.contains(&tool.id.as_str()));
    assert_eq!(
        selected.last().map(|message| message.id.as_str()),
        Some(tail_instruction.id.as_str())
    );
}

#[test]
fn test_overflow_recovery_accepts_plain_active_request_heading_as_workflow_anchor() {
    let summary = make_compact_summary_message(
        "m0",
        "assistant",
        "Stable Context:\n- Existing project context\n\nActive Request:\n- Fix SDL2 compatibility in doom-engine build\n\nNext Actions:\n- Rebuild and verify runtime",
    );
    let mut assistant = make_message("m1", "assistant", "assistant-only continuation");
    assistant.tool_calls = Some(vec![AgentToolCall {
        id: "call-1".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "read_file".to_string(),
            arguments: "{}".to_string(),
        },
    }]);
    let mut tool = make_message("m2", "tool", "tool output");
    tool.tool_call_id = Some("call-1".to_string());
    let tail_instruction = make_compaction_instruction_message("m3", "current compaction overlay");

    let selected = build_overflow_recovery_compaction_messages(
        &[
            summary.clone(),
            assistant.clone(),
            tool.clone(),
            tail_instruction.clone(),
        ],
        "openai",
        10_000,
        0,
        0,
    )
    .expect("overflow recovery should accept flexible Active Request headings");

    let selected_ids = selected
        .iter()
        .map(|message| message.id.as_str())
        .collect::<Vec<_>>();
    assert!(selected_ids.contains(&summary.id.as_str()));
    assert!(selected_ids.contains(&assistant.id.as_str()));
    assert!(selected_ids.contains(&tool.id.as_str()));
    assert_eq!(
        selected.last().map(|message| message.id.as_str()),
        Some(tail_instruction.id.as_str())
    );
}

#[test]
fn test_internal_synthetic_user_message_uses_compaction_instruction_id_fallback() {
    let synthetic = make_message(
        "compaction-instruction-legacy",
        "user",
        "Synthetic compaction prompt",
    );

    assert!(synthetic.is_internal_synthetic_user_message());
    assert!(!synthetic.is_external_request_message());
}

#[test]
fn test_build_compaction_preservation_hints_capture_active_request_and_references() {
    let mut read_file = make_message("m0", "assistant", "Reading types file");
    read_file.tool_calls = Some(vec![AgentToolCall {
        id: "call_A".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "read_file".to_string(),
            arguments: json!({
                "path": "src/lib/types.ts"
            })
            .to_string(),
        },
    }]);

    let mut tool_result = make_message(
        "m1",
        "tool",
        "interface InterfaceA { id: string }\ninterface InterfaceB { name: string }",
    );
    tool_result.tool_call_id = Some("call_A".to_string());

    let request = make_message(
        "m2",
        "user",
        "Rename `InterfaceB` to `InterfaceC` after the `read_file` result.",
    );

    let hints = build_compaction_preservation_hints(&[read_file, tool_result, request]);
    assert_eq!(hints.active_request.len(), 1);
    assert!(hints.active_request[0].contains("InterfaceB"));
    assert!(hints.active_request[0].contains("InterfaceC"));
    assert!(
        hints
            .required_references
            .iter()
            .any(|hint| hint.contains("src/lib/types.ts")),
        "required references should keep the referenced file path"
    );
    assert!(
        hints
            .required_references
            .iter()
            .any(|hint| hint.contains("InterfaceB")),
        "required references should keep the referenced existing symbol"
    );
    assert!(
        hints
            .required_references
            .iter()
            .any(|hint| hint.contains("InterfaceC")),
        "required references should keep the requested target symbol"
    );
}

#[test]
fn test_build_compaction_preservation_hints_capture_consecutive_external_request_block() {
    let mut first_request = make_message("m0", "user", "Refactor the compaction contract.");
    first_request.source = Some(MessageSource::Ui);
    let mut second_request = make_message(
        "m1",
        "user",
        "Specifically update `docs/specs/message-compaction.md` and keep `Active Request` semantic.",
    );
    second_request.source = Some(MessageSource::Ui);

    let hints = build_compaction_preservation_hints(&[first_request, second_request]);

    assert!(
        hints
            .active_request
            .iter()
            .any(|hint| hint.contains("Refactor the compaction contract")),
        "the first message in a contiguous external request block should remain in the distillation seed"
    );
    assert!(
        hints
            .active_request
            .iter()
            .any(|hint| hint.contains("Active Request")),
        "the latest message in the contiguous external request block should also remain in the distillation seed"
    );
    assert!(
        hints
            .required_references
            .iter()
            .any(|hint| hint.contains("docs/specs/message-compaction.md")),
        "required references should still be extracted from the contiguous request block"
    );
}

#[test]
fn test_build_compaction_preservation_hints_ignore_synthetic_user_messages() {
    let mut synthetic = make_message("compaction-instruction-legacy", "user", "Synthetic message");
    synthetic.source = Some(MessageSource::CompactionInstruction);

    let hints = build_compaction_preservation_hints(&[synthetic]);
    assert!(hints.active_request.is_empty());
    assert!(hints.required_references.is_empty());
}

#[test]
fn test_build_compaction_preservation_hints_carry_forward_prior_summary_open_requests() {
    let prior_summary = "\
### Stable Context
- Existing work item

### Key Decisions & Constraints
- Keep refactor incremental

### Active Request
- Refactor `src/lib/file-b.ts`

### Required References
- Preserve file path `src/lib/file-b.ts`
- Preserve identifier `InterfaceB`

### Current State
- Refactor still pending

### Recent Tool Results
- read_file(path=src/lib/file-b.ts) -> success

### Next Actions
- Update references";
    let summary_message = make_compact_summary_message(
        "m0",
        "assistant",
        &build_compact_summary_text(prior_summary, &[]),
    );
    let request = make_message("m1", "user", "Also update `src/lib/file-c.ts`.");

    let hints = build_compaction_preservation_hints(&[summary_message, request]);

    assert!(
        hints
            .active_request
            .iter()
            .any(|hint| hint.contains("src/lib/file-b.ts")),
        "prior unresolved request should be carried forward from the previous summary"
    );
    assert!(
        hints
            .active_request
            .iter()
            .any(|hint| hint.contains("src/lib/file-c.ts")),
        "new request should still be included"
    );
    assert!(
        hints
            .required_references
            .iter()
            .any(|hint| hint.contains("InterfaceB")),
        "prior required references should be carried forward from the previous summary"
    );
}

#[test]
fn test_build_compaction_preservation_hints_prioritize_live_external_request_over_full_summary_limit(
) {
    let prior_summary = "\
### Stable Context
- Existing work item

### Active Request
- Preserve `src/lib/file-a.ts`
- Preserve `src/lib/file-b.ts`
- Preserve `src/lib/file-c.ts`
- Preserve `src/lib/file-d.ts`

### Required References
- Preserve file path `src/lib/file-a.ts`

### Current State
- Refactor still pending";
    let summary_message = make_compact_summary_message(
        "m0",
        "assistant",
        &build_compact_summary_text(prior_summary, &[]),
    );
    let request = make_message(
        "m1",
        "user",
        "Also update `src/lib/live-file.ts` immediately.",
    );

    let hints = build_compaction_preservation_hints(&[summary_message, request]);

    assert_eq!(
        hints.active_request.len(),
        4,
        "active request hints should still obey the fixed bullet budget"
    );
    assert!(
        hints
            .active_request
            .iter()
            .any(|hint| hint.contains("src/lib/live-file.ts")),
        "the latest live external request must survive even when prior summary active-request bullets already fill the limit"
    );
}

#[test]
fn test_build_compaction_preservation_hints_preserves_colon_terminated_request_bullets() {
    let prior_summary = "\
### Stable Context
- Existing work item

### Active Request
- Next step:
- Refactor `src/lib/file-b.ts`

### Required References
- Preserve file path `src/lib/file-b.ts`
";
    let summary_message = make_compact_summary_message(
        "m0",
        "assistant",
        &build_compact_summary_text(prior_summary, &[]),
    );

    let hints = build_compaction_preservation_hints(&[summary_message]);

    assert!(
        hints.active_request.iter().any(|hint| hint == "Next step:"),
        "colon-terminated request bullets should remain part of the active request"
    );
}

#[test]
fn test_build_compaction_preservation_hints_accept_plain_section_headings() {
    let prior_summary = "\
Stable Context:
- Existing work item

Active Request:
- Refactor `src/lib/file-b.ts`

Required References:
- Preserve file path `src/lib/file-b.ts`
";
    let summary_message = make_compact_summary_message(
        "m0",
        "assistant",
        &build_compact_summary_text(prior_summary, &[]),
    );

    let hints = build_compaction_preservation_hints(&[summary_message]);

    assert!(
        hints
            .active_request
            .iter()
            .any(|hint| hint.contains("src/lib/file-b.ts")),
        "plain section headings should still feed Active Request hints"
    );
    assert!(
        hints
            .required_references
            .iter()
            .any(|hint| hint.contains("src/lib/file-b.ts")),
        "plain section headings should still feed Required References hints"
    );
    assert!(
        hints
            .active_request
            .iter()
            .any(|hint| hint.contains("src/lib/file-b.ts")),
        "subsequent bullets in the same section should still be collected"
    );
}

#[test]
fn test_build_compaction_preservation_hints_filter_non_identifier_backticks_and_non_paths() {
    let request = make_message(
        "m0",
        "user",
        "Run `pnpm refactor:validate`, keep version 18.3 noted, and rename `ActualSymbol` in `src/lib/types.ts`.",
    );

    let hints = build_compaction_preservation_hints(&[request]);

    assert!(
        hints
            .required_references
            .iter()
            .any(|hint| hint.contains("ActualSymbol")),
        "real identifier references should be preserved"
    );
    assert!(
        hints
            .required_references
            .iter()
            .any(|hint| hint.contains("src/lib/types.ts")),
        "real file paths should be preserved"
    );
    assert!(
        !hints
            .required_references
            .iter()
            .any(|hint| hint.contains("pnpm refactor:validate")),
        "command-like backtick spans should not consume required reference slots"
    );
    assert!(
        !hints
            .required_references
            .iter()
            .any(|hint| hint.contains("18.3")),
        "bare dotted values should not be treated as file paths"
    );
}

#[test]
fn test_build_compaction_preservation_hints_strip_harness_meta_wrappers() {
    let request = make_message(
        "m0",
        "user",
        "<current_datetime>2026-05-26T19:38:10.415+09:00</current_datetime>\n\nFix spawnPickups in `doom-app/src/game.cpp`.\n\n<system_reminder>\nConsider updating plan.md to reflect current progress and next steps.\n</system_reminder>\n<system_reminder>\n<sql_tables>Available tables: todos, todo_deps, inbox_entries</sql_tables>\n</system_reminder>",
    );

    let hints = build_compaction_preservation_hints(&[request]);

    assert!(
        hints
            .active_request
            .iter()
            .any(|hint| hint.contains("Fix spawnPickups in `doom-app/src/game.cpp`.")),
        "semantic user request should survive wrapper stripping"
    );
    assert!(
        !hints
            .active_request
            .iter()
            .any(|hint| hint.contains("current_datetime") || hint.contains("system_reminder")),
        "harness metadata wrappers must not leak into active request hints"
    );
}

#[test]
fn test_sanitize_compaction_semantic_text_strips_multiline_wrapper_blocks() {
    let sanitized = sanitize_compaction_semantic_text(
        "Keep this request.\n<current_datetime>\n2026-05-26T20:05:13.256+09:00\n</current_datetime>\n<system_reminder>\n<sql_tables>Available tables: todos, todo_deps, inbox_entries</sql_tables>\n</system_reminder>\nStill keep this.",
    );

    assert_eq!(sanitized, "Keep this request.\nStill keep this.");
}

#[test]
fn test_compaction_artifact_line_detects_known_divider_and_wrapper_lines() {
    assert!(is_compaction_artifact_line(
        "Latest included: doom-app/src/game.cpp"
    ));
    assert!(is_compaction_artifact_line("345 messages condensed"));
    assert!(is_compaction_artifact_line("<system_reminder>"));
    assert!(is_compaction_artifact_line("</system_reminder>"));
    assert!(!is_compaction_artifact_line(
        "Fix spawnPickups in doom-app/src/game.cpp"
    ));
}

#[test]
fn test_build_compaction_preservation_hints_strip_compact_divider_chrome_from_fenced_text() {
    let request = make_message(
        "m0",
        "user",
        "```컨텍스트가 위에서 압축됨\nEarlier: Earlier conversation context\nLatest included: doom-app/src/game.cpp\n345 messages condensed\nSummary\nFix spawnPickups stale end anchor in `doom-app/src/game.cpp`.\n```",
    );

    let hints = build_compaction_preservation_hints(&[request]);

    assert!(
        hints
            .active_request
            .iter()
            .any(|hint| hint.contains("Fix spawnPickups stale end anchor")),
        "semantic request text should remain after compact divider chrome is removed"
    );
    assert!(
        !hints.active_request.iter().any(|hint| {
            hint.contains("컨텍스트가 위에서 압축됨")
                || hint.contains("Earlier:")
                || hint.contains("Latest included:")
                || hint.contains("messages condensed")
        }),
        "compact divider chrome must not leak into active request hints"
    );
}

#[test]
fn test_build_compaction_preservation_hints_strip_multiline_harness_wrappers() {
    let request = make_message(
        "m0",
        "user",
        "Fix spawnPickups in `doom-app/src/game.cpp`.\n<current_datetime>\n2026-05-26T20:06:01.072+09:00\n</current_datetime>\n<system_reminder>\n<sql_tables>Available tables: todos, todo_deps, inbox_entries</sql_tables>\n</system_reminder>",
    );

    let hints = build_compaction_preservation_hints(&[request]);

    assert!(
        hints
            .active_request
            .iter()
            .any(|hint| hint.contains("Fix spawnPickups in `doom-app/src/game.cpp`.")),
        "semantic request should survive multiline wrapper stripping"
    );
    assert!(
        !hints.active_request.iter().any(|hint| {
            hint.contains("current_datetime")
                || hint.contains("system_reminder")
                || hint.contains("sql_tables")
                || hint.contains("2026-05-26")
        }),
        "multiline harness wrapper content must not leak into active request hints"
    );
}

#[test]
fn test_preflight_compaction_split_after_removing_incomplete_tool_chains() {
    let intro = make_message("m0", "user", "Need analysis");

    let mut stale_assistant = make_message("m1", "assistant", "Old stale tool call");
    stale_assistant.tool_calls = Some(vec![AgentToolCall {
        id: "stale_call".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "toolA".to_string(),
            arguments: "{}".to_string(),
        },
    }]);

    let mut current_assistant = make_message("m2", "assistant", "Current resolved tool call");
    current_assistant.tool_calls = Some(vec![AgentToolCall {
        id: "current_call".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "toolB".to_string(),
            arguments: "{}".to_string(),
        },
    }]);

    let mut current_tool = make_message("m3", "tool", &"very large tool result ".repeat(200));
    current_tool.tool_call_id = Some("current_call".to_string());

    let cleaned = remove_incomplete_tool_chains(vec![
        intro,
        stale_assistant,
        current_assistant,
        current_tool,
    ]);

    let preview = preview_preflight_compaction_selection(&cleaned);
    assert_eq!(preview.compacted_ids, vec!["m0", "m1", "m2", "m3"]);
    assert!(preview.preserved_ids.is_empty());
}

#[test]
fn test_normalize_request_messages_removes_stale_incomplete_tool_chains_before_compaction() {
    let intro = make_message("m0", "user", "Need analysis");

    let mut stale_assistant = make_message("m1", "assistant", "Old stale tool call");
    stale_assistant.tool_calls = Some(vec![AgentToolCall {
        id: "stale_call".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "toolA".to_string(),
            arguments: "{}".to_string(),
        },
    }]);

    let mut current_assistant = make_message("m2", "assistant", "Current resolved tool call");
    current_assistant.tool_calls = Some(vec![AgentToolCall {
        id: "current_call".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "toolB".to_string(),
            arguments: "{}".to_string(),
        },
    }]);

    let mut current_tool = make_message("m3", "tool", &"very large tool result ".repeat(200));
    current_tool.tool_call_id = Some("current_call".to_string());

    let normalized = normalize_request_messages(vec![
        intro,
        stale_assistant,
        current_assistant,
        current_tool,
    ]);

    assert_eq!(normalized.len(), 4);
    assert_eq!(normalized[0].id, "m0");
    assert_eq!(normalized[1].id, "m1");
    assert!(normalized[1].tool_calls.is_none());
    assert_eq!(normalized[2].id, "m2");
    assert_eq!(normalized[3].id, "m3");
    let preview = preview_preflight_compaction_selection(&normalized);
    assert_eq!(preview.compacted_ids, vec!["m0", "m1", "m2", "m3"]);
    assert!(preview.preserved_ids.is_empty());
}

#[test]
fn test_same_tail_compaction_allows_retry_when_no_compact_summary_exists() {
    let messages = vec![
        make_message("m0", "assistant", "Earlier context"),
        make_message("m1", "user", "Latest request"),
    ];

    assert!(!should_skip_same_tail_compaction(&messages, None, 1));
}

#[test]
fn test_same_tail_compaction_allows_follow_up_compaction_after_summary_injection() {
    let summary =
        make_compact_summary_message("compact-summary-test", "assistant", "Compacted summary");

    let messages = vec![
        summary,
        make_message("m1", "assistant", "Large preserved context"),
        make_message("m2", "user", "Latest request"),
    ];

    assert!(!should_skip_same_tail_compaction(&messages, None, 2));
}

#[test]
fn test_same_tail_compaction_does_not_skip_latest_request_after_summary() {
    let summary =
        make_compact_summary_message("compact-summary-test", "assistant", "Compacted summary");

    let messages = vec![summary, make_message("m1", "user", "Latest request")];

    assert!(!should_skip_same_tail_compaction(&messages, None, 2));
}

#[test]
fn test_remove_incomplete_tool_chains() {
    let mut msg1 = make_message("m1", "assistant", "I will call tools");
    msg1.tool_calls = Some(vec![
        AgentToolCall {
            id: "call_A".to_string(),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: "toolA".to_string(),
                arguments: "{}".to_string(),
            },
        },
        AgentToolCall {
            id: "call_B".to_string(),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: "toolB".to_string(),
                arguments: "{}".to_string(),
            },
        },
    ]);

    let mut msg2 = make_message("m2", "tool", "result A");
    msg2.tool_call_id = Some("call_A".to_string());

    let cleaned = remove_incomplete_tool_chains(vec![msg1, msg2]);
    assert_eq!(cleaned.len(), 2);

    let cleaned_assistant = &cleaned[0];
    assert_eq!(cleaned_assistant.tool_calls.as_ref().unwrap().len(), 1);
    assert_eq!(
        cleaned_assistant.tool_calls.as_ref().unwrap()[0].id,
        "call_A"
    );
}

#[test]
fn test_remove_incomplete_tool_chains_preserves_stable_prefix_before_unstable_suffix() {
    let mut stable_assistant = make_message("m1", "assistant", "Completed tools");
    stable_assistant.tool_calls = Some(vec![AgentToolCall {
        id: "call_A".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "toolA".to_string(),
            arguments: "{}".to_string(),
        },
    }]);

    let mut stable_tool = make_message("m2", "tool", "result A");
    stable_tool.tool_call_id = Some("call_A".to_string());

    let mut unstable_assistant = make_message("m3", "assistant", "Pending tools");
    unstable_assistant.tool_calls = Some(vec![AgentToolCall {
        id: "call_B".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "toolB".to_string(),
            arguments: "{}".to_string(),
        },
    }]);

    let cleaned = remove_incomplete_tool_chains(vec![
        stable_assistant.clone(),
        stable_tool.clone(),
        unstable_assistant,
    ]);

    assert_eq!(cleaned.len(), 3);
    assert_eq!(
        cleaned[0].tool_calls.as_ref().map(|calls| calls.len()),
        Some(1)
    );
    assert_eq!(cleaned[1].tool_call_id.as_deref(), Some("call_A"));
    assert!(cleaned[2].tool_calls.is_none());
}

#[test]
fn test_remove_incomplete_tool_chains_drops_orphan_tool_from_unstable_suffix_tail() {
    let mut stable_assistant = make_message("m1", "assistant", "Completed tools");
    stable_assistant.tool_calls = Some(vec![AgentToolCall {
        id: "call_A".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "toolA".to_string(),
            arguments: "{}".to_string(),
        },
    }]);

    let mut stable_tool = make_message("m2", "tool", "result A");
    stable_tool.tool_call_id = Some("call_A".to_string());

    let mut orphan_tool = make_message("m3", "tool", "orphan result");
    orphan_tool.tool_call_id = Some("missing_call".to_string());

    let cleaned = remove_incomplete_tool_chains(vec![
        stable_assistant.clone(),
        stable_tool.clone(),
        orphan_tool,
    ]);

    assert_eq!(cleaned.len(), 2);
    assert_eq!(cleaned[0].id, stable_assistant.id);
    assert_eq!(cleaned[1].id, stable_tool.id);
}

#[test]
fn test_merge_consecutive_user_messages_preserves_first_id_and_appends_content() {
    let earlier_user = make_message("m1", "user", "Earlier user");
    let middle_user = make_message("m2", "user", "Should stay separate");
    let assistant = make_message("m3", "assistant", "Assistant reply");
    let trailing_user_a = make_message("m4", "user", "Latest user A");
    let trailing_user_b = make_message("m5", "user", "Latest user B");

    let merged = merge_consecutive_user_messages(vec![
        earlier_user,
        middle_user,
        assistant,
        trailing_user_a,
        trailing_user_b,
    ]);

    assert_eq!(merged.len(), 4);
    assert_eq!(merged[0].id, "m1");
    assert_eq!(merged[1].id, "m2");
    assert_eq!(merged[2].id, "m3");
    assert_eq!(merged[3].id, "m4");
    assert_eq!(
        text_content_parts(&merged[3]),
        vec!["Latest user A", "\n\n---\n\n", "Latest user B"]
    );
}

#[test]
fn test_merge_consecutive_user_messages_preserves_non_trailing_sequence_boundaries() {
    let user_a = make_message("m1", "user", "User A");
    let user_b = make_message("m2", "user", "User B");
    let assistant = make_message("m3", "assistant", "Assistant");

    let merged = merge_consecutive_user_messages(vec![user_a, user_b, assistant]);

    assert_eq!(merged.len(), 3);
    assert_eq!(merged[0].id, "m1");
    assert_eq!(merged[1].id, "m2");
    assert_eq!(merged[2].id, "m3");
}

#[test]
fn test_batch_tool_calls_in_messages() {
    let mut msg = make_message("m1", "assistant", "Many tools");
    let mut tool_calls = vec![];
    for i in 0..5 {
        tool_calls.push(AgentToolCall {
            id: format!("call_{}", i),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: "tool".to_string(),
                arguments: "{}".to_string(),
            },
        });
    }
    msg.tool_calls = Some(tool_calls);

    let msgs = vec![msg];
    let batched = batch_tool_calls_in_messages(&msgs, 3);

    assert_eq!(batched.len(), 2);
    assert_eq!(batched[0].id, "m1_batch_0");
    assert_eq!(batched[1].id, "m1_batch_1");
}

#[test]
fn test_select_messages_within_context() {
    let mut msgs = vec![];
    for i in 0..5 {
        let role = if i % 2 == 0 { "user" } else { "assistant" };
        msgs.push(make_message(&format!("msg{}", i), role, "Message content"));
    }

    let selected = select_messages_within_context(&msgs, "openai", Some(5000), None, None);
    assert_eq!(selected.len(), 5);
    assert_eq!(selected[0].id, "msg0");
    assert_eq!(selected[4].id, "msg4");
}

#[test]
fn test_select_messages_drops_oversized_single_message_across_providers() {
    let msgs = vec![make_message(
        "big_msg",
        "user",
        &"Very long content ".repeat(100),
    )];

    for provider in ["gemini", "openai", "anthropic"] {
        let selected = select_messages_within_context(&msgs, provider, Some(10), None, None);
        assert!(
            selected.is_empty(),
            "oversized single-message payloads should be dropped for provider {provider}"
        );
    }
}

#[test]
fn test_select_messages_removes_orphaned_tool_tail_without_truncation() {
    let summary = make_compact_summary_message("compact-summary-1", "assistant", "Summary");

    let mut orphan_tool = make_message("tool-1", "tool", "orphan result");
    orphan_tool.tool_call_id = Some("missing_call".to_string());

    let mut paired_assistant = make_message("assistant-1", "assistant", "paired call");
    paired_assistant.tool_calls = Some(vec![AgentToolCall {
        id: "paired_call".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "tool".to_string(),
            arguments: "{}".to_string(),
        },
    }]);

    let mut paired_tool = make_message("tool-2", "tool", "paired result");
    paired_tool.tool_call_id = Some("paired_call".to_string());

    let selected = select_messages_within_context(
        &[
            summary,
            orphan_tool,
            paired_assistant.clone(),
            paired_tool.clone(),
        ],
        "gemini",
        Some(5000),
        None,
        None,
    );

    assert_eq!(selected.len(), 3);
    assert_eq!(selected[0].id, "compact-summary-1");
    assert_eq!(selected[1].id, "assistant-1");
    assert_eq!(selected[2].id, "tool-2");
}

#[test]
fn test_select_messages_keeps_single_pinned_user_message_when_it_fits() {
    let msgs = vec![make_message("msg0", "user", "hello")];

    let selected = select_messages_within_context(&msgs, "gemini", Some(1000), None, None);
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].id, "msg0");
}

#[test]
fn test_select_messages_can_disable_first_user_pinning() {
    let msgs = vec![
        make_message("msg0", "user", "oldest user context"),
        make_message("msg1", "assistant", "assistant reply"),
        make_message("msg2", "user", "latest user context"),
    ];

    let options = SelectionOptions {
        max_messages: Some(2),
        pin_first_user_message: false,
        ..SelectionOptions::default()
    };

    let selected =
        select_messages_within_context(&msgs, "gemini", Some(5000), Some(&options), None);
    assert_eq!(selected.len(), 2);
    assert_eq!(selected[0].id, "msg1");
    assert_eq!(selected[1].id, "msg2");
}

#[test]
fn test_select_messages_without_pinning_does_not_merge_first_and_latest_user_messages() {
    let msgs = vec![
        make_message("msg0", "user", "initial user context"),
        make_message("msg1", "assistant", "assistant reply"),
        make_message("msg2", "user", "latest user context"),
    ];

    let options = SelectionOptions {
        max_messages: Some(1),
        pin_first_user_message: false,
        ..SelectionOptions::default()
    };

    let selected =
        select_messages_within_context(&msgs, "gemini", Some(5000), Some(&options), None);
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].id, "msg2");
}

#[test]
fn test_compact_mode_selection_options_disable_first_user_pinning() {
    let options = build_compact_context_selection_options(
        Some("system".to_string()),
        Some("tools".to_string()),
        "openai",
        7,
        Some(0.63),
    );

    assert_eq!(options.system_prompt.as_deref(), Some("system"));
    assert_eq!(options.tools_json.as_deref(), Some("tools"));
    assert_eq!(options.max_messages, None);
    assert_eq!(options.max_tool_calls_per_message, Some(7));
    assert!(!options.pin_first_user_message);
    assert_eq!(options.fallback_calibration_ratio, Some(0.63));
}

#[test]
fn test_compact_mode_selection_options_keep_gemini_tool_visibility_contract() {
    let options = build_compact_context_selection_options(None, None, "gemini", 7, None);

    assert_eq!(options.max_tool_calls_per_message, Some(100));
    assert!(!options.pin_first_user_message);
}

#[test]
fn test_select_recent_messages_fifo_keeps_last_n_messages() {
    let msgs = vec![
        make_message("msg0", "user", "zero"),
        make_message("msg1", "assistant", "one"),
        make_message("msg2", "user", "two"),
        make_message("msg3", "assistant", "three"),
    ];

    let selected = select_recent_messages_fifo(&msgs, "gemini", 2, 100);
    assert_eq!(selected.len(), 2);
    assert_eq!(selected[0].id, "msg2");
    assert_eq!(selected[1].id, "msg3");
}

#[test]
fn test_select_recent_messages_fifo_falls_back_to_latest_non_tool_message() {
    let mut assistant = make_message("assistant-1", "assistant", "tool call");
    assistant.tool_calls = Some(vec![AgentToolCall {
        id: "call_1".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "tool".to_string(),
            arguments: "{}".to_string(),
        },
    }]);

    let mut tool = make_message("tool-1", "tool", "tool result");
    tool.tool_call_id = Some("call_1".to_string());

    let selected = select_recent_messages_fifo(&[assistant.clone(), tool], "gemini", 1, 100);
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].id, assistant.id);
}

#[test]
fn test_estimate_text_tokens_grows_with_input_size() {
    let short = estimate_text_tokens("Hello");
    let long = estimate_text_tokens(&"Hello world ".repeat(20));

    assert!(short > 0);
    assert!(long > short);
}

#[test]
fn test_uses_compaction_strategy_only_for_compact_mode() {
    assert!(uses_compaction_strategy("compact"));
    assert!(!uses_compaction_strategy("window"));
}

#[test]
fn test_resolve_context_management_settings_prefers_direct_keys_over_legacy_blob() {
    let legacy = json!({
        "contextStrategy": "compact",
        "windowSize": 20,
        "maxInputContext": 49152,
        "toolCallGroupVisibleCount": 4
    });
    let mut direct = HashMap::new();
    direct.insert("maxInputContext".to_string(), json!(131072));
    direct.insert("toolCallGroupVisibleCount".to_string(), json!(8));

    let settings = resolve_context_management_settings(Some(&legacy), &direct);

    assert_eq!(settings.context_strategy(), "compact");
    assert_eq!(settings.window_size(), 20);
    assert_eq!(settings.max_input_context(), 131072);
    assert_eq!(settings.tool_call_group_visible_count(), 8);
}

#[test]
fn test_calculate_context_safety_margin() {
    assert_eq!(calculate_context_safety_margin(10_000), 1024);
    assert_eq!(calculate_context_safety_margin(100_000), 5000);
    assert_eq!(calculate_context_safety_margin(500_000), 8192);
}

#[test]
fn test_estimate_tokens_bpe_basic_text() {
    let msg = make_message_simple("user", "Hello assistant");
    let tokens = estimate_tokens_bpe(&msg);
    assert!(tokens > 1);
}

#[test]
fn test_estimate_tokens_bpe_with_tools() {
    let mut msg = make_message_simple("assistant", "I will use a tool");
    msg.tool_calls = Some(vec![AgentToolCall {
        id: "call_123".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "get_weather".to_string(),
            arguments: "{\"city\": \"Seoul\"}".to_string(),
        },
    }]);

    let tokens = estimate_tokens_bpe(&msg);
    assert!(tokens > 5);
}

#[test]
fn test_grounded_total_tokens_no_grounding() {
    let messages = vec![
        make_message_simple("user", "Hello"),
        make_message_simple("assistant", "Hi there"),
        make_message_simple("user", "What's up"),
    ];

    let tokens = calculate_grounded_total_tokens(&messages, 10, 5);
    assert!(tokens > 15);
}

#[test]
fn test_grounded_total_tokens_with_grounding() {
    let msg1 = make_message_simple("user", "Hello");
    let mut msg2 = make_message_simple("assistant", "Hi there");
    msg2.usage = Some(json!({ "totalTokens": 100 }));

    let msg3 = make_message_simple("user", "A very long new message");
    let messages = vec![msg1, msg2, msg3.clone()];

    let tokens = calculate_grounded_total_tokens(&messages, 50, 50);
    let expected = 100 + estimate_tokens_bpe(&msg3);
    assert_eq!(tokens, expected);
}

#[test]
fn test_grounded_total_tokens_keeps_summary_aware_anchor_after_compaction() {
    let summary = make_compact_summary_message("compact-summary-123", "assistant", "Summary...");

    let mut grounded = make_message_simple("assistant", "Hi there");
    grounded.usage = Some(json!({ "totalTokens": 100 }));

    let tail = make_message_simple("user", "Hello");

    let messages = vec![summary.clone(), grounded.clone(), tail.clone()];

    let tokens = calculate_grounded_total_tokens(&messages, 10, 5);
    let expected = 100 + estimate_tokens_bpe(&tail);

    assert_eq!(tokens, expected);
}

#[test]
fn test_prompt_anchor_calibration_keeps_summary_aware_anchor_after_compaction() {
    let summary = make_compact_summary_message(
        "compact-summary-123",
        "assistant",
        &"Compacted summary ".repeat(1500),
    );

    let intro = make_message_simple(
        "user",
        &"Existing tail before grounded response ".repeat(1000),
    );
    let grounded = make_message_simple("assistant", "Grounded assistant output");

    let messages = vec![summary.clone(), intro.clone(), grounded];

    let bpe_input = estimate_tokens_bpe(&summary) + estimate_tokens_bpe(&intro) + 10 + 5;
    let expected_ratio = (bpe_input as f64 * 0.9).ceil() / bpe_input as f64;
    let mut messages = messages;
    messages[2].usage = Some(json!({ "promptTokens": (bpe_input as f64 * 0.9).ceil() as usize }));

    let ratio = derive_bpe_calibration_ratio(&messages, 10, 5);
    assert!((ratio - expected_ratio).abs() < 1e-9);
}

#[test]
fn test_prompt_anchored_total_tokens_keeps_summary_aware_anchor_after_compaction() {
    let summary = make_compact_summary_message(
        "compact-summary-123",
        "assistant",
        &"Compacted summary ".repeat(1500),
    );

    let intro = make_message_simple(
        "user",
        &"Existing tail before grounded response ".repeat(1000),
    );
    let mut grounded = make_message_simple("assistant", "Grounded assistant output");
    let tail = make_message_simple("user", "Fresh delta after anchor");

    let mut messages = vec![
        summary.clone(),
        intro.clone(),
        grounded.clone(),
        tail.clone(),
    ];

    let bpe_input = estimate_tokens_bpe(&summary) + estimate_tokens_bpe(&intro) + 10 + 5;
    let prompt_tokens = (bpe_input as f64 * 0.9).ceil() as usize;
    messages[2].usage = Some(json!({ "promptTokens": prompt_tokens }));
    grounded.usage = Some(json!({ "promptTokens": prompt_tokens }));
    let ratio = prompt_tokens as f64 / bpe_input as f64;
    let bpe_output = estimate_tokens_bpe(&grounded) + estimate_tokens_bpe(&tail);
    let expected = prompt_tokens + (bpe_output as f64 * ratio).ceil() as usize;

    let tokens = calculate_prompt_anchored_total_tokens(&messages, 10, 5);
    assert_eq!(tokens, expected);
}

#[test]
fn test_conservative_preflight_prompt_tokens_biases_anchor_delta_upward() {
    let summary = make_compact_summary_message(
        "compact-summary-1",
        "assistant",
        &"Compacted summary ".repeat(1500),
    );
    let intro = make_message("intro", "user", &"Stable tail before anchor ".repeat(1000));
    let mut grounded = make_message("assistant-anchor", "assistant", "Grounded output");
    let tail = make_message("tail", "user", "Fresh delta after anchor");

    let mut messages = vec![
        summary.clone(),
        intro.clone(),
        grounded.clone(),
        tail.clone(),
    ];
    let bpe_input = estimate_tokens_bpe(&summary) + estimate_tokens_bpe(&intro) + 10 + 5;
    let prompt_tokens = (bpe_input as f64 * 0.9).ceil() as usize;
    messages[2].usage = Some(json!({ "promptTokens": prompt_tokens }));
    grounded.usage = Some(json!({ "promptTokens": prompt_tokens }));
    let ratio = prompt_tokens as f64 / bpe_input as f64;
    let delta_bpe = estimate_tokens_bpe(&grounded) + estimate_tokens_bpe(&tail);
    let anchored_total = prompt_tokens + (delta_bpe as f64 * ratio).ceil() as usize;
    let expected = prompt_tokens + (delta_bpe as f64 * ratio * 1.05).ceil() as usize;

    let conservative = calculate_conservative_preflight_prompt_tokens(&messages, 10, 5, None);
    assert_eq!(conservative, expected);
    assert!(conservative >= anchored_total);
}

#[test]
fn test_conservative_preflight_prompt_tokens_fallback_biases_full_estimate_upward() {
    let messages = vec![
        make_message("m1", "user", "Hello"),
        make_message("m2", "assistant", "No grounded usage yet"),
    ];
    let base = messages.iter().map(estimate_tokens_bpe).sum::<usize>() + 10 + 5;
    let conservative = calculate_conservative_preflight_prompt_tokens(&messages, 10, 5, None);

    assert_eq!(conservative, (base as f64 * 1.05).ceil() as usize);
}

#[test]
fn test_conservative_preflight_prompt_tokens_uses_latest_prompt_anchor() {
    let earlier_user = make_message("u1", "user", &"Earlier context ".repeat(1200));
    let older_anchor = make_message("a1", "assistant", "Older grounded output");
    let older_anchor_clone_for_bpe = older_anchor.clone();

    let newer_user = make_message(
        "u2",
        "user",
        &"Newer context that should define the anchor ".repeat(1000),
    );
    let mut latest_anchor = make_message("a2", "assistant", "Latest grounded output");

    let tail = make_message("u3", "user", "Tail delta after latest anchor");
    let mut messages = vec![
        earlier_user.clone(),
        older_anchor,
        newer_user.clone(),
        latest_anchor.clone(),
        tail.clone(),
    ];

    let bpe_input = estimate_tokens_bpe(&earlier_user)
        + estimate_tokens_bpe(&older_anchor_clone_for_bpe)
        + estimate_tokens_bpe(&newer_user)
        + 10
        + 5;
    let prompt_tokens = (bpe_input as f64 * 0.92).ceil() as usize;
    messages[3].usage = Some(json!({ "promptTokens": prompt_tokens }));
    latest_anchor.usage = Some(json!({ "promptTokens": prompt_tokens }));
    let ratio = prompt_tokens as f64 / bpe_input as f64;
    let delta_bpe = estimate_tokens_bpe(&latest_anchor) + estimate_tokens_bpe(&tail);
    let expected = prompt_tokens + (delta_bpe as f64 * ratio * 1.05).ceil() as usize;
    let stale_denominator = estimate_tokens_bpe(&earlier_user) + 10 + 5;
    let stale_prompt_tokens = (stale_denominator as f64 * 0.92).ceil() as usize;
    let stale_anchor_ratio = stale_prompt_tokens as f64 / stale_denominator as f64;
    let stale_anchor_estimate = stale_prompt_tokens
        + ((estimate_tokens_bpe(&older_anchor_clone_for_bpe)
            + estimate_tokens_bpe(&newer_user)
            + estimate_tokens_bpe(&latest_anchor)
            + estimate_tokens_bpe(&tail)) as f64
            * stale_anchor_ratio
            * 1.05)
            .ceil() as usize;

    let actual = calculate_conservative_preflight_prompt_tokens(&messages, 10, 5, None);
    assert_eq!(actual, expected);
    assert_ne!(actual, stale_anchor_estimate);
}

#[test]
fn test_try_derive_bpe_calibration_ratio_returns_none_without_anchor() {
    let messages = vec![
        make_message("m1", "user", "hello"),
        make_message("m2", "assistant", "no usage here"),
    ];

    assert_eq!(try_derive_bpe_calibration_ratio(&messages, 10, 5), None);
}

#[test]
fn test_conservative_preflight_prompt_tokens_uses_preserved_ratio_when_compaction_loses_anchor() {
    let preserved_context = vec![
        make_message("u1", "user", &"Large earlier context ".repeat(1200)),
        make_message("a1", "assistant", &"Older grounded output ".repeat(1000)),
    ];
    let mut grounded_anchor = make_message("a2", "assistant", "Latest grounded output");
    let mut full_messages = preserved_context.clone();
    let grounded_denominator =
        full_messages.iter().map(estimate_tokens_bpe).sum::<usize>() + 10 + 5;
    grounded_anchor.usage = Some(json!({
        "promptTokens": (grounded_denominator as f64 * 0.9).ceil() as usize
    }));
    full_messages.push(grounded_anchor.clone());
    let preserved_ratio = try_derive_bpe_calibration_ratio(&full_messages, 10, 5)
        .expect("expected grounded promptTokens anchor");

    let compacted_messages = vec![
        make_compact_summary_message("compact-summary-1", "assistant", "Compacted summary"),
        make_message("u-tail", "user", "Fresh request after compaction"),
    ];
    let base = compacted_messages
        .iter()
        .map(estimate_tokens_bpe)
        .sum::<usize>()
        + 10
        + 5;

    let conservative = calculate_conservative_preflight_prompt_tokens(
        &compacted_messages,
        10,
        5,
        Some(preserved_ratio),
    );

    assert_eq!(
        conservative,
        ((base as f64 * preserved_ratio).ceil() as f64 * 1.05).ceil() as usize
    );
}

#[test]
fn test_resolve_preserved_calibration_ratio_prefers_post_compaction_layout() {
    let raw_prefix = make_message(
        "u1",
        "user",
        &"large raw prefix before compaction ".repeat(1200),
    );
    let mut raw_anchor = make_message("a1", "assistant", "grounded output");
    let raw_denominator = estimate_tokens_bpe(&raw_prefix) + 10 + 5;
    raw_anchor.usage = Some(json!({
      "promptTokens": (raw_denominator as f64 * 0.95).ceil() as usize
    }));
    let raw_messages = vec![raw_prefix.clone(), raw_anchor];

    let summary = make_compact_summary_message(
        "compact-summary-1",
        "assistant",
        &"compacted summary ".repeat(1500),
    );
    let tail_user = make_message("u2", "user", &"tail before anchor ".repeat(1000));
    let mut tail_anchor = make_message("a2", "assistant", "tail grounded output");
    let prompt_denominator =
        estimate_tokens_bpe(&summary) + estimate_tokens_bpe(&tail_user) + 10 + 5;
    tail_anchor.usage = Some(json!({
      "promptTokens": (prompt_denominator as f64 * 0.9).ceil() as usize
    }));
    let prompt_messages = vec![summary.clone(), tail_user.clone(), tail_anchor];

    let resolved = resolve_preserved_calibration_ratio(&raw_messages, &prompt_messages, 10, 5)
        .expect("expected calibration ratio");

    let expected_post_ratio = (prompt_denominator as f64 * 0.9).ceil() / prompt_denominator as f64;
    let stale_raw_ratio = (raw_denominator as f64 * 0.95).ceil() / raw_denominator as f64;

    assert!((resolved - expected_post_ratio).abs() < 1e-9);
    assert_ne!(resolved, stale_raw_ratio);
}

#[test]
fn test_try_derive_bpe_calibration_ratio_skips_short_prefix_anchor_and_uses_older_valid_anchor() {
    let earlier_user = make_message("u1", "user", &"stable prefix ".repeat(1500));
    let mut earlier_anchor = make_message("a1", "assistant", "older grounded output");
    let earlier_denominator = estimate_tokens_bpe(&earlier_user) + 10 + 5;
    earlier_anchor.usage = Some(json!({
        "promptTokens": (earlier_denominator as f64 * 0.92).ceil() as usize
    }));

    let short_summary = make_compact_summary_message("compact-summary-1", "assistant", "tiny");
    let mut latest_anchor = make_message("a2", "assistant", "latest but unstable anchor");
    let latest_denominator = estimate_tokens_bpe(&earlier_user)
        + estimate_tokens_bpe(&earlier_anchor)
        + estimate_tokens_bpe(&short_summary)
        + 10
        + 5;
    latest_anchor.usage = Some(json!({
        "promptTokens": (latest_denominator as f64 * 1.6).ceil() as usize
    }));

    let messages = vec![
        earlier_user,
        earlier_anchor.clone(),
        short_summary,
        latest_anchor,
    ];
    let ratio = try_derive_bpe_calibration_ratio(&messages, 10, 5)
        .expect("expected earlier valid anchor to be used");

    let expected = (earlier_denominator as f64 * 0.92).ceil() / earlier_denominator as f64;
    assert!((ratio - expected).abs() < 1e-9);
}

#[test]
fn test_try_derive_bpe_calibration_ratio_accepts_valid_low_ratio_anchor() {
    let prefix = make_message("u1", "user", &"cross tokenizer prefix ".repeat(1500));
    let mut anchor = make_message("a1", "assistant", "grounded output");
    let denominator = estimate_tokens_bpe(&prefix) + 10 + 5;
    let valid_ratio = (PROMPT_ANCHOR_RATIO_MIN + 1.0) / 2.0;
    let prompt_tokens = (denominator as f64 * valid_ratio).ceil() as usize;
    anchor.usage = Some(json!({
        "promptTokens": prompt_tokens
    }));

    let messages = vec![prefix, anchor];
    let ratio = try_derive_bpe_calibration_ratio(&messages, 10, 5)
        .expect("expected valid low grounded anchor to be accepted");

    let expected = prompt_tokens as f64 / denominator as f64;
    assert!((ratio - expected).abs() < 1e-9);
}

#[test]
fn test_try_derive_bpe_calibration_ratio_rejects_extreme_low_ratio_anchor() {
    let prefix = make_message("u1", "user", &"extreme ratio prefix ".repeat(1500));
    let mut anchor = make_message("a1", "assistant", "grounded output");
    let denominator = estimate_tokens_bpe(&prefix) + 10 + 5;
    let invalid_ratio = PROMPT_ANCHOR_RATIO_MIN / 2.0;
    anchor.usage = Some(json!({
        "promptTokens": (denominator as f64 * invalid_ratio).ceil() as usize
    }));

    let messages = vec![prefix, anchor];
    assert_eq!(try_derive_bpe_calibration_ratio(&messages, 10, 5), None);
}

#[test]
fn test_select_messages_within_context_uses_preserved_calibration_after_compaction() {
    let oversized_summary =
        make_compact_summary_message("compact-summary-1", "assistant", &"summary ".repeat(800));
    let tail = make_message("u-tail", "user", "Keep this newest turn");

    let options = SelectionOptions {
        system_prompt: Some("system prompt".to_string()),
        tools_json: Some("[]".to_string()),
        max_messages: None,
        max_tool_calls_per_message: Some(4),
        pin_first_user_message: false,
        fallback_calibration_ratio: Some(0.25),
    };

    let selected = select_messages_within_context(
        &[oversized_summary.clone(), tail.clone()],
        "gemini",
        Some(600),
        Some(&options),
        Some(&ModelContextInfo {
            context_window: 128_000,
        }),
    );

    assert_eq!(selected.len(), 2);
    assert_eq!(selected[0].id, oversized_summary.id);
    assert_eq!(selected[1].id, tail.id);
}

#[test]
fn test_trim_messages_to_fit_conservative_limit_drops_oldest_messages_until_fit() {
    let summary =
        make_compact_summary_message("compact-summary-1", "assistant", &"summary ".repeat(700));
    let older = make_message("older", "assistant", &"older context ".repeat(250));
    let newest = make_message("newest", "user", "Newest actionable turn");
    let preserved_ratio = Some(0.25);
    let limit = 250;

    let trimmed = trim_messages_to_fit_conservative_limit(
        &[summary.clone(), older, newest.clone()],
        "gemini",
        limit,
        10,
        5,
        preserved_ratio,
    );

    assert_eq!(trimmed.len(), 2);
    assert_eq!(trimmed[0].id, "older");
    assert_eq!(trimmed[1].id, newest.id);
    assert!(
        calculate_conservative_preflight_prompt_tokens(&trimmed, 10, 5, preserved_ratio) < limit
    );
}

#[test]
fn test_trim_messages_to_fit_conservative_limit_preserves_resolved_tool_chain() {
    let mut assistant = make_message("assistant", "assistant", "Calling tool");
    assistant.tool_calls = Some(vec![AgentToolCall {
        id: "call_1".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "toolA".to_string(),
            arguments: "{}".to_string(),
        },
    }]);
    let mut tool_result = make_message("tool", "tool", "Tool result");
    tool_result.tool_call_id = Some("call_1".to_string());
    let newest = make_message("newest", "user", "Newest user turn");

    let trimmed = trim_messages_to_fit_conservative_limit(
        &[assistant.clone(), tool_result.clone(), newest.clone()],
        "gemini",
        usize::MAX,
        10,
        5,
        None,
    );

    assert_eq!(trimmed.len(), 3);
    assert_eq!(trimmed[0].id, assistant.id);
    assert_eq!(trimmed[1].id, tool_result.id);
    assert_eq!(trimmed[2].id, newest.id);
}

#[test]
fn test_truncate_single_oversized_message_to_fit_conservative_limit_truncates_user_text() {
    let oversized = make_message("user-1", "user", &"very large latest request ".repeat(600));
    let limit = 600;

    let truncated = truncate_single_oversized_message_to_fit_conservative_limit(
        std::slice::from_ref(&oversized),
        limit,
        10,
        5,
        None,
    );

    assert_eq!(truncated.len(), 1);
    assert_ne!(
        estimate_tokens_bpe(&truncated[0]),
        estimate_tokens_bpe(&oversized)
    );
    let MCPContent::Text { text, .. } = &truncated[0].content[0] else {
        panic!("expected text content");
    };
    assert!(text.contains("...[truncated for context fit]..."));
    assert!(calculate_conservative_preflight_prompt_tokens(&truncated, 10, 5, None) < limit);
}

#[test]
fn test_truncate_single_oversized_message_to_fit_conservative_limit_skips_assistant_tool_call_message(
) {
    let mut assistant = make_message("assistant-1", "assistant", "Calling tool");
    assistant.tool_calls = Some(vec![AgentToolCall {
        id: "call_1".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "toolA".to_string(),
            arguments: "{}".to_string(),
        },
    }]);

    let truncated = truncate_single_oversized_message_to_fit_conservative_limit(
        &[assistant.clone()],
        100,
        10,
        5,
        None,
    );

    assert_eq!(truncated.len(), 1);
    assert_eq!(
        estimate_tokens_bpe(&truncated[0]),
        estimate_tokens_bpe(&assistant)
    );
    assert!(truncated[0].tool_calls.is_some());
    assert_eq!(truncated[0].role, assistant.role);
}

#[test]
fn test_fit_compaction_request_messages_to_limit_rejects_lossy_multi_message_trimming() {
    let summary =
        make_compact_summary_message("compact-summary-1", "assistant", &"summary ".repeat(700));
    let older = make_message("older", "assistant", &"older context ".repeat(250));
    let newest = make_message("newest", "user", "Newest actionable turn");
    let limit = calculate_conservative_preflight_prompt_tokens(
        &[summary.clone(), newest.clone()],
        10,
        5,
        None,
    ) + 1;

    let error = fit_compaction_request_messages_to_limit(
        &[summary.clone(), older, newest.clone()],
        "gemini",
        limit,
        10,
        5,
    )
    .expect_err("compaction-fit should fail instead of dropping older raw history");

    assert!(error.contains("without lossy cache-aligned trimming"));
}

#[test]
fn test_fit_compaction_request_messages_to_limit_rejects_summary_only_payload() {
    let summary =
        make_compact_summary_message("compact-summary-1", "assistant", &"summary ".repeat(700));
    let delta = make_message("delta", "user", "Fresh delta that will be dropped");

    let error = fit_compaction_request_messages_to_limit(&[summary, delta], "gemini", 80, 10, 5)
        .expect_err("compaction should fail instead of summarizing only the prior summary");

    assert!(
        error.contains("prior compact summary anchor") || error.contains("effective context limit")
    );
}

#[test]
fn test_fit_compaction_request_messages_to_limit_truncates_single_raw_message() {
    let oversized = make_message("user-1", "user", &"very large compact input ".repeat(600));

    let fitted = fit_compaction_request_messages_to_limit(
        std::slice::from_ref(&oversized),
        "openai",
        600,
        10,
        5,
    )
    .expect("single raw message should be truncated to fit compaction request");

    assert_eq!(fitted.len(), 1);
    assert_ne!(
        estimate_tokens_bpe(&fitted[0]),
        estimate_tokens_bpe(&oversized)
    );
    let MCPContent::Text { text, .. } = &fitted[0].content[0] else {
        panic!("expected text content");
    };
    assert!(text.contains("...[truncated for context fit]..."));
}

#[test]
fn test_build_compact_summary_text_includes_recent_tool_snapshot() {
    let mut assistant = make_message("assistant-1", "assistant", "Writing file");
    assistant.tool_calls = Some(vec![AgentToolCall {
        id: "call_write".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "workspace__writeFile".to_string(),
            arguments: "{\"path\":\"src/app.tsx\",\"content\":\"updated\"}".to_string(),
        },
    }]);

    let mut tool = make_message("tool-1", "tool", "Successfully wrote src/app.tsx");
    tool.tool_call_id = Some("call_write".to_string());

    let summary = build_compact_summary_text("User asked for an update.", &[assistant, tool]);

    assert!(summary.contains("### Previous Conversation Summary"));
    assert!(summary.contains("### Recent Tool Call Snapshot (latest 5)"));
    assert!(summary.contains("workspace__writeFile(content=updated, path=src/app.tsx) -> success: Successfully wrote src/app.tsx"));
}

#[test]
fn test_build_compact_summary_text_limits_snapshot_to_latest_five_completed_tool_calls() {
    let mut messages = Vec::new();

    for index in 0..6 {
        let mut assistant =
            make_message(&format!("assistant-{index}"), "assistant", "Calling tool");
        assistant.tool_calls = Some(vec![AgentToolCall {
            id: format!("call_{index}"),
            r#type: "function".to_string(),
            function: ToolCallFunction {
                name: "workspace__writeFile".to_string(),
                arguments: format!("{{\"path\":\"file-{index}.txt\"}}"),
            },
        }]);
        messages.push(assistant);

        let mut tool = make_message(
            &format!("tool-{index}"),
            "tool",
            &format!("Wrote file-{index}.txt"),
        );
        tool.tool_call_id = Some(format!("call_{index}"));
        messages.push(tool);
    }

    let summary = build_compact_summary_text("Compacted summary", &messages);

    assert!(!summary.contains("file-0.txt"));
    assert!(summary.contains("file-1.txt"));
    assert!(summary.contains("file-5.txt"));
}

#[test]
fn test_backend_compact_summary_clamp_uses_wrapped_token_budget() {
    let compacted_messages = vec![
        make_message(
            "user-1",
            "user",
            "Need a concise recap of the ongoing contract review.",
        ),
        make_message(
            "assistant-1",
            "assistant",
            "I will inspect the file and compare terms.",
        ),
    ];
    let expected_hard_limit_tokens = 13_107;
    let mut oversized_summary = "A ".repeat(80_000);
    let mut wrapped_summary = build_compact_summary_message_for_messages(
        TEST_SESSION_ID,
        &oversized_summary,
        &compacted_messages,
        0,
    );
    while estimate_tokens_bpe(&wrapped_summary) <= expected_hard_limit_tokens {
        oversized_summary.push_str(&"A ".repeat(20_000));
        wrapped_summary = build_compact_summary_message_for_messages(
            TEST_SESSION_ID,
            &oversized_summary,
            &compacted_messages,
            0,
        );
    }

    let result = clamp_compact_summary_to_context_limit(
        TEST_SESSION_ID,
        &oversized_summary,
        &compacted_messages,
        131_072,
    );

    assert!(result.was_clamped);
    assert_eq!(result.hard_limit_tokens, expected_hard_limit_tokens);

    let clamped_message = build_compact_summary_message_for_messages(
        TEST_SESSION_ID,
        &result.summary,
        &compacted_messages,
        0,
    );
    assert!(estimate_tokens_bpe(&clamped_message) <= result.hard_limit_tokens);
    assert!(result.original_estimated_tokens > result.hard_limit_tokens);
}

#[test]
fn test_validate_compact_summary_accepts_long_unstructured_summary() {
    let summary = "The agent inspected the failing build path, compared the recent compiler output, identified the namespace mismatch in main.cpp, confirmed the renderer z-buffer access bug, and noted that the next step is applying the remaining fixes before rebuilding."
        .to_string();

    let result = validate_compact_summary_for_testing(&summary, 8);

    assert!(result.is_ok());
}

#[test]
fn test_validate_compact_summary_rejects_short_summary_for_large_compaction() {
    let result = validate_compact_summary_for_testing("Too short to be useful.", 8);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Compaction summary was too short"));
}

#[test]
fn test_compaction_fallback_artifact_relative_path_uses_compaction_tool_results_dir() {
    let path = compaction_fallback_artifact_relative_path_for_testing(
        TEST_SESSION_ID,
        "message:with spaces",
        1_717_060_000_000,
    );

    assert!(path.starts_with(".libragent/tool-results/compaction/"));
    assert!(path.ends_with(".md"));
    assert!(!path.contains(' '));
    assert!(!path.contains(':'));
}

#[test]
fn test_build_compaction_hard_fallback_summary_includes_sections_and_artifact_guidance() {
    let messages = vec![
        TestMessageBuilder::new("user-fallback", "user")
            .text(
                "Investigate compaction failures in src-tauri/src/agent/session_manager/compact.rs",
            )
            .source(MessageSource::Ui)
            .build(),
        {
            let mut assistant = make_message(
                "assistant-fallback",
                "assistant",
                "Running workspace__readFile",
            );
            assistant.tool_calls = Some(vec![AgentToolCall {
                id: "call_fallback".to_string(),
                r#type: "function".to_string(),
                function: ToolCallFunction {
                    name: "workspace__readFile".to_string(),
                    arguments: "{\"path\":\"src-tauri/src/agent/session_manager/compact.rs\"}"
                        .to_string(),
                },
            }]);
            assistant
        },
        {
            let mut tool = make_message(
                "tool-fallback",
                "tool",
                "Opened src-tauri/src/agent/session_manager/compact.rs and found repeated summary retry failures.",
            );
            tool.tool_call_id = Some("call_fallback".to_string());
            tool
        },
    ];

    let summary = build_compaction_hard_fallback_summary_for_testing(
        &messages,
        ".libragent/tool-results/compaction/fallback-123.md",
        "tool-fallback",
        7,
        CompactionRecoveryPhase::DegradedTools,
        3,
        "Compaction summary was too short: got 41 chars.",
    );

    assert!(summary.contains("### Active Request"));
    assert!(summary.contains("### Required References"));
    assert!(summary.contains("### Current State"));
    assert!(summary.contains("### Recent Tool Results"));
    assert!(summary.contains("### Next Actions"));
    assert!(summary.contains("### Fallback Note"));
    assert!(summary.contains(".libragent/tool-results/compaction/fallback-123.md"));
    assert!(summary.contains("Open `.libragent/tool-results/compaction/fallback-123.md`"));
    assert!(summary.contains("Auto-saved via fallback summary"));
}

#[test]
fn test_clear_message_prompt_token_checkpoint_clears_direct_and_usage_truth() {
    let mut message = TestMessageBuilder::new("m-usage", "assistant")
        .text("checkpoint")
        .build();
    message.prompt_tokens = Some(12_345);
    message.usage = Some(json!({
        "promptTokens": 67_890,
        "completionTokens": 321
    }));

    assert!(clear_message_prompt_token_checkpoint_for_testing(
        &mut message
    ));
    assert_eq!(message.prompt_tokens_value(), None);
    assert_eq!(
        message
            .usage
            .as_ref()
            .and_then(|usage| usage.get("completionTokens"))
            .and_then(|value| value.as_u64()),
        Some(321)
    );
}

#[test]
fn test_build_compact_summary_text_caps_long_argument_preview() {
    let mut assistant = make_message("assistant-long", "assistant", "Writing file");
    assistant.tool_calls = Some(vec![AgentToolCall {
        id: "call_long".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "workspace__writeFile".to_string(),
            arguments: format!(
                "{{\"content\":\"{}\",\"path\":\"src/huge.ts\"}}",
                "a".repeat(300)
            ),
        },
    }]);

    let mut tool = make_message("tool-long", "tool", "Wrote src/huge.ts");
    tool.tool_call_id = Some("call_long".to_string());

    let summary = build_compact_summary_text("Compacted summary", &[assistant, tool]);

    assert!(summary.contains("workspace__writeFile("));
    assert!(summary.contains("path=src/huge.ts"));
    assert!(!summary.contains(&"a".repeat(150)));
}

#[test]
fn test_build_compact_summary_message_for_messages_reuses_normal_request_wrapper() {
    let mut assistant = make_message("assistant-shared", "assistant", "Running command");
    assistant.tool_calls = Some(vec![AgentToolCall {
        id: "call_shared".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "workspace__runCommand".to_string(),
            arguments: "{\"command\":\"git status\"}".to_string(),
        },
    }]);

    let mut tool = make_message("tool-shared", "tool", "On branch dev/0.7.x");
    tool.tool_call_id = Some("call_shared".to_string());

    let summary_message = build_compact_summary_message_for_messages(
        "test-session",
        "Earlier work was summarized.",
        &[assistant, tool],
        42,
    );

    assert_eq!(summary_message.id, "compact-summary-test-session");
    assert_eq!(summary_message.role, "assistant");
    assert_eq!(summary_message.source, Some(MessageSource::CompactSummary));

    let MCPContent::Text { text, .. } = &summary_message.content[0] else {
        panic!("expected compact summary text");
    };
    assert!(text.contains("### Previous Conversation Summary"));
    assert!(text.contains("### Recent Tool Call Snapshot (latest 5)"));
    assert!(
        text.contains("workspace__runCommand(command=git status) -> success: On branch dev/0.7.x")
    );
}
