use tauri_mcp_agent_lib::agent::llm::prompt::build_system_prompt;
use tauri_mcp_agent_lib::agent::AgentConfig;

#[tokio::test]
async fn system_prompt_exposes_agent_runtime_identity() {
    let config = AgentConfig {
        id: Some("agent-123".to_string()),
        name: "Ops Bot".to_string(),
        system_prompt: "You are a precise operator.".to_string(),
        ..AgentConfig::default()
    };

    let prompt = build_system_prompt(&config, None, None, None, Vec::new())
        .await
        .expect("prompt should build");

    assert!(prompt.contains("## Agent Runtime Identity"));
    assert!(prompt.contains("Agent Name: Ops Bot"));
    assert!(prompt.contains("Agent ID: agent-123"));
    assert!(prompt.contains("Session ID: (unknown-session)"));
}

#[tokio::test]
async fn system_prompt_preserves_full_parent_session_id_for_sub_agents() {
    let config = AgentConfig {
        id: Some("agent-child".to_string()),
        name: "Worker".to_string(),
        system_prompt: "You are a worker.".to_string(),
        parent_session_id: Some("a1b2c3d4e5".to_string()),
        depth: Some(1),
        ..AgentConfig::default()
    };

    let prompt = build_system_prompt(&config, None, None, None, Vec::new())
        .await
        .expect("prompt should build");

    assert!(prompt.contains("Parent Session: a1b2c3d4e5"));
    assert!(prompt.contains("Parent Session `a1b2c3d4e5`"));
    assert!(prompt.contains("return deliverables in your final text response"));
    assert!(prompt.contains("Scratchpad notes are private to this session only"));
}
