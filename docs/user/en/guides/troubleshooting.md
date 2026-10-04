---
title: Troubleshooting Guide
---

# Troubleshooting Guide

> Diagnose symptoms, identify causes, and apply fixes based on actual UI element labels.

---

## 1. API Keys & Models

### Chat Fails Immediately or Requests Error Out

**Cause**: API key is missing or invalid.

**Resolution**:

1. Navigate to **Settings → AI & Models** in the sidebar.
2. In **Provider API Keys**, enter the key in the field for your provider.
3. Click **Save Changes**.
4. Return to **Chat** and resend your prompt.

Obtain API keys: [Anthropic](https://console.anthropic.com/) · [OpenAI](https://platform.openai.com/api-keys) · [Gemini](https://aistudio.google.com/) · [Groq](https://console.groq.com/keys)

> [!NOTE]
> Settings does not display a "Connected / Validating" status indicator. Verify functionality by sending a real message in Chat.

### `Invalid API key` / Authentication Failed

1. Open **Settings → AI & Models → Provider API Keys**.
2. Remove leading or trailing whitespace and re-paste the key.
3. Generate a new key from your provider console if needed.
4. Click **Save Changes**.

### `Rate limit exceeded`

Wait a few minutes before retrying, or change **Default LLM** / session **Model** to another model or provider.

### Slow Replies or High Token Costs

| Strategy | Location in UI |
| --- | --- |
| Select a smaller or faster model | **Model** selector in Chat or **Default LLM** in Settings |
| Reduce input context | **Settings → Chat Interface → Max Input Context** |
| Run local offline models | Add a Custom OpenAI Provider pointing to Ollama |

### Output is Too Random or Inconsistent

Open **Settings → AI & Models → Model Preferences**. Enable **Override temperature** and reduce the **Temperature** value (e.g., 0.2–0.5).

---

## 2. Sessions

### Cannot Find an Existing Session

1. Restart the application.
2. Search within session history.
3. Deleted sessions cannot be recovered.

### New Session Will Not Open

1. Verify provider keys and **Default LLM** in **Settings → AI & Models**, then click **Save Changes**.
2. In the sidebar, select **Chat → Built-in Assistants** and click an assistant card.
3. Send a prompt from the draft (**New Session**) state.

> [!NOTE]
> Sessions launch from assistant selection cards rather than a generic "+ New Session" button.

### Session Execution Takes Too Long

The agent may be processing multiple tools or orchestrating sub-tasks. Review the progress badges in the UI. If necessary, interrupt the session and retry with a smaller model.

---

## 3. Tools, Environment & MCP

### Application Reports Missing Python, Node, or uv

Do not look in the Settings menu:

1. Open **Chat → Built-in Assistants → App Wizard**.
2. Request an environment diagnosis. App Wizard runs `setup-wizard` (alias `bootstrap`) to detect your OS and guide runtime installations.

### External MCP Tools Do Not Connect

There is no "MCP Servers" tab in Settings. Use the sidebar **Extensions** screen (`/mcp-servers`).

1. **Recommended**: Select from [Extensions → Recommended Extensions](extensions.md).
2. **Custom / Imports**: Use [Installing Custom MCP Servers](custom-mcp.md) or invoke `@skill:tool-installer`.
3. Confirm that your assistant profile permits access to the target MCP server.
4. Verify that execution runtimes (`npx`, `uv`) execute properly in your terminal.

### Agent Does Not Invoke Available Tools

Specify tool usage explicitly in your prompt, or verify that the active assistant profile allows the required built-in or MCP tools. Inspect tool execution badges in the agent response.

### File or Workspace Tool Failures

Verify the workspace folder path and ensure proper OS file permissions. Review configured paths in **Settings → General**.

### Sub-Agent Cannot Find Files / Not Visible in Org

- Child sessions do not inherit parent workspace folders or local skills automatically. Follow the isolation rules in the [Sub-Agents & Orchestration Guide](sub-agents.md). Write a self-contained handoff via `@skill:delegate` or use a shared workspace path.
- The sidebar **Org** view displays only explicit organization hierarchies. For standard delegations, check sub-agent badges in the session conversation history.

---

## 4. Application Stability & UI

### The Application Freezes

1. Close the application completely and relaunch it.
2. If freezes persist, report the issue on [GitHub Discussions](https://github.com/fritzprix/libr-agent/discussions) with your OS version, app version, and error logs (**exclude API keys**).

> [!NOTE]
> The **Dev** tab appears only in development builds and is not required for daily use.

### The User Interface Renders Incorrectly

Restart the application. If graphics glitches persist, check your OS display settings and graphics drivers.

---

## 5. Common Error Messages

| Message | Recommended Action |
| --- | --- |
| Invalid API key | Re-enter key in **Settings → AI & Models → Provider API Keys**. |
| Rate limit exceeded | Wait briefly or switch to another provider model. |
| Model not found | Update **Default LLM** or click **Refresh models** in Chat. |
| Connection refused / Timeout | Check network connection, local model daemon, or MCP process health. |
| Permission denied | Verify folder read/write permissions in your operating system. |
| Session not found | Restart application and search in Session History. |

---

## Reporting Issues

When reporting issues on [GitHub Discussions](https://github.com/fritzprix/libr-agent/discussions), provide your OS version, LibrAgent version, active provider and model, error messages, and reproduction steps. **Never include API keys or secrets in bug reports.**

---

## Related Documentation

- [Connecting Models](../getting-started/connecting-models.md) — Provider setup
- [5-Minute Quickstart](../getting-started/5-minute-tutorial.md) — App Wizard instructions
- [First Agent Chat](../getting-started/first-agent.md) — Session basics
- [Skills Guide](skills.md) — Skill scopes and catalog
- [Sub-Agents & Orchestration Guide](sub-agents.md) — Multi-agent coordination
- [Extensions Guide](extensions.md) — MCP presets
- [Custom MCP Guide](custom-mcp.md) — Manual MCP configuration
