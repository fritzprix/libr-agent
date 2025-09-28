# 🚀 SynapticFlow Project Guidelines

## 1. Project Overview

**SynapticFlow: A High-Freedom AI Agent Platform - Infinitely Expandable with MCP!**

SynapticFlow is a next-generation desktop AI agent platform that combines the lightness of Tauri with the intuitiveness of React. Users can automate daily tasks by giving AI agents their own unique personalities and abilities through a powerful and extensible tool ecosystem.

## 2. Key Features

- **Agent/Role Management:** Create, edit, and delete various AI agent roles with custom system prompts to define unique personalities.
- **Dual MCP Backend:** A hybrid system combining a native Rust backend for performance-critical tasks and a Web Worker-based backend for lightweight, dependency-free tools.
- **Unified Tool System:** A seamless interface for the AI to access both native and web-based tools, including file management, code execution, and browser automation.
- **Advanced Chat Interface:** A feature-rich chat experience with support for streaming responses, tool calls, and rich content rendering.
- **Centralized Configuration:** All settings, including API keys, models, and UI preferences, are managed and stored securely within the application.
- **High-Performance & Secure:** Built on the Tauri framework for a fast, secure, and cross-platform desktop experience.

## 3. Technology Stack

- **Core Frameworks:**
  - **Tauri:** 2.x (Rust + WebView)
  - **React:** 18.3.1
  - **TypeScript:** 5.6.2
- **Backend & Tooling:**
  - **RMCP (Rust Model Context Protocol):** 0.6.4
  - **Tokio:** 1.0 (Async Rust runtime)
  - **Serde:** 1.0 (Rust serialization/deserialization)
- **Frontend & UI:**
  - **Vite:** 6.0.3
  - **Tailwind CSS:** 4.1.11
  - **shadcn/ui:** Component library
- **Database:**
  - **IndexedDB:** For local storage of roles and conversations.

## 4. Architecture

SynapticFlow's architecture is designed for modularity, performance, and extensibility. It is composed of a React frontend, a Tauri Rust backend, and a unique dual MCP (Model Context Protocol) system.

### A. MCP Integration: The Dual Backend

The core of SynapticFlow's extensibility lies in its dual MCP backend, which allows for two types of tool servers to run concurrently:

1. **Rust Tauri Backend (High-Performance Native Tools):**
   - Managed by `MCPServerManager` in the Rust backend (`src-tauri/src/mcp/server.rs`).
   - Ideal for performance-intensive or security-sensitive tasks that require native system access (e.g., file system operations, code execution).
   - Servers are spawned as `stdio`-based child processes and communicate with the Rust backend via the RMCP protocol.

2. **Web Worker Backend (Lightweight & Dependency-Free Tools):**
   - Runs entirely within the browser via a Web Worker (`src/lib/web-mcp/mcp-worker.ts`).
   - Perfect for tools written in TypeScript that do not require native system access (e.g., calculators, data transformers, API clients).
   - Managed by `WebMCPProvider` and communicates with the main thread via `mcp-proxy.ts`.

**Unified Access:** The frontend interacts with both backends through a unified client, `rust-backend-client.ts`, which uses Tauri commands to communicate with the Rust `MCPServerManager`. The `MCPServerManager` is responsible for routing tool calls to the appropriate backend (native or Web Worker), making the distinction transparent to the AI agent.

### B. Chat Feature: A Modern, Composable System

The chat feature is built using a modern, composable architecture that ensures a high degree of maintainability and flexibility.

- **Compound Components:** The UI is structured using a compound component pattern (e.g., `Chat.Header`, `Chat.Messages`, `Chat.Input`), which are assembled from individual components in `src/features/chat/components/`. This provides a clean and declarative API for building the chat interface.
- **State Management (`ChatProvider`):** All chat-related state is managed within `ChatProvider` (`src/context/ChatContext.tsx`). This includes message history, loading states, and tool execution status, providing a single source of truth for the entire chat feature.
- **Service Layer (`useAIService` & `AIServiceFactory`):** AI model interactions are abstracted into a dedicated service layer. The `useAIService` hook (`src/hooks/use-ai-service.ts`) provides a simple interface for submitting prompts and receiving streaming responses. The `AIServiceFactory` (`src/lib/ai-service/factory.ts`) dynamically selects the appropriate AI provider (e.g., OpenAI, Anthropic) based on user settings.
- **Tool Call Flow:** When the AI model requests a tool call, the `useToolProcessor` hook (`src/hooks/use-tool-processor.ts`) intercepts the request, uses the unified MCP client to execute the tool, and submits the result back to the AI.

