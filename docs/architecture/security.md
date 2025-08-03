# Security in SynapticFlow

This document outlines the key security considerations and practices for SynapticFlow.

## 1. API Key Management

-   **In-App Storage**: API keys for AI services are stored securely within the application's local data, managed via the settings UI. They are not stored in plaintext files or environment variables in production builds.
-   **No Version Control**: API keys should never be committed to version control.

## 2. Tauri Security

-   **Allowlist**: The `tauri.conf.json` file should be configured with a strict allowlist to prevent unauthorized access to system resources.
-   **Input Validation**: All data passed from the frontend to the backend via Tauri commands is validated in the Rust code to prevent injection attacks or unexpected behavior.

## 3. MCP Server Sandboxing

-   **Process Isolation**: MCP servers are run as separate processes, isolating them from the main application.
-   **Permissions**: Be cautious about the permissions granted to MCP servers. For example, a filesystem server should be restricted to a specific directory if possible.

## 4. Code Security

-   **Dependency Audits**: Regularly audit dependencies for known vulnerabilities.
-   **Strict Typing**: The use of TypeScript and Rust helps prevent many common security vulnerabilities, such as type confusion.
