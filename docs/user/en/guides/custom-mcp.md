---
title: Installing Custom MCP Servers
---

# Installing Custom MCP Servers

> Install Model Context Protocol (MCP) servers not included in the recommended list. Register them via the **Add Extension** UI or delegate the setup to `@skill:tool-installer`.  
> To use pre-configured presets, refer to [Installing MCP with Extensions](extensions.md).

---

## When to Use Custom MCP Servers

- NPM or `npx` packages and GitHub MCP repositories.
- Custom local commands using `node`, `python`, or `uvx`.
- Remote HTTP or Server-Sent Events (SSE) MCP endpoints.
- Migrating MCP configurations from Cursor, VS Code, Windsurf, or Claude Desktop.

---

## Method A — UI: Add Extension

1. Navigate to **Extensions** → **Tools** in the sidebar.
2. Click **Add Extension**.
3. Complete the form fields and click **Save**.

### Common Fields

- **Name** — Unique identifier (e.g., `filesystem`, `my-search`). Reserved built-in names are not allowed.
- **Description** — Optional summary of server capabilities.
- **Transport Type**:
  - **stdio (Local Process)** — Launches a process on your local system.
  - **HTTP (Remote Server)** — Connects to a remote network endpoint.

### stdio Configuration (Local Process)

| Field | Description | Example |
| --- | --- | --- |
| **Command** | Executable binary name | `npx`, `uvx`, `node`, `python` |
| **Arguments** | Space-separated arguments | `-y @modelcontextprotocol/server-filesystem /tmp` |
| **Environment Variables** | Key-value pairs passed to the process | API keys and configuration secrets |

**Example — Filesystem MCP:**

- **Name**: `filesystem`
- **Transport**: `stdio`
- **Command**: `npx`
- **Arguments**: `-y @modelcontextprotocol/server-filesystem /path/to/folder`

If `npx` or `uv` is missing, install the required runtimes using `@skill:setup-wizard` or the **App Wizard**.

### HTTP Configuration (Remote Server)

| Field | Description |
| --- | --- |
| **URL** | Complete URL for the MCP endpoint |
| **API Key / Token** (Optional) | Automatically attached as `Authorization: Bearer <token>` |
| **Custom Headers** (Advanced) | Additional HTTP headers in JSON format |
| **Enable SSE** | Enables streaming for Server-Sent Events. Disable for stateless HTTP |

### Steps After Saving

1. Verify that the extension shows the **Active** status in the list.
2. Permit access to the extension in your target Assistant settings if needed.
3. Start a new session or send the next message to load the tools.

To modify or remove an extension, click **Edit** or **Delete** on the extension card.

---

## Method B — Agent Setup: `@skill:tool-installer`

Instruct the agent in chat:

```
@skill:tool-installer
Register @modelcontextprotocol/server-everything in LibrAgent using npx.
```

```
@skill:tool-installer
Import MCP server configurations from Cursor into LibrAgent.
```

The tool installer skill handles:

- Direct registration from npm packages, GitHub repositories, or JSON configurations.
- Automatic configuration imports from Cursor, VS Code, Windsurf, and Claude Desktop.

For detailed procedures, refer to the bundled skill documentation in the [Skills Guide](skills.md).

---

## Connect to an Assistant

Tools remain unavailable in chat until you enable them for your active assistant profile.

1. Open **Assistants** and click **Edit** on your target assistant.
2. In the tool permissions section, select the newly registered MCP server.
3. Start or reload a session using that assistant profile.

---

## Troubleshooting

| Symptom | Verification Steps |
| --- | --- |
| Server saved, but tools do not appear | Check if server is **Active**, permit tools in Assistant settings, and reload the session. |
| `command not found` or npx failure | Run the App Wizard or prompt `@skill:setup-wizard` to install missing runtimes. |
| Startup timeout error | Increase timeout in **Settings → Advanced → MCP Discovery Timeout**. |
| Name registration rejected | The name conflicts with a built-in server. Choose a different name. |
| Preset requires an API key | Enter required keys in the extension configuration form before starting. |

---

## Related Documentation

- [Installing MCP with Extensions](extensions.md) — Pre-configured extension presets
- [5-Minute Quickstart](../getting-started/5-minute-tutorial.md) — App Wizard setup
- [Troubleshooting Guide](troubleshooting.md) — General application recovery
