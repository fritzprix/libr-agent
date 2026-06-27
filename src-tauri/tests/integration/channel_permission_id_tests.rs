use tauri_mcp_agent_lib::agent::tool_approvals::{
    build_channel_permission_description, build_channel_permission_input_preview,
    generate_channel_permission_request_id,
};

#[test]
fn channel_permission_request_ids_are_32_char_uuid_hex() {
    let request_id = generate_channel_permission_request_id();

    assert_eq!(request_id.len(), 32);
    assert!(request_id.chars().all(|ch| ch.is_ascii_hexdigit()));
    assert!(request_id.chars().all(|ch| !ch.is_ascii_uppercase()));
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
