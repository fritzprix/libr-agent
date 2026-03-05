# Built-in Tools Design Principles

## Overview

This document outlines the core design principles and anti-patterns to avoid when creating or refactoring built-in MCP tools in LibrAgent. These principles are derived from real-world refactoring sessions and aim to maximize LLM efficiency, system robustness, and code maintainability.

## 1. Context Economy & Cognitive Load

AI agents operate within strict context limits and can suffer from "cognitive overload" when presented with too many options or overly verbose descriptions.

- **Hide Internal Callbacks:** Never expose tools to the LLM that are meant strictly for internal system use or frontend UI callbacks (e.g., `resumeFromWait`, `getUserAnswer`). The backend router can handle these internally without polluting the `all_tools()` export.
- **Concise Descriptions:** Tool descriptions should be brief and focused on _when_ and _how_ to use the tool. Avoid repeating information already present in the JSON schema properties.

* **Smart Defaults over Verbosity:** Do not provide paragraphs of error-handling advice or prerequisites in the tool description. Let the tool fail gracefully with a descriptive error message and guidance string instead.

- **Progressive Disclosure:** For tools like `listTools`, provide a compact summary when queried broadly, and detailed schemas only when a specific query is provided.

## 2. Consolidation & Auto-Switching

Reduce the sheer number of tools by combining highly similar operations into a single tool with smart, data-driven behavior.

- **Merge Sibling Tools:** If two tools achieve the exact same goal but differ only in input type (e.g., `exportFile` for a single file vs. `exportZip` for an array of files), merge them into a single `export` tool.
- **Data-Driven Logic:** Let the backend determine the execution mode. For example, if an array contains one file, export raw; if multiple files or a directory, compress to ZIP. The LLM shouldn't have to make micro-decisions about file counts.

## 3. Strict Synchronization & State Integrity

Frontend state and asynchronous background tasks should never govern critical data integrity.

- **Synchronous Verification:** Whenever a tool or configuration is created/updated (e.g., registering an MCP server), the system must verify its validity synchronously _before_ committing it to the database.
- **Backend as Source of Truth:** Do not rely on the frontend to fire background validation events (e.g., `probe_mcp_server`) after a save. If an entity is in the database, it must be fully verified and operational.
- **No "Ghost" Entities:** If a configuration (like an external MCP server) lacks a cache or metadata due to a transient failure, it must still be visible in tool listings (with an appropriate warning). Never silently filter out registered entities just because their secondary state is unpopulated.

## 4. Don't Repeat Yourself (DRY) in Integration

Built-in tools should leverage the system's central architecture rather than re-inventing wheels.

- **Use Central Managers:** If the system provides an `MCPServerManager` for spawning and communicating with servers, do not write raw `tokio::process::Command` or `reqwest::Client` logic inside a specific tool's handler. Use the central manager.
- **Single Source of Validation:** Formulate validation logic (e.g., `verify_config`) in the core service layer and reuse it across UI endpoints, tool handlers, and agent routines.

## 5. Cross-Platform UX Considerations

Tools that interact with the host OS must provide a seamless experience across platforms.

- **Process Stealth:** On Windows, always apply the `CREATE_NO_WINDOW` flag (`0x08000000`) when spawning background or verification processes via `std::os::windows::process::CommandExt` to prevent distracting terminal flashes.
