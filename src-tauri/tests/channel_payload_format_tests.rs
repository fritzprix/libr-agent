use std::collections::HashMap;

use tauri_mcp_agent_lib::agent::session_manager::format_channel_payload_for_test;

#[test]
fn channel_payload_filters_unsafe_meta_keys_and_preserves_them_in_body() {
    let mut meta = HashMap::new();
    meta.insert("source".to_string(), "override".to_string());
    meta.insert("valid_key".to_string(), "ok".to_string());
    meta.insert("bad key".to_string(), "<bad>".to_string());
    meta.insert("9starts_wrong".to_string(), "value".to_string());

    let payload = format_channel_payload_for_test("bridge", "hello", &meta);

    assert!(payload.contains(r#"<channel source="bridge" valid_key="ok">"#));
    assert!(!payload.contains(r#"source="override""#));
    assert!(!payload.contains(r#"bad key="&lt;bad&gt;""#));
    assert!(!payload.contains(r#"9starts_wrong="value""#));
    assert!(payload.contains("[channel_meta]"));
    assert!(payload.contains("source=override"));
    assert!(payload.contains("bad key=&lt;bad&gt;"));
    assert!(payload.contains("9starts_wrong=value"));
}
