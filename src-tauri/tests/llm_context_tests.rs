use serde_json::json;
use std::collections::HashMap;
use tauri_mcp_agent_lib::agent::llm::completion::{
    build_compact_context_selection_options, build_compact_summary_text,
    find_preflight_compaction_split_index, merge_consecutive_user_messages,
    resolve_context_management_settings, should_skip_same_tail_compaction,
    should_trigger_background_compaction, uses_compaction_strategy,
};
use tauri_mcp_agent_lib::agent::llm::context_selector::*;
use tauri_mcp_agent_lib::agent::llm::token_utils::*;
use tauri_mcp_agent_lib::agent::types::{ToolCall as AgentToolCall, ToolCallFunction};
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::models::chat::Message;

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

    let idx = find_preflight_compaction_split_index(&[earlier, latest_user]);
    assert_eq!(idx, 1);
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

    let idx = find_preflight_compaction_split_index(&[intro, assistant, tool_result]);
    assert_eq!(idx, 3);
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

    let idx = find_preflight_compaction_split_index(&[intro, assistant, tool_result]);
    assert_eq!(idx, 1);
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

    let idx = find_preflight_compaction_split_index(&cleaned);
    assert_eq!(idx, cleaned.len());
}

#[test]
fn test_same_tail_compaction_skips_duplicate_when_no_compact_summary_exists() {
    let messages = vec![
        make_message("m0", "assistant", "Earlier context"),
        make_message("m1", "user", "Latest request"),
    ];

    assert!(should_skip_same_tail_compaction(&messages, 1));
}

#[test]
fn test_same_tail_compaction_allows_follow_up_compaction_after_summary_injection() {
    let mut summary = make_message("m0", "user", "Compacted summary");
    summary.id = "compact-summary-test".to_string();

    let messages = vec![
        summary,
        make_message("m1", "assistant", "Large preserved context"),
        make_message("m2", "user", "Latest request"),
    ];

    assert!(!should_skip_same_tail_compaction(&messages, 2));
}

#[test]
fn test_same_tail_compaction_stops_when_only_existing_summary_is_left_to_compact() {
    let mut summary = make_message("m0", "user", "Compacted summary");
    summary.id = "compact-summary-test".to_string();

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
    let summary = make_message("compact-summary-1", "user", "Summary");

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
    );

    assert_eq!(options.system_prompt.as_deref(), Some("system"));
    assert_eq!(options.tools_json.as_deref(), Some("tools"));
    assert_eq!(options.max_messages, None);
    assert_eq!(options.max_tool_calls_per_message, Some(7));
    assert!(!options.pin_first_user_message);
}

#[test]
fn test_compact_mode_selection_options_keep_gemini_tool_visibility_contract() {
    let options = build_compact_context_selection_options(None, None, "gemini", 7);

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
    assert_eq!(calculate_compact_threshold(10000), 9000);
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
fn test_grounded_total_tokens_ignores_grounding_after_compaction() {
    // After compaction, Step A rebuilds as [compact-summary, ...tail].
    // compact-summary appears BEFORE the grounded assistant message,
    // so calculate_grounded_total_tokens must fall back to full BPE.
    let mut summary = make_message_simple("system", "Summary...");
    summary.id = "compact-summary-123".to_string();

    let mut grounded = make_message_simple("assistant", "Hi there");
    grounded.usage = Some(json!({ "totalTokens": 100 }));

    let tail = make_message_simple("user", "Hello");

    // compact-summary (idx 0) is BEFORE grounded assistant (idx 1) → BPE fallback
    let messages = vec![summary.clone(), grounded.clone(), tail.clone()];

    let tokens = calculate_grounded_total_tokens(&messages, 10, 5);
    let expected = estimate_tokens_bpe(&summary)
        + estimate_tokens_bpe(&grounded)
        + estimate_tokens_bpe(&tail)
        + 10
        + 5;

    assert_eq!(tokens, expected);
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
