use async_trait::async_trait;
use std::sync::Arc;
use tauri_mcp_agent_lib::agent::context::{registry::ContextRegistry, ContextProvider};
use tauri_mcp_agent_lib::agent::llm::build_system_prompt;
use tauri_mcp_agent_lib::agent::AgentConfig;
use tauri_mcp_agent_lib::mcp::types::ContextVolatility;

struct StableProvider;

#[async_trait]
impl ContextProvider for StableProvider {
    fn provider_id(&self) -> &str {
        "stable_provider"
    }

    async fn get_context(&self, _assistant_id: Option<&str>) -> Result<String, String> {
        Ok("## Stable Provider\nStable provider content".to_string())
    }

    fn priority(&self) -> i32 {
        10
    }

    fn volatility(&self) -> ContextVolatility {
        ContextVolatility::Stable
    }
}

struct VolatileProvider;

#[async_trait]
impl ContextProvider for VolatileProvider {
    fn provider_id(&self) -> &str {
        "volatile_provider"
    }

    async fn get_context(&self, _assistant_id: Option<&str>) -> Result<String, String> {
        Ok("## Volatile Provider\nVolatile provider content".to_string())
    }

    fn priority(&self) -> i32 {
        20
    }

    fn volatility(&self) -> ContextVolatility {
        ContextVolatility::Volatile
    }
}

#[tokio::test]
async fn context_registry_splits_stable_and_volatile_provider_output() {
    let mut registry = ContextRegistry::new();
    registry.register(Box::new(StableProvider));
    registry.register(Box::new(VolatileProvider));

    let (stable, volatile) = registry.build_context_split(None).await;

    assert!(stable.contains("## Stable Provider"));
    assert!(!stable.contains("## Volatile Provider"));
    assert!(volatile.contains("## Volatile Provider"));
    assert!(!volatile.contains("## Stable Provider"));
}

#[tokio::test]
async fn build_system_prompt_keeps_stable_provider_content_ahead_of_volatile_content() {
    let agent_config = AgentConfig {
        system_prompt: "Base system prompt.".to_string(),
        ..Default::default()
    };

    let mut registry = ContextRegistry::new();
    registry.register(Box::new(StableProvider));
    registry.register(Box::new(VolatileProvider));

    let prompt = build_system_prompt(
        &agent_config,
        None,
        None,
        Some(Arc::new(registry)),
        None,
        vec![],
    )
    .await
    .expect("prompt should build");

    let stable_index = prompt
        .find("## Stable Provider")
        .expect("stable provider block should exist");
    let volatile_index = prompt
        .find("## Volatile Provider")
        .expect("volatile provider block should exist");

    assert!(stable_index < volatile_index);
}
