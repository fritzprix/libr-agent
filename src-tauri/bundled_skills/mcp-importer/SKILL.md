---
name: mcp-importer
description: Imports and synchronizes existing MCP (Model Context Protocol) configurations from other AI editors and agents (Cursor, VS Code, Windsurf, etc.) into LibrAgent or Claude Desktop. Use when users want to "sync", "import", or "bring settings" from their existing tools.
---

# MCP Importer

This skill specializes in migrating existing MCP server configurations from other environments into the current LibrAgent session or Claude Desktop.

## Triggering

Trigger this skill when a user asks to:
- "Import settings from Cursor/VS Code/etc."
- "Sync my MCP servers"
- "See what MCP servers are installed in other apps"
- "Bring my Cline/Roo Code tools here"

## 🚀 Workflow

### 1. Identify Sources
Consult [app_paths.md](references/app_paths.md) to locate configuration files for:
- Cursor (Global/Project)
- VS Code (Cline, Roo Code, Copilot)
- Claude Code
- Windsurf
- Zed

### 2. Scan and Compare
1. Read the detected configuration files.
2. List all found MCP servers.
3. Compare with currently registered tools in LibrAgent (use `tool__list`).
4. Identify "New" servers (not yet in LibrAgent).

### 3. Interactive Import
1. Present the list of found servers to the user.
2. Ask which ones to import (All or specific ones).
3. Use `tool__register` for each selected server.
4. (Optional) Offer to update `claude_desktop_config.json` as well.

## 🛠 Guidelines

- **Duplicate Prevention**: Do not register servers that already exist in LibrAgent with the same name and command.
- **Path Resolution**: Resolve environment variables like `%APPDATA%` or `%USERPROFILE%` before reading.
- **Workspace Scope**: If a project-level config (e.g., `.cursor/mcp.json`) is found in the current workspace, prioritize it.

## Reference
- [app_paths.md](references/app_paths.md): List of supported apps and their config locations.
