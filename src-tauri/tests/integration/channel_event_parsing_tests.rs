use tauri_mcp_agent_lib::mcp::session_isolation::channel_events::{
    try_parse_channel_event, SessionChannelEvent, MAX_CHANNEL_CONTENT_BYTES,
};

#[test]
fn rejects_oversized_channel_message_content() {
    let oversized = "x".repeat(MAX_CHANNEL_CONTENT_BYTES + 1);
    let line = format!(
        r#"{{"jsonrpc":"2.0","method":"claude/channel","params":{{"content":"{}"}}}}"#,
        oversized
    );
    assert!(
        try_parse_channel_event(line.as_bytes(), "telegram").is_none(),
        "oversized channel content should be dropped"
    );
}

#[test]
fn accepts_channel_message_at_content_limit() {
    let content = "x".repeat(MAX_CHANNEL_CONTENT_BYTES);
    let line = format!(
        r#"{{"jsonrpc":"2.0","method":"claude/channel","params":{{"content":"{}"}}}}"#,
        content
    );
    let event = try_parse_channel_event(line.as_bytes(), "telegram")
        .expect("content at limit should parse");
    match event {
        SessionChannelEvent::Message { notification, .. } => {
            assert_eq!(notification.content.len(), MAX_CHANNEL_CONTENT_BYTES);
        }
        SessionChannelEvent::PermissionVerdict { .. } => {
            panic!("expected message event");
        }
    }
}
