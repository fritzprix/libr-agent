use tauri_mcp_agent_lib::agent::types::CreateSessionRequest;
use tauri_mcp_agent_lib::repositories::{SessionMetadata, SessionStatus};
use tauri_mcp_agent_lib::services::agent_service::resolve_child_session_model_provider;

fn build_parent_session(model: &str, provider: &str) -> SessionMetadata {
    let now = chrono::Utc::now().timestamp_millis();
    SessionMetadata {
        id: "parent-session".to_string(),
        name: Some("Parent".to_string()),
        status: SessionStatus::Idle,
        model: model.to_string(),
        provider: provider.to_string(),
        agent_config: Some(r#"{"name":"Parent"}"#.to_string()),
        parent_session_id: None,
        lineage_id: Some("parent-session".to_string()),
        depth: Some(0),
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        created_at: now,
        updated_at: now,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        yolo_mode: false,
        workspace_override: None,
    }
}

#[test]
fn child_sessions_inherit_parent_model_and_provider_by_default() {
    let parent = build_parent_session("gpt-5.4", "openai");

    let (model, provider) = resolve_child_session_model_provider(None, None, Some(&parent));

    assert_eq!(model.as_deref(), Some("gpt-5.4"));
    assert_eq!(provider.as_deref(), Some("openai"));
}

#[test]
fn child_sessions_allow_explicit_override_without_losing_unspecified_parent_field() {
    let parent = build_parent_session("gpt-5.4", "openai");

    let (model, provider) = resolve_child_session_model_provider(
        Some("claude-sonnet-4.5".to_string()),
        None,
        Some(&parent),
    );

    assert_eq!(model.as_deref(), Some("claude-sonnet-4.5"));
    assert_eq!(provider.as_deref(), Some("openai"));
}

#[test]
fn create_session_request_deserializes_optional_model_and_provider() {
    let request: CreateSessionRequest = serde_json::from_value(serde_json::json!({
        "assistantId": "assistant-1",
        "request": "delegate this",
        "model": "claude-sonnet-4.5",
        "provider": "anthropic"
    }))
    .expect("request should deserialize");

    assert_eq!(request.model.as_deref(), Some("claude-sonnet-4.5"));
    assert_eq!(request.provider.as_deref(), Some("anthropic"));
}
