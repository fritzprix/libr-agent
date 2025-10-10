# SynapticFlow — App Architecture & Runtime Flow

This document summarizes the frontend (`src/`) and backend (`src-tauri/`) structure and explains how the application initializes and routes requests at runtime. It focuses on the service-oriented architecture for tool integration.

It is meant as a guide for contributors and maintainers to understand the main responsibilities, feature wiring, and where to look for implementation details.

---

## 1. High-level Overview

- **Frontend**: `src/` — React + TypeScript (Vite), UI components, feature modules, and a central service registry for tool integration.
- **Backend**: `src-tauri/` — Rust (Tauri) code that provides native services (e.g., filesystem, sandbox), manages external MCP processes, and exposes Tauri commands.

The app's architecture is centered around a **service-oriented model** for integrating tools. The frontend's `BuiltInToolProvider` acts as a central registry where different "services" can register their tools. This makes the system highly modular and extensible.

There are three main types of services:

1. **Native Rust Services**: Implemented in the Tauri backend for high performance and access to native capabilities (e.g., `filesystem`, `sandbox`).
2. **Web Worker Services**: Run in a separate browser thread, ideal for browser-based tools or intensive tasks that shouldn't block the UI (e.g., `planning`).
3. **External MCP Servers**: Standalone processes that communicate via stdio, managed by the Rust backend.

---

## 2. Project Layout (Important Folders)

- `src/`
  - `app/` — App entry and layout (`App.tsx`, `main.tsx`).
  - `components/` — Reusable UI components.
  - `context/` — Core React contexts (Session, Assistant, Settings, etc.).
  - `features/` — Feature folders, including the core **`tools`** feature which contains the service architecture.
    - `features/tools/` — Contains the `BuiltInToolProvider` (`index.tsx`), specialized providers (`RustMCPToolProvider.tsx`, `BrowserToolProvider.tsx`), and the `useServiceContext` hook.
  - `hooks/` — Reusable hooks (e.g., `use-rust-backend`).
  - `lib/` — Service layer, utilities, `rust-backend-client.ts`, MCP types.
  - `models/` — TypeScript models (chat messages, attachments).

- `src-tauri/`
  - `src/` — Tauri Rust source (commands, MCP manager).
  - `src/mcp/builtin/` — Implementations of the native Rust services (e.g., `filesystem.rs`, `sandbox.rs`).
  - `Cargo.toml`, `tauri.conf.json` — Tauri/Rust config and dependencies.

---

## 3. App Initialization & Provider Wiring (`src/app/App.tsx`)

The top-level `App` component composes the application providers. The order is critical. The key change from the previous architecture is that `BuiltInToolProvider` now wraps the specialized tool providers.

Key providers related to the service architecture:

- **`BuiltInToolProvider`**: The central service registry. It does not provide tools itself but collects them from other providers.
- **`WebMCPProvider`**: Manages Web Worker-based services.
  - **`WebMCPServiceRegistry`**: Registers specific Web Worker services like `planning` and `playbook`.
- **`BrowserToolProvider`**: Registers services that interact with the browser environment.
- **`RustMCPToolProvider`**: Registers the native services provided by the Rust backend.

These providers are nested within `BuiltInToolProvider` and use its `register` function to make their tools available to the rest of the application.

---

## 4. Core Concept: The Service Architecture

The tool system was refactored from a monolithic `use-unified-mcp` hook to a modular, service-based architecture. The core of this system is in `src/features/tools/`.

### The `BuiltInToolProvider`

Found in `src/features/tools/index.tsx`, this provider is the heart of the tool system. It maintains a registry of all available "services" and their tools. It exposes functions to `register`, `unregister`, and `executeTool`. It is also responsible for dynamically generating the part of the system prompt that lists available tools and service contexts.

### The `BuiltInService` Interface

This TypeScript interface (`src/features/tools/index.tsx`) defines the contract that every service must adhere to. This ensures all tool providers behave consistently.

```typescript
export interface BuiltInService {
  metadata: ServiceMetadata;
  listTools: () => MCPTool[];
  executeTool: (toolCall: ToolCall) => Promise<MCPResponse<unknown>>;
  getServiceContext: (options) => Promise<ServiceContext<unknown>>;
  switchContext: (options) => Promise<void>;
  // ... and optional load/unload methods
}
```

- `getServiceContext` is particularly important, as it allows each service to contribute dynamic information to the AI's system prompt (e.g., the current file open in the editor, the current web page content).

### Service Registration

Specialized providers like `RustMCPToolProvider` and `BrowserToolProvider` are simple React components. On mount, they:

1. Call the `useBuiltInTool()` hook to get the `register` function.
2. Fetch their own tools (e.g., by calling a Tauri command to list native services).
3. For each service they manage, they call `register(serviceId, serviceImplementation)`, where `serviceImplementation` is an object that conforms to the `BuiltInService` interface.

This pattern makes it easy to add new sources of tools by simply creating a new provider component and mounting it within `BuiltInToolProvider`.

---

## 5. Typical Runtime Flows

### 1. App Start

- `App.tsx` mounts the providers.
- `BuiltInToolProvider` initializes its empty service registry.
- `RustMCPToolProvider` mounts, calls the `listBuiltinServers` Tauri command, and then `register`s the `filesystem` and `sandbox` services.
- `BrowserToolProvider` and `WebMCPServiceRegistry` mount and register their respective services.
- The UI is now aware of all tools from all services.

### 2. User Triggers a Tool Call

- The chat UI gets a tool call request from the AI, e.g., `builtin_filesystem__read_file`.
- The UI calls `executeTool` from the `useBuiltInTool` hook.
- `BuiltInToolProvider` parses the tool name. It extracts the service ID (`filesystem`) and the tool name (`read_file`).
- It looks up the `filesystem` service in its registry and calls the `executeTool` method on that service object.
- The `filesystem` service's implementation (defined in `RustMCPToolProvider`) then calls the actual Tauri command `callBuiltinTool` to execute the logic in the Rust backend.
- The result is returned up the chain.

### 3. Session Changes

- The user switches to a different chat session.
- The `useEffect` in `BuiltInToolProvider` detects the change in `currentSession`.
- It iterates through all registered services and calls the `switchContext({ sessionId })` method on each one.
- This allows services to perform cleanup or load session-specific data (e.g., the `workspace` service can load the files associated with the new session).

---

## 6. Where to look for implementation details

- **Core Service Architecture**: `src/features/tools/index.tsx` (see `BuiltInToolProvider` and the `BuiltInService` interface).
- **Native Rust Service Integration**: `src/features/tools/RustMCPToolProvider.tsx` and the `useRustBackend` hook.
- **Browser Service Integration**: `src/features/tools/BrowserToolProvider.tsx`.
- **Web Worker Service Integration**: `src/context/WebMCPContext.tsx` and `src/features/tools/WebMCPServiceRegistry.tsx`.
- **Rust Native Service Implementations**: `src-tauri/src/mcp/builtin/`.
