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
fn decode_evaluate_and_screenshot_methods() {
    let eval = decode_request(
        r#"{"id":"e1","method":"evaluate","params":{"sessionId":"s1","script":"document.title"}}"#,
    )
    .expect("evaluate request");
    assert_eq!(eval.method, ExtensionMethod::Evaluate.as_str());
    assert_eq!(eval.params["script"], "document.title");

    let click = decode_request(
        r##"{"id":"c1","method":"clickElement","params":{"sessionId":"s1","selector":"#go"}}"##,
    )
    .expect("clickElement request");
    assert_eq!(click.method, ExtensionMethod::ClickElement.as_str());
    assert_eq!(click.params["selector"], "#go");

    let input = decode_request(
        r##"{"id":"i1","method":"inputText","params":{"sessionId":"s1","selector":"input[name=q]","text":"hi"}}"##,
    )
    .expect("inputText request");
    assert_eq!(input.method, ExtensionMethod::InputText.as_str());
    assert_eq!(input.params["text"], "hi");

    let shot = decode_request(
        r#"{"id":"s1","method":"takeScreenshot","params":{"sessionId":"s1","fullPage":false}}"#,
    )
    .expect("screenshot request");
    assert_eq!(shot.method, ExtensionMethod::TakeScreenshot.as_str());

    let back = decode_request(r#"{"id":"b1","method":"goBack","params":{"sessionId":"s1"}}"#)
        .expect("goBack request");
    assert_eq!(back.method, ExtensionMethod::GoBack.as_str());
}

#[test]
fn documented_defaults() {
    assert_eq!(DEFAULT_BRIDGE_PORT, 3847);
    assert_eq!(DEV_BRIDGE_TOKEN, "libragent-dev");
}

#[test]
fn page_state_maps_history_navigation_status_from_extension() {
    use tauri_mcp_agent_lib::browser_extension_bridge::{
        page_state_from_extension_tab, ExtensionTabState,
    };
    use tauri_mcp_agent_lib::browser_sidecar::HistoryNavigationStatus;

    let navigated = page_state_from_extension_tab(ExtensionTabState {
        url: "https://example.com/".into(),
        title: Some("Example".into()),
        tab_id: Some(1),
        navigation_status: Some("navigated".into()),
        navigation_message: None,
    });
    assert_eq!(
        navigated.navigation_status,
        Some(HistoryNavigationStatus::Navigated)
    );
    assert!(navigated.navigation_message.is_none());

    let no_entry = page_state_from_extension_tab(ExtensionTabState {
        url: "https://example.org/".into(),
        title: Some("Example".into()),
        tab_id: Some(1),
        navigation_status: Some("noHistoryEntry".into()),
        navigation_message: Some("back navigation produced no observable page change".into()),
    });
    assert_eq!(
        no_entry.navigation_status,
        Some(HistoryNavigationStatus::NoHistoryEntry)
    );
    assert!(no_entry
        .navigation_message
        .as_deref()
        .unwrap_or("")
        .contains("no observable page change"));

    let create_like = page_state_from_extension_tab(ExtensionTabState {
        url: "https://example.com/".into(),
        title: None,
        tab_id: Some(2),
        navigation_status: None,
        navigation_message: None,
    });
    assert!(create_like.navigation_status.is_none());
    assert_eq!(
        create_like.navigation_message.as_deref(),
        Some("Opened via Chrome extension bridge")
    );
}

#[test]
fn decode_history_reply_includes_navigation_status() {
    let raw = r#"{"id":"b1","ok":true,"result":{"url":"https://example.com/","title":"Example","tabId":7,"navigationStatus":"noHistoryEntry","navigationMessage":"No back history entry"}}"#;
    let reply = decode_reply(raw).expect("reply");
    let result = reply.result.expect("result");
    assert_eq!(result["navigationStatus"], "noHistoryEntry");
    assert_eq!(result["url"], "https://example.com/");
}
