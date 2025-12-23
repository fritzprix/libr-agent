# Thronglet Architecture

This document details the internal design and architectural decisions of **Thronglet**, the Rust-based Agent Runtime.

## 1. System Overview

Thronglet acts as a **Stateful, Event-Driven Control Loop** that manages the lifecycle of an autonomous AI agent. It is designed to be **host-agnostic**, meaning it does not depend on Tauri, HTTP, or any specific transport layer. Instead, it relies on strict traits (`LLMProvider`, `ToolProvider`) to interact with the outside world.

### Component Diagram

```mermaid
graph TD
    Host[Host Application (LibrAgent)] -->|Input/Commands| Agent
    Agent -->|Events/State| Host
    
    subgraph Thronglet ["Thronglet (Crate)"]
        Agent[Agent Struct]
        State[Conversation History]
        Loop[Think-Act-Observe Loop]
        
        Agent -->|Owns| State
        Agent -->|Runs| Loop
        
        Loop -->|Generate| LLM[LLMProvider Trait]
        Loop -->|Execute| Tools[ToolProvider Trait]
    end
    
    LLM -.->|Impl| RemoteLLM[RemoteLLMProvider (Host Bridge)]
    Tools -.->|Impl| MCPManager[WrappedMcpManager (Host Bridge)]
```

## 2. Core Components

### 2.1 The Agent Struct (`Agent<L, T>`)

The `Agent` struct is the entry point. It is generic over two types:
- `L: LLMProvider`: The logic for generating text/tool calls.
- `T: ToolProvider`: The logic for executing tools.

```rust
pub struct Agent<L, T> {
    config: AgentConfig,
    llm_provider: Arc<L>,
    tool_provider: Arc<T>,
    history: Vec<Message>,
}
```

**Key Responsibilities:**
- **State Management**: Appending user inputs, storing tool results.
- **Loop Orchestration**: Calling `run_loop()` to drive the agent until it stops (yields a final answer) or errors.

### 2.2 The Control Loop (`run_loop`)

The core of Thronglet is the `run_loop`. It implements a **Recursive ReAct Pattern**:

1.  **Think (Generate)**: 
    - The Agent sends the full conversation history to `LLMProvider::generate`.
    - It receives an `LLMResponse` containing either text, tool calls, or both.
2.  **Act (Process)**:
    - If the response has `tool_calls`, the agent iterates through them.
    - It routes each call to `ToolProvider::call_tool`.
3.  **Observe (Feedback)**:
    - The result of the tool execution (`ToolResult`) is converted into a `Message::Tool` message.
    - This message is appended to the history.
4.  **Loop**:
    - If tool calls were executed, the loop repeats (Step 1) with the new history.
    - If no tool calls were generated (or explicit stop signal), the loop terminates and returns the final text.

## 3. Interfaces (Traits)

Thronglet enforces a clean separation of concerns via traits.

### 3.1 LLMProvider

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

**Design Rationale:**
- **Stateless by default**: The provider conceptually just "completes" the history. State is held by the `Agent`.
- **Async**: Generation is inherently blocking/long-running.
- **Provider Agnostic**: It works with OpenAI, Anthropic, or even a local mock.

### 3.2 ToolProvider

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

**Design Rationale:**
- **Unified Interface**: Regardless of whether tools are local binaries, HTTP endpoints, or browser scripts, the Agent sees a uniform `call_tool` interface.
- **Routing**: The `server_name` parameter allows routing to specific sub-systems (e.g., "browser", "filesystem").

## 4. Integration with LibrAgent (Host)

LibrAgent embeds Thronglet and provides the concrete implementations for the traits.

### 4.1 Remote LLM Execution (The Bridge)

Since LibrAgent's UI holds the API keys and configuration, the Rust backend **does not** call OpenAI directly. Instead, it uses a **Reverse Bridge**:

1.  **Rust**: `RemoteLLMProvider` creates a unique `request_id`.
2.  **Rust**: Emits a Tauri Event `agent://llm_request` with the history and `request_id`.
3.  **Frontend**: Listens to the event.
4.  **Frontend**: Calls `AIService` (OpenAI/Anthropic SDK) using user's keys.
5.  **Frontend**: Streams the response to the UI (for ephemeral feedback).
6.  **Frontend**: Invokes Rust command `agent_llm_response` with the final result.
7.  **Rust**: The `RemoteLLMProvider` (which was `await`ing on a channel) receives the result and returns it to the Agent.

This design ensures:
- **Security**: API keys never need to be stored in the Rust backend persistence if not desired.
- **UI Responsiveness**: The UI drives the generation visualization.

### 4.2 Tool Execution (Wrapped MCP)

LibrAgent's `MCPServerManager` is already a sophisticated tool orchestrator. Thronglet integrates it easily:

- `WrappedMcpManager` struct wraps a reference to `MCPServerManager`.
- It implements `ToolProvider`.
- When Thronglet calls `call_tool`, it simply delegates to `MCPServerManager::call_tool_unified`.

## 5. Error Handling

Thronglet uses a unified `AgentError` enum:

- `LLMError`: Issues with the generation provider (timeout, network, invalid format).
- `ToolError`: Issues with tool execution (not found, runtime error).
- `InternalError`: Logic bugs or bad state.

Errors in the loop are generally **recoverable** by the LLM (e.g., if a tool fails, the error message is fed back to the LLM so it can retry), but `System Errors` (like channel closing) propagate up to stop the agent.
