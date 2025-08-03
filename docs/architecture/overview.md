# SynapticFlow System Architecture

This document provides a high-level overview of the SynapticFlow architecture, detailing its main components and their interactions.

## Core Philosophy

The architecture is designed to be modular and extensible, separating concerns between the user interface, backend logic, and the AI agent capabilities provided by the Model Context Protocol (MCP).

## Architectural Layers

SynapticFlow is composed of three primary layers:

1.  **Frontend Layer (React)**: The user-facing application built with React and TypeScript.
2.  **Backend Layer (Tauri/Rust)**: The native backend that manages system resources, processes, and communication.
3.  **MCP Layer (External Processes)**: External MCP servers that provide tools and capabilities to the AI agents.

### 1. Frontend Layer (`src/`)

**Responsibilities**:

-   **User Interface (UI)**: Renders the chat interface, settings modals, and all other visual components.
-   **State Management**: Manages the application state using React Context providers (`src/context/`). This includes session history, settings, and connected assistants.
-   **User Interaction**: Handles all user input, from sending messages to configuring agents.
-   **Tauri Communication**: Interacts with the backend via a dedicated service layer (`src/lib/`) that invokes Tauri commands.

**Key Components**:

-   `App.tsx`: The main application component.
-   `Chat.tsx`: The core chat interface.
-   `AssistantEditor.tsx`: The UI for creating and editing AI agents.
-   `SettingsModal.tsx`: The UI for managing application settings.
-   `tauri-mcp-client.ts`: The client responsible for communicating with the Rust backend.

### 2. Backend Layer (`src-tauri/`)

**Responsibilities**:

-   **Native Operations**: Provides access to the native file system, process management, and other system-level APIs.
-   **MCP Server Management**: Manages the lifecycle of MCP server processes (starting, stopping, and monitoring) via the `MCPServerManager`.
-   **Tool Dispatch**: Receives tool call requests from the frontend, forwards them to the appropriate MCP server, and returns the results.
-   **Security**: Enforces security policies and manages access to sensitive resources.

**Key Components**:

-   `lib.rs`: Defines all the Tauri commands that bridge the frontend and backend.
-   `mcp.rs`: Contains the logic for managing MCP servers (`MCPServerManager`) and data structures (`MCPTool`, `MCPServerConfig`).

### 3. MCP Layer (External)

**Responsibilities**:

-   **Providing Tools**: Exposes a set of capabilities (tools) that AI agents can use.
-   **Executing Logic**: Runs the actual tool logic (e.g., reading a file, searching the web).
-   **Adhering to the Protocol**: Communicates with the SynapticFlow backend using the Model Context Protocol (MCP) over stdio, HTTP, or WebSockets.

This layered architecture ensures that the UI remains decoupled from the underlying system operations, and the tool capabilities can be expanded independently by developing new MCP servers.
