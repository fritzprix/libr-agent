use tauri_mcp_agent_lib::agent::tool_approvals::{
    build_channel_permission_description, build_channel_permission_input_preview,
    generate_channel_permission_request_id,
};

#[test]
fn channel_permission_request_ids_match_claude_constraints() {
    let request_id = generate_channel_permission_request_id();

    assert_eq!(request_id.len(), 5);
    assert!(request_id
        .chars()
        .all(|ch| matches!(ch, 'a'..='k' | 'm'..='z')));
    assert!(!request_id.contains('l'));
}

#[test]
fn channel_permission_preview_preserves_head_and_tail_context() {
    let long_input = "x".repeat(240);
    let preview = build_channel_permission_input_preview(&long_input);

    assert_eq!(preview.chars().count(), 183);
    assert!(preview.contains('…'));
    assert!(preview.starts_with(&"x".repeat(140)));
    assert!(preview.ends_with(&"x".repeat(40)));
}

#[test]
fn channel_permission_description_mentions_tool_name() {
    let description = build_channel_permission_description("Bash", r#"{"command":"ls"}"#);
    assert!(description.contains("Bash"));
}
