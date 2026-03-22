# MCP Configuration Paths (Windows)

This table lists the default configuration paths for various AI-powered editors and agents on Windows.

| Application | Config File Path | JSON Root Key |
| :--- | :--- | :--- |
| **Claude Desktop** | `%APPDATA%\Claude\claude_desktop_config.json` | `mcpServers` |
| **Claude Code** | `%USERPROFILE%\.claude.json` (Global)<br>`.mcp.json` (Project) | `mcpServers` |
| **Cursor** | `%USERPROFILE%\.cursor\mcp.json` (Global)<br>`.cursor\mcp.json` (Project) | `mcpServers` |
| **Cline (VS Code)** | `%APPDATA%\Code\User\globalStorage\saoudrizwan.claude-dev\settings\cline_mcp_settings.json` | `mcpServers` |
| **Roo Code (VS Code)** | `%APPDATA%\Code\User\globalStorage\rooveterinaryinc.roo-cline\settings\mcp_settings.json` | `mcpServers` |
| **Windsurf** | `%USERPROFILE%\.codeium\windsurf\mcp_config.json` | `mcpServers` |
| **VS Code (Copilot)** | `.vscode\mcp.json` | `servers` |
| **Zed** | `%USERPROFILE%\.config\zed\settings.json` | `context_servers` |

## Key Differences

- **Claude Desktop/Cursor/Claude Code/Cline/Windsurf**: Most use `mcpServers` as the root key and share the same internal structure (`command`, `args`, `env`).
- **VS Code (Copilot)**: Uses `servers` as the root key.
- **Zed**: Uses `context_servers` and might have different field names.
- **Goose**: Uses YAML format (`~/.config/goose/config.yaml`).

## Automation Tip
When importing, always check if the file exists first. If multiple apps are installed, offer the user a choice or sync across all relevant paths.
