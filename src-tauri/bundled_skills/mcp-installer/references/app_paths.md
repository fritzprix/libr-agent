# MCP Configuration Paths

Default configuration paths for AI-powered editors and agents. Resolve environment variables (`%APPDATA%`, `%USERPROFILE%`, `~`) before reading.

## Windows

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

## macOS

| Application | Config File Path | JSON Root Key |
| :--- | :--- | :--- |
| **Claude Desktop** | `~/Library/Application Support/Claude/claude_desktop_config.json` | `mcpServers` |
| **Claude Code** | `~/.claude.json` (Global)<br>`.mcp.json` (Project) | `mcpServers` |
| **Cursor** | `~/.cursor/mcp.json` (Global)<br>`.cursor/mcp.json` (Project) | `mcpServers` |
| **Cline (VS Code)** | `~/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json` | `mcpServers` |
| **Roo Code (VS Code)** | `~/Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/mcp_settings.json` | `mcpServers` |
| **Windsurf** | `~/.codeium/windsurf/mcp_config.json` | `mcpServers` |
| **VS Code (Copilot)** | `.vscode/mcp.json` | `servers` |
| **Zed** | `~/.config/zed/settings.json` | `context_servers` |

## Linux

| Application | Config File Path | JSON Root Key |
| :--- | :--- | :--- |
| **Claude Desktop** | `~/.config/Claude/claude_desktop_config.json` | `mcpServers` |
| **Claude Code** | `~/.claude.json` (Global)<br>`.mcp.json` (Project) | `mcpServers` |
| **Cursor** | `~/.cursor/mcp.json` (Global)<br>`.cursor/mcp.json` (Project) | `mcpServers` |
| **Cline (VS Code)** | `~/.config/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json` | `mcpServers` |
| **Roo Code (VS Code)** | `~/.config/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/mcp_settings.json` | `mcpServers` |
| **Windsurf** | `~/.codeium/windsurf/mcp_config.json` | `mcpServers` |
| **VS Code (Copilot)** | `.vscode/mcp.json` | `servers` |
| **Zed** | `~/.config/zed/settings.json` | `context_servers` |

## Key Differences

- **Claude Desktop / Cursor / Claude Code / Cline / Windsurf**: Most use `mcpServers` as the root key and share the same internal structure (`command`, `args`, `env`).
- **VS Code (Copilot)**: Uses `servers` as the root key.
- **Zed**: Uses `context_servers` and may have different field names.
- **Goose**: Uses YAML format (`~/.config/goose/config.yaml` on Linux/macOS).

## Automation Tips

- Always check if the file exists before reading.
- If multiple apps are installed, offer the user a choice or sync from the most relevant source.
- When importing, prioritize project-level configs in the current workspace over global configs.
