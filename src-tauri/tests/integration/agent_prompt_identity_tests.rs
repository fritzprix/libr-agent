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

    let prompt = build_system_prompt(&config, None, None, None, None, Vec::new())
        .await
        .expect("prompt should build");

    assert!(prompt.contains("## Agent Runtime Identity"));
    assert!(prompt.contains("Agent Name: Ops Bot"));
    assert!(prompt.contains("Agent ID: agent-123"));
}
