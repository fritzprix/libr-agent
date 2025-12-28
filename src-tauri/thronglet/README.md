# Thronglet 🤖

**Thronglet** is a lightweight, asynchronous, and trait-based AI Agent runtime written in Rust. It is designed to be the core "brain" of **LibrAgent**, providing a flexible architecture for building autonomous agents that can "Think" (LLM generation), "Act" (Tool execution), and "Observe" (Process results).

## 🌟 Core Philosophy

Thronglet follows a clean **dependency injection** pattern. The core `Agent` struct does not know *how* to call an LLM or *how* to execute tools; it only knows *that* it can via traits. This allows for:

1.  **Flexibility**: Swap out LLM backends (Direct API, Proxy, or Frontend-delegated) easily.
2.  **Testability**: Mock providers allow for deterministic testing of agent logic without real API calls.
3.  **Portability**: Can be used in CLI tools, server-side services, or desktop apps (like LibrAgent).

## 🏗 Architecture

The architecture revolves around three main components:

### 1. The Agent (`Agent<L, T>`)
The central state machine. It manages:
- **Conversation History**: Maintains the sequence of User, Assistant, and Tool messages.
- **The Loop**: Implements the recursive **Think-Act-Observe** loop.
- **State**: Holds configuration (System Prompt, etc.).

### 2. LLMProvider Trait (`traits::LLMProvider`)
Abstracts the interface to the Language Model.
```rust
#[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn generate(
        &self,
        history: Vec<Message>,
        system_prompt: String,
    ) -> Result<LLMResponse, AgentError>;
}
```

### 3. ToolProvider Trait (`traits::ToolProvider`)
Abstracts the interface to the Tool/MCP ecosystem.
```rust
#[async_trait]
pub trait ToolProvider: Send + Sync {
    async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<ToolResult, AgentError>;

    async fn list_tools(&self, server_names: Vec<String>) -> Result<Vec<ToolDefinition>, AgentError>;
}
```

---

## 🚀 Integration Example: LibrAgent

In **LibrAgent**, Thronglet is integrated by bridging the Rust backend to the existing Tauri/Frontend services.

### 1. RemoteLLMProvider (Rust -> Frontend Bridge)
Since the Frontend (`use-ai-service.ts`) holds the API keys and configuration, the Rust backend delegates LLM requests to the Frontend via Tauri Events.

```rust
// pseudo-code
struct RemoteLLMProvider { app_handle: AppHandle }

impl LLMProvider for RemoteLLMProvider {
    async fn generate(&self, history: Vec<Message>, ...) -> Result<LLMResponse, ...> {
        // 1. Emit event 'agent://llm_request' to Frontend
        self.app_handle.emit("agent://llm_request", payload)?;
        
        // 2. Wait for response via a one-shot channel
        // (The frontend calls 'agent_llm_response' command to fulfill this)
        rx.await?
    }
}
```

### 2. WrappedMcpManager (Rust -> Rust Adapter)
LibrAgent has a powerful `MCPServerManager`. We wrap it to implement `ToolProvider`.

```rust
struct WrappedMcpManager { inner: &'static MCPServerManager }

impl ToolProvider for WrappedMcpManager {
    async fn call_tool(&self, server, tool, args) -> Result<ToolResult, ...> {
        // Delegate to the existing unified MCP manager
        self.inner.call_tool_unified(server, tool, args, None).await
    }
}
```

### 3. Instantiating the Agent
```rust
pub async fn agent_start(...) {
    // 1. Create Providers
    let llm_provider = Arc::new(RemoteLLMProvider::new(...));
    let tool_provider = Arc::new(WrappedMcpManager::new(...));

    // 2. Create Agent
    let mut agent = Agent::new(config, llm_provider, tool_provider);

    // 3. Receive Input & Run
    agent.input("Hello, can you check the weather?").await?;
}
```

## 📦 Directory Structure

- `src/agent.rs`: The main `Agent` struct and loop logic.
- `src/traits.rs`: `LLMProvider` and `ToolProvider` definitions.
- `src/models.rs`: Data structures (Message, Content, ToolCall, etc.).
- `src/mock.rs`: Mock implementations for testing.
- `src/error.rs`: Custom error types.

## 🛠 Testing

Thronglet includes a comprehensive `MockLLMProvider` and `MockToolProvider` in `src/mock.rs`. This allows you to script entire conversations and verify agent behavior without touching the network.

```rust
let mock_llm = MockLLMProvider::new();
mock_llm.push_response("Calculate", response_with_tool_call);
mock_llm.push_response("TOOL_RESULT", final_answer_response);

let agent = Agent::new(config, Arc::new(mock_llm), ...);
agent.input("Calculate 2+2").await?;
```