For a more detailed breakdown, see the [Chat Feature Architecture Document](docs/architecture/chat-feature-architecture.md).

### C. Tool Ecosystem & Agent System

SynapticFlow's power comes from its extensible tool ecosystem and agent management system.

- **Built-in Tools:** The platform includes a set of powerful built-in tools, such as:
  - **Secure File Manager:** For reading, writing, and listing files within a sandboxed workspace.
  - **Code Execution:** For running scripts and commands in a secure environment.
  - **Browser Automation:** For controlling a headless browser to perform web-based tasks.
  - Tool logic can be found in `src/features/tools/` and `src-tauri/src/mcp/builtin/`.
- **Agent Management:** The agent/role management system (`src/features/agents/`) allows users to create and customize agents with specific system prompts and toolsets. These agents are then seamlessly integrated into the chat feature, allowing the user to switch between different AI personalities and capabilities on the fly.

## 5. Coding Rules & Style

To maintain code quality and consistency, all contributors must adhere to the following rules:

- **Centralized Logging:** Always use the centralized logger (`getLogger` from `@/lib/logger`) instead of `console.log`. This provides structured, context-aware logging that integrates with Tauri's native logging capabilities.
- **Strict TypeScript:** The use of `any` is strictly prohibited. Always use specific types or, if necessary, `unknown` with proper type guards. Do not disable linting rules.
- **No Inline `import()` Types:** Use standard `import type` statements at the top of the file for better readability and maintainability.
- **Follow Linter and Formatter:** All code must pass the ESLint and Prettier checks defined in the project. Run `pnpm refactor:validate` before submitting changes.

### Rust Method/Function Declaration and Calling Guide

#### Method vs. Associated Function

- **Method**: Takes `self` (or `&self`, `&mut self`) as the first parameter in an `impl` block.  
  → Called through instance: `self.method_name(...)`
- **Associated Function**: No `self` parameter.  
  → Called through type name: `TypeName::function_name(...)`

#### Example

```rust
impl MyStruct {
    // Method: requires self
    fn do_something(&self, arg: i32) { ... }

    // Associated function: no self
    fn helper(arg: i32) { ... }
}

// Calling methods
let obj = MyStruct::new();
obj.do_something(42);           // ✅ Method call
MyStruct::helper(42);           // ✅ Associated function call
```

#### Error Prevention Checklist

- If using `self` in a function, declare `self` as the first parameter.
- Associated functions cannot use `self`.
- Call methods through instances, associated functions through type names.

#### Common Mistakes and Fixes

##### ❌ Wrong Example

```rust
fn copy_dir_contents(src: &Path, dst: &Path) -> Result<(), String> {
    self.copy_dir_contents(&src_path, &dst_path)?; // Error!
}
```

##### ✅ Correct Examples

- **If declared as associated function, call through type name:**

```rust
fn copy_dir_contents(src: &Path, dst: &Path) -> Result<(), String> {
    SessionManager::copy_dir_contents(&src_path, &dst_path)?;
}
```

- **If using as method, add self parameter:**

```rust
fn copy_dir_contents(&self, src: &Path, dst: &Path) -> Result<(), String> {
    self.copy_dir_contents(&src_path, &dst_path)?;
}
```

#### IDE/Compiler Usage

- Rust compiler clearly indicates these mistakes, so read error messages carefully and check function declarations/calls.
- Use "Go to Definition" in IDEs like VS Code or IntelliJ Rust to easily check if a function is a method or associated function.

**Summary:** Always remember that the presence/absence of `self` parameter determines calling method. When compilation errors occur, recheck function declaration and calling patterns.

For more details, refer to the [Copilot Instructions](.github/copilot-instructions.md).

## 6. Development Workflow

1. **Install Dependencies:** `pnpm install`
2. **Run Development Server:** `pnpm tauri dev`
3. **Validate Changes:** `pnpm refactor:validate` (run this before committing)
4. **Build for Production:** `pnpm tauri build`

## 7. References & Documentation

- **Architecture:**
  - [Chat Feature Architecture](docs/architecture/chat-feature-architecture.md)
- **Implementation Files:**
  - **MCP:**
    - [MCPServerManager (Rust)](src-tauri/src/mcp/server.rs)
    - [WebMCPProvider (TypeScript)](src/lib/web-mcp/)
    - [Unified MCP Client](src/lib/rust-backend-client.ts)
  - **Chat:**
    - [ChatProvider](src/context/ChatContext.tsx)
    - [useAIService Hook](src/hooks/use-ai-service.ts)
    - [Tool Processor Hook](src/hooks/use-tool-processor.ts)
