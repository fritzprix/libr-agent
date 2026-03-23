# MCP Installation Reference

This reference provides standard procedures for installing and registering MCP servers from various sources (GitHub, npm) into different environments (LibrAgent, Claude Desktop).

## 1. Registration Methods

### 1.1 LibrAgent (Current Environment)
To register a new MCP server in LibrAgent, use the `tool__register` tool.

**Parameters:**
- `name`: Unique slug for the server (e.g., 'github', 'local-fs')
- `description`: Purpose of the server
- `transport`: 
    - `type`: 'stdio' (most common for local)
    - `command`: The executable (e.g., 'npx', 'uvx', 'python')
    - `args`: List of arguments
    - `env`: Environment variables (optional)

**Example (npm/npx):**
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

**Example (Python/uvx):**
```json
{
  "name": "sqlite",
  "description": "SQLite database explorer",
  "transport": {
    "type": "stdio",
    "command": "uvx",
    "args": ["mcp-server-sqlite", "--db-path", "path/to/db"]
  }
}
```

### 1.2 Claude Desktop
For Claude Desktop, you must edit the `claude_desktop_config.json` file.

**Path (Windows):** `%APPDATA%\Claude\claude_desktop_config.json`

**Structure:**
```json
{
  "mcpServers": {
    "server-name": {
      "command": "npx",
      "args": ["-y", "@org/package"],
      "env": {
        "KEY": "VALUE"
      }
    }
  }
}
```

## 2. Installation Sources

### 2.1 npm / npx
- Use `npx -y <package-name>` for quick execution without manual install.
- Advantage: Always uses the latest version, no local management needed.

### 2.2 GitHub (TypeScript/Node)
1. Clone the repository.
2. Run `npm install`.
3. Run `npm run build`.
4. Register with `node <path-to-dist/index.js>`.

### 2.3 GitHub (Python)
- Use `uvx --from <git-url> <command>` for direct execution.
- Or clone and use `uv run`.

## 3. Environment Variables
Always check if the MCP server requires:
- API Keys (e.g., `GITHUB_PERSONAL_ACCESS_TOKEN`)
- Local paths (e.g., `DB_PATH`)
- Configuration flags
