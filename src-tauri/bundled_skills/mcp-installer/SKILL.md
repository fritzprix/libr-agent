---
name: mcp-installer
description: Automates the registration of new MCP (Model Context Protocol) servers in LibrAgent or Claude Desktop. Use when users share new MCP connection info (npm/npx package, GitHub URL, or config JSON) and want to register it as an active tool.
---

# MCP Installer

This skill helps you register MCP servers into the current LibrAgent session or the user's Claude Desktop configuration.

## Triggering

Trigger this skill when a user provides:
- An npm package name (e.g., `@org/package`)
- A GitHub repository URL (e.g., `https://github.com/user/repo`)
- A JSON configuration block for an MCP server
- A request to "import" or "sync" MCP settings from another app (e.g., Cursor, VS Code)

## 🚀 Workflow

### 1. Analyze Information
Identify the source and transport details.
- **npx/npm**: Use `npx -y <package>` for registration.
- **GitHub**: Determine if it's TypeScript (needs build) or Python (can use `uvx`).
- **Claude Config**: Extract command, args, and env variables.

### 2. Implementation: LibrAgent (Current Session)
Use the `tool__register` tool to add the MCP server.

**Example Task:**
"Register the 'everything' MCP server using npx."

**Execution:**
```json
{
  "name": "everything",
  "description": "MCP server with all capabilities",
  "transport": {
    "type": "stdio",
    "command": "npx",
    "args": ["-y", "@modelcontextprotocol/server-everything"]
  }
}
```

### 3. Implementation: Claude Desktop (Global)
If the user specifically asks to install it in Claude Desktop:
1. Locate `%APPDATA%\Claude\claude_desktop_config.json`.
2. Read the current configuration.
3. Append the new server entry.
4. Save the file and notify the user to restart Claude Desktop.

## 🛠 Guidelines

- **Naming**: Use lowercase slugs (e.g., `github`, `brave-search`).
- **Safety**: Verify the source before execution. If it requires sensitive environment variables, ask the user to provide them or use the `runInPersistentPowerShell` tool if input is needed.
- **Dependencies**: Use [system-setup](../system-setup/SKILL.md) if `node`, `npm`, or `uv` are missing.

## Reference
For detailed installation patterns, see [references/installation.md](references/installation.md).
