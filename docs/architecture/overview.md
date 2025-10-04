# SynapticFlow System Architecture

This document provides a high-level overview of the SynapticFlow architecture, detailing its main components and their interactions.

## Core Philosophy

The architecture is designed to be modular and extensible, separating concerns between the user interface, backend logic, and the AI agent capabilities provided by various tool services.

## Architectural Layers

SynapticFlow is composed of three primary layers:

1.  **Frontend Layer (React)**: The user-facing application built with React and TypeScript.
2.  **Backend Layer (Tauri/Rust)**: The native backend that manages system resources, provides native services, and communicates with external processes.
3.  **Service Layer (Tools & Capabilities)**: A flexible layer of tool providers that can be native Rust services, in-browser Web Worker services, or external processes adhering to the Model Context Protocol (MCP).

### 1. Frontend Layer (`src/`)

**Responsibilities**:

- **User Interface (UI)**: Renders the chat interface, settings modals, and all other visual components.
- **State Management**: Manages application state using React Context providers (`src/context/`). This includes session history, settings, and connected assistants.
- **Service Integration**: The frontend features a central service registry (`BuiltInToolProvider`) that dynamically collects and manages tools from all available services.
- **User Interaction**: Handles all user input, from sending messages to configuring agents.
- **Backend Communication**: Interacts with the Tauri backend via a dedicated hook (`useRustBackend`) and client functions that invoke Tauri commands.

**Key Components**:

- `App.tsx`: The main application component that wires up all the context providers.
- `BuiltInToolProvider`: The central React context that acts as a registry for all tool-providing services.
- `useBuiltInTool()`: The hook used by other providers to register their services.
- `Chat.tsx`: The core chat interface.
- `rust-backend-client.ts`: The client responsible for communicating with the Rust backend.

### 2. Backend Layer (`src-tauri/`)

**Responsibilities**:

- **Native Operations**: Provides access to the native file system, process management, and other system-level APIs.
- **Native Service Provider**: Implements and exposes "built-in" services (like `filesystem` and `sandbox`) that are executed directly within the Rust backend for performance and security.
- **External Process Management**: Manages the lifecycle of external MCP server processes (starting, stopping, and monitoring) via the `MCPServerManager`.
- **Security**: Enforces security policies, such as path validation for filesystem operations.

**Key Components**:

- `lib.rs`: Defines all the Tauri commands that bridge the frontend and backend.
- `mcp/`: Contains the logic for managing external MCP servers.
- `mcp/builtin/`: Contains the implementation of the native Rust services.

### 3. Service Layer (Tools & Capabilities)

This is not a single process, but a collection of services that provide tools to the AI agent. The frontend's `BuiltInToolProvider` makes these disparate sources appear as a unified set of tools.

**Types of Services**:

- **Native Rust Services**: High-performance, secure tools implemented in Rust and exposed via Tauri commands. Examples include `filesystem` and `code sandbox`. These are managed by the `RustMCPToolProvider` on the frontend.
- **Web Worker Services**: Tools that run in a separate browser thread, ideal for tasks like data processing or interacting with browser APIs without blocking the UI. Examples include `planning` and `playbook`. These are managed by the `WebMCPProvider`.
- **External MCP Servers**: Standalone processes that communicate with SynapticFlow over stdio, adhering to the Model Context Protocol (MCP). This allows third-party developers to integrate their own tools.

This layered and service-oriented architecture ensures that the UI remains decoupled from the underlying tool implementations, and that new capabilities can be added modularly by creating new services.
