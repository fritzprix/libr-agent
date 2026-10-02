//! Message decode tests for the Chrome MV3 extension bridge protocol.

use serde_json::json;
use tauri_mcp_agent_lib::browser_extension_bridge::{
    decode_reply, decode_request, ExtensionMethod, DEFAULT_BRIDGE_PORT, DEV_BRIDGE_TOKEN,
};

#[test]
fn decode_create_session_request() {
    let raw = r#"{"id":"abc","method":"createSession","params":{"sessionId":"s1","url":"https://example.com"}}"#;
    let req = decode_request(raw).expect("request should decode");
    assert_eq!(req.id, "abc");
    assert_eq!(req.method, ExtensionMethod::CreateSession.as_str());
    assert_eq!(req.params["sessionId"], "s1");
    assert_eq!(req.params["url"], "https://example.com");
}

#[test]
fn decode_ok_reply_with_tab_state() {
    let raw = r#"{"id":"abc","ok":true,"result":{"url":"https://example.com","title":"Example","tabId":42}}"#;
    let reply = decode_reply(raw).expect("reply should decode");
    assert_eq!(reply.id.as_deref(), Some("abc"));
    assert!(reply.ok);
    let result = reply.result.expect("result");
    assert_eq!(result["url"], "https://example.com");
    assert_eq!(result["tabId"], 42);
}

#[test]
fn decode_error_reply() {
    let raw = r#"{"id":"abc","ok":false,"error":"Unknown sessionId: s1"}"#;
    let reply = decode_reply(raw).expect("reply should decode");
    assert!(!reply.ok);
    assert_eq!(reply.error.as_deref(), Some("Unknown sessionId: s1"));
}

#[test]
fn decode_rejects_invalid_json() {
    let err = decode_reply("{not-json").expect_err("should fail");
    assert!(err.contains("Invalid extension bridge reply JSON"));
}

#[test]
fn roundtrip_request_value() {
    let value = json!({
        "id": "x",
        "method": "navigate",
        "params": { "sessionId": "s", "url": "https://a.test" }
    });
    let raw = value.to_string();
    let req = decode_request(&raw).expect("decode");
    assert_eq!(req.method, "navigate");
}

#[test]
fn documented_defaults() {
    assert_eq!(DEFAULT_BRIDGE_PORT, 3847);
    assert_eq!(DEV_BRIDGE_TOKEN, "libragent-dev");
}
