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

#[test]
fn channel_payload_filters_dangerous_html_meta_attribute_names() {
    let mut meta = HashMap::new();
    meta.insert("onclick".to_string(), "alert(1)".to_string());
    meta.insert("onerror".to_string(), "alert(1)".to_string());
    meta.insert("style".to_string(), "color:red".to_string());
    meta.insert("safe_meta".to_string(), "ok".to_string());

    let payload = format_channel_payload_for_test("bridge", "hello", &meta);

    assert!(payload.contains(r#"<channel source="bridge" safe_meta="ok">"#));
    assert!(!payload.contains(r#"onclick="#));
    assert!(!payload.contains(r#"onerror="#));
    assert!(!payload.contains(r#"style="#));
    assert!(payload.contains("[channel_meta]"));
    assert!(payload.contains("onclick=alert(1)"));
    assert!(payload.contains("onerror=alert(1)"));
    assert!(payload.contains("style=color:red"));
}

#[test]
fn channel_payload_allows_legitimate_on_prefixed_meta_keys() {
    let mut meta = HashMap::new();
    meta.insert("one".to_string(), "1".to_string());
    meta.insert("online".to_string(), "true".to_string());
    meta.insert("oncall".to_string(), "alice".to_string());
    meta.insert("ongoing".to_string(), "yes".to_string());

    let payload = format_channel_payload_for_test("bridge", "hello", &meta);

    assert!(payload.contains(r#"one="1""#));
    assert!(payload.contains(r#"online="true""#));
    assert!(payload.contains(r#"oncall="alice""#));
    assert!(payload.contains(r#"ongoing="yes""#));
    assert!(!payload.contains("[channel_meta]"));
}

#[test]
fn channel_payload_filters_reserved_names_case_insensitively() {
    let mut meta = HashMap::new();
    meta.insert("SOURCE".to_string(), "override".to_string());
    meta.insert("STYLE".to_string(), "color:red".to_string());
    meta.insert("OnClick".to_string(), "alert(1)".to_string());
    meta.insert("safe_meta".to_string(), "ok".to_string());

    let payload = format_channel_payload_for_test("bridge", "hello", &meta);

    assert!(payload.contains(r#"<channel source="bridge" safe_meta="ok">"#));
    assert!(!payload.contains(r#"SOURCE="#));
    assert!(!payload.contains(r#"STYLE="#));
    assert!(!payload.contains(r#"OnClick="#));
    assert!(payload.contains("[channel_meta]"));
    assert!(payload.contains("SOURCE=override"));
    assert!(payload.contains("STYLE=color:red"));
    assert!(payload.contains("OnClick=alert(1)"));
}
