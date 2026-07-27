use tauri_mcp_agent_lib::agent::types::CreateSessionRequest;
use tauri_mcp_agent_lib::agent::ExecutionMode;

#[test]
fn create_session_request_deserializes_execution_mode_yolo() {
    let request: CreateSessionRequest = serde_json::from_value(serde_json::json!({
        "assistantId": "assistant-1",
        "request": "run unattended",
        "executionMode": "yolo"
    }))
    .expect("request should deserialize");

    assert_eq!(request.execution_mode, Some(ExecutionMode::Yolo));
}

#[test]
fn create_session_request_defaults_execution_mode_when_omitted() {
    let request: CreateSessionRequest = serde_json::from_value(serde_json::json!({
        "assistantId": "assistant-1",
        "request": "hello"
    }))
    .expect("request should deserialize");

    assert_eq!(request.execution_mode, None);
}

#[test]
fn create_session_request_deserializes_execution_mode_unsafe() {
    let request: CreateSessionRequest = serde_json::from_value(serde_json::json!({
        "assistantId": "assistant-1",
        "request": "run shell unattended",
        "executionMode": "unsafe"
    }))
    .expect("request should deserialize");

    assert_eq!(request.execution_mode, Some(ExecutionMode::Unsafe));
}

#[test]
fn create_session_request_rejects_invalid_execution_mode() {
    let result = serde_json::from_value::<CreateSessionRequest>(serde_json::json!({
        "assistantId": "assistant-1",
        "request": "hello",
        "executionMode": "Yolo"
    }));

    assert!(
        result.is_err(),
        "serde rename_all=lowercase should reject PascalCase Yolo"
    );
}

#[test]
fn create_session_request_allows_omitted_request_for_idle_create() {
    let request: CreateSessionRequest = serde_json::from_value(serde_json::json!({
        "assistantId": "assistant-1",
        "executionMode": "unsafe"
    }))
    .expect("request should be optional");

    assert_eq!(request.request, None);
    assert_eq!(request.execution_mode, Some(ExecutionMode::Unsafe));
}

#[test]
fn create_session_request_treats_blank_request_as_present_string() {
    let request: CreateSessionRequest = serde_json::from_value(serde_json::json!({
        "assistantId": "assistant-1",
        "request": "   "
    }))
    .expect("blank request should deserialize");

    assert_eq!(request.request.as_deref(), Some("   "));
}
