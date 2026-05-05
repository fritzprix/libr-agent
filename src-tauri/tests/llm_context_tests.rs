use serde_json::json;
use std::collections::HashMap;
use tauri_mcp_agent_lib::agent::llm::completion::{
    build_compact_context_selection_options, build_compact_summary_message_for_messages,
    build_compact_summary_text, fit_compaction_request_messages_to_limit,
    merge_consecutive_user_messages, normalize_request_messages,
    preview_background_compaction_selection, preview_preflight_compaction_selection,
    resolve_context_management_settings, resolve_preserved_calibration_ratio,
    should_skip_same_tail_compaction, should_trigger_background_compaction,
    should_trigger_post_response_compaction, uses_compaction_strategy,
};
use tauri_mcp_agent_lib::agent::llm::context_selector::*;
use tauri_mcp_agent_lib::agent::llm::response::build_post_response_compaction_snapshot;
use tauri_mcp_agent_lib::agent::llm::token_utils::*;
use tauri_mcp_agent_lib::agent::types::{ToolCall as AgentToolCall, ToolCallFunction};
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::{Message, MessageSource};

fn make_message(id: &str, role: &str, text: &str) -> Message {
    Message {
        id: id.to_string(),
        session_id: "test-session".to_string(),
        role: role.to_string(),
        content: vec![MCPContent::Text {
            text: text.to_string(),
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
        created_at: 0,
        updated_at: 0,
        source: None,
        error: None,
        metadata: None,
    }
}

fn make_message_simple(role: &str, text: &str) -> Message {
    make_message(&format!("msg-{}", text.len()), role, text)
}

fn make_compact_summary_message(id: &str, role: &str, text: &str) -> Message {
    let mut message = make_message(id, role, text);
    message.source = Some(MessageSource::CompactSummary);
    message
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
fn test_find_preflight_compaction_split_index_preserves_latest_user_turn() {
    let earlier = make_message("m0", "assistant", "Earlier context");
    let latest_user = make_message("m1", "user", &"latest request ".repeat(200));

    let preview = preview_preflight_compaction_selection(&[earlier, latest_user]);
    assert_eq!(preview.compacted_ids, vec!["m0"]);
    assert_eq!(preview.preserved_ids, vec!["m1"]);
}

#[test]
fn test_find_preflight_compaction_split_index_preserves_latest_non_tool_turn() {
    let earlier = make_message("m0", "user", "Earlier user context");
    let latest_assistant = make_message("m1", "assistant", "Latest non-tool turn");

    let preview = preview_preflight_compaction_selection(&[earlier, latest_assistant]);
    assert_eq!(preview.compacted_ids, vec!["m0"]);
    assert_eq!(preview.preserved_ids, vec!["m1"]);
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
fn test_find_background_compaction_split_index_preserves_active_request_before_deferred_tool_execution(
) {
    let request = make_message("m0", "user", "Refactor auth to JWT");

    let mut assistant = make_message("m1", "assistant", "Calling tools");
    assistant.tool_calls = Some(vec![AgentToolCall {
        id: "call_A".to_string(),
        r#type: "function".to_string(),
        function: ToolCallFunction {
            name: "toolA".to_string(),
            arguments: "{}".to_string(),
        },
    }]);

    let preview = preview_background_compaction_selection(&[request, assistant]);
    assert!(preview.compacted_ids.is_empty());
    assert_eq!(preview.preserved_ids, vec!["m0", "m1"]);
}

#[test]
fn test_find_background_compaction_split_index_ignores_internal_synthetic_user_messages() {
    let mut synthetic = make_message("m1", "user", "Synthetic compaction prompt");
    synthetic.source = Some(MessageSource::CompactionInstruction);

    assert!(!synthetic.is_external_request_message());

    let preview = preview_background_compaction_selection(&[synthetic]);
    assert_eq!(preview.compacted_ids, vec!["m1"]);
    assert!(preview.preserved_ids.is_empty());
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
fn test_find_background_compaction_split_index_preserves_latest_external_request_block() {
    let older = make_message("m0", "user", "Older request");

    let newer = make_message("m1", "user", "Latest real request");
    let assistant = make_message("m2", "assistant", "Working on latest request");

    let preview = preview_background_compaction_selection(&[older, newer, assistant]);
    assert!(preview.compacted_ids.is_empty());
    assert_eq!(preview.preserved_ids, vec!["m0", "m1", "m2"]);
}

#[test]
fn test_build_post_response_compaction_snapshot_appends_pending_message_once() {
    let request = make_message("m0", "user", "Latest real request");
    let pending = make_message("m1", "assistant", "Pending assistant turn");

    let snapshot = build_post_response_compaction_snapshot(&[request], Some(&pending));
    assert_eq!(snapshot.len(), 2);
    assert_eq!(snapshot[0].id, "m0");
    assert_eq!(snapshot[1].id, "m1");
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

    assert!(!should_skip_same_tail_compaction(&messages, 1));
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

    assert!(!should_skip_same_tail_compaction(&messages, 2));
}

#[test]
fn test_same_tail_compaction_stops_when_only_existing_summary_is_left_to_compact() {
    let summary =
        make_compact_summary_message("compact-summary-test", "assistant", "Compacted summary");

    let messages = vec![summary, make_message("m1", "user", "Latest request")];

    assert!(should_skip_same_tail_compaction(&messages, 1));
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
fn test_remove_incomplete_tool_chains_drops_orphan_tool_from_unstable_suffix_only() {
    let stable_user = make_message("m1", "user", "Stable prefix");
    let mut orphan_tool = make_message("m2", "tool", "orphan result");
    orphan_tool.tool_call_id = Some("missing_call".to_string());

    let cleaned = remove_incomplete_tool_chains(vec![stable_user.clone(), orphan_tool]);

    assert_eq!(cleaned.len(), 1);
    assert_eq!(cleaned[0].id, stable_user.id);
}

#[test]
fn test_merge_consecutive_user_messages_only_merges_trailing_run() {
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
fn test_select_messages_regression_large_message() {
    let msgs = vec![make_message(
        "big_msg",
        "user",
        &"Very long content ".repeat(100),
    )];

    let selected = select_messages_within_context(&msgs, "gemini", Some(10), None, None);
    assert!(selected.is_empty());
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
fn test_estimate_text_tokens() {
    let text = "Hello, world! This is a test.";
    let tokens = estimate_text_tokens(text);
    assert!(tokens > 0);
}

#[test]
fn test_calculate_compact_threshold() {
    assert_eq!(calculate_compact_threshold(10000), 9500);
}

#[test]
fn test_background_compaction_trigger_uses_threshold_boundary() {
    let safe_limit = 49152;
    let threshold = calculate_compact_threshold(safe_limit);

    assert!(!should_trigger_background_compaction(
        threshold, safe_limit, "compact"
    ));
    assert!(should_trigger_background_compaction(
        threshold + 1,
        safe_limit,
        "compact"
    ));
}

#[test]
fn test_background_compaction_trigger_respects_strategy() {
    let safe_limit = 49152;
    let threshold = calculate_compact_threshold(safe_limit);

    assert!(!should_trigger_background_compaction(
        threshold + 500,
        safe_limit,
        "window"
    ));
}

#[test]
fn test_post_response_compaction_trigger_matches_background_threshold_contract() {
    let safe_limit = 49152;
    let threshold = calculate_compact_threshold(safe_limit);

    assert!(!should_trigger_post_response_compaction(
        threshold, safe_limit, "compact"
    ));
    assert!(should_trigger_post_response_compaction(
        threshold + 1,
        safe_limit,
        "compact"
    ));
    assert!(should_trigger_post_response_compaction(
        safe_limit, safe_limit, "compact"
    ));
    assert!(!should_trigger_post_response_compaction(
        threshold + 1,
        safe_limit,
        "window"
    ));
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
fn test_fit_compaction_request_messages_to_limit_preserves_summary_anchor() {
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

    let fitted = fit_compaction_request_messages_to_limit(
        &[summary.clone(), older, newest.clone()],
        "gemini",
        limit,
        10,
        5,
    )
    .expect("compaction payload should fit after dropping raw delta");

    assert_eq!(fitted.len(), 2);
    assert_eq!(fitted[0].id, summary.id);
    assert_eq!(fitted[1].id, newest.id);
}

#[test]
fn test_fit_compaction_request_messages_to_limit_rejects_summary_only_payload() {
    let summary =
        make_compact_summary_message("compact-summary-1", "assistant", &"summary ".repeat(700));
    let delta = make_message("delta", "user", "Fresh delta that will be dropped");

    let error = fit_compaction_request_messages_to_limit(&[summary, delta], "gemini", 80, 10, 5)
        .expect_err("compaction should fail instead of summarizing only the prior summary");

    assert!(error.contains("prior compact summary anchor"));
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
