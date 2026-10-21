---
name: tool-installer
description: "Registers, verifies, or imports MCP (Model Context Protocol) servers in LibrAgent or Claude Desktop. Use when users share MCP connection info (npm/npx/PyPI package, MCP server GitHub repo, config JSON, server URL), want a capability that needs a new MCP server (\"I want stock data\"), or want to import/sync settings from Cursor, VS Code, Windsurf, and similar editors. GitHub URL rule: a repo or subpath that contains SKILL.md is a skill — use skill-deployer instead; an MCP server package or MCP config uses this skill; if unsure, inspect the repo before installing. Triggers: install MCP, add MCP server, connect this, 연결해줘, 깔아줘, 설치해줘, MCP 추가, 서버 등록."
---

# MCP Installer

This skill registers MCP servers into the current LibrAgent session or the user's Claude Desktop configuration. It covers both **direct installation** (from packages or URLs) and **import/sync** (from other AI editors).

## 🚀 Workflow

### Step 0: Determine Input Path

| User intent | Path |
| --- | --- |
| "Import from Cursor", "sync my MCP servers", "what's installed elsewhere?" | **Import** → below |
| npm/PyPI package, MCP server GitHub repo, server URL, or pasted JSON config | **Direct install** → below |
| GitHub URL whose repo or subpath contains `SKILL.md` | Not an MCP server → use **skill-deployer** |
| GitHub URL and you cannot tell | Inspect it first (README, `package.json`/`pyproject.toml`, `SKILL.md`), then pick the path. Ask the user only if it is still unclear. |
| "Install it" with no package, URL, or config | Ask what to install. Do not guess a package. |

### Import Workflow

1. **Identify sources** — Consult [app_paths.md](references/app_paths.md) for config locations.
2. **Scan** — Read detected config files. Resolve environment variables (`%APPDATA%`, `%USERPROFILE%`, `~`) before reading. If a project-level config (e.g., `.cursor/mcp.json`) exists in the current workspace, prioritize it.
3. **Compare** — List all found MCP servers. Use `tool__listServers` to compare with LibrAgent's current registrations. Mark servers as new or already registered.
4. **Select** — Present the list to the user. Ask which servers to import (all or specific ones).
5. **Register** — Use `tool__registerServer` for each selected server. Skip duplicates (same name and command). Each call asks the user for approval (see Step 2 below).
6. **Verify** — Run Step 3 below for each registered server.
7. **(Optional)** — Offer to update `claude_desktop_config.json` as well.

**Schema notes:** VS Code Copilot uses `servers` as the root key; Zed uses `context_servers`. Normalize entries before registration.

### Direct Install Workflow

#### 1. Analyze Information

Identify the source and transport details.

- **npx/npm**: Use `npx -y <package>` for registration.
- **GitHub**: Determine if it's TypeScript (needs build) or Python (can use `uvx`).
- **Claude Config**: Extract command, args, and env variables.

#### 2. Register in LibrAgent

Use the `tool__registerServer` tool to add the MCP server. Never hand-edit LibrAgent config files or the database.

`tool__registerServer` and `tool__updateServer` always pause for the user's approval, even in YOLO mode, because a stdio server runs an arbitrary command. Before you call it, tell the user in one line what will run (command and args, or URL) and which env variables it needs. If the user rejects it, stop and ask what to change; do not retry the same call.

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

#### 3. Verify (required)

Installation is not done until the server is verified.

1. Call `tool__verifyServer` with the server name.
2. On success, report the tools it exposes. Tell the user the server is saved and that its tools become callable after it is attached to an assistant (`agent__updateAgent`) and a new session starts.
3. On failure, report the real error and the next step:
   - `command not found` / missing `node`, `npx`, `uv`, `python` → run **setup-wizard**, then verify again.
   - Missing or wrong API key / env variable → ask the user for the value, then `tool__updateServer`.
   - Package not found or crashes on start → show the error output; do not report success.

#### 4. Claude Desktop (Optional)

If the user specifically asks to install it in Claude Desktop:

1. Locate `claude_desktop_config.json` (see [app_paths.md](references/app_paths.md)).
2. Read the current configuration.
3. Append the new server entry.
4. Save the file and notify the user to restart Claude Desktop.

## 🛠 Guidelines

- **Duplicate Prevention**: Do not register servers that already exist in LibrAgent with the same name and command.
- **Naming**: Use lowercase slugs (e.g., `github`, `brave-search`).
- **Safety**: Verify the source before execution. If it requires sensitive environment variables, ask the user to provide them or use the `workspace__runInPersistentPowerShell` tool if input is needed.
- **Dependencies**: Use [setup-wizard](../setup-wizard/SKILL.md) if `node`, `npm`, or `uv` are missing.
- **Path Resolution**: Resolve environment variables before reading config files on any platform.

## Reference

- [installation.md](references/installation.md): npm, GitHub, and Python install patterns
- [app_paths.md](references/app_paths.md): Config paths for Cursor, VS Code, Windsurf, and other editors
