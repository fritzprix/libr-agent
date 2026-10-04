---
title: Assistants Guide
---

# Assistants Guide

> Create and manage customized AI agent profiles in the sidebar **Assistants** page (`/assistants`).  
> To start a conversation, click an assistant card on the **Chat** screen.

---

## Manage Assistants

1. Open **Assistants** in the sidebar.
2. Click **Create New Assistant** to build a custom profile.
3. Click **Edit** or **Delete** on an assistant card. Built-in profiles marked **PROTECTED** cannot be deleted.

Cards shown under **Built-in Assistants** and **My Assistants** on the **Chat** screen use these same profiles to launch sessions.

---

## Assistant Configuration Tabs

The **Create New Assistant** and **Edit Assistant** views contain three tabs:

### 1. General Tab

| Field | Description | Requirement |
| --- | --- | --- |
| **Assistant Name** | The display name shown on assistant cards | Required |
| **Description** | A brief summary of the assistant's role | Optional |
| **System Prompt** | Directives defining behavior, persona, and rules | Required |

Click **Save** to apply changes.

### 2. Tools Tab

- **Built-in Tools**: Core built-in tools remain enabled permanently. Use toggles to enable or disable optional built-in tool packages.
- **MCP Servers**: Select external MCP servers accessible to this assistant. To register new MCP servers, open **Extensions** in the sidebar (there is no MCP tab in Settings).

### 3. Skills Tab

Attach skills scoped specifically to this assistant. For details on skill directories and precedence, refer to the [Skills Guide](skills.md).

---

## Recommended Workflows

| Objective | Recommended Method |
| --- | --- |
| Start an immediate chat | Open **Chat** and click a Built-in or Custom Assistant card. |
| Create a specialized persona | Open **Assistants → Create New Assistant**, or invoke `@skill:recruit`. |
| Optimize tool selection | Invoke `@skill:boost`, or adjust tool toggles in the Tools tab. |
| Connect new MCP tools | Register via [Extensions](extensions.md), then permit access in the Assistant **Tools** tab. |

---

## Related Documentation

- [First Agent Chat](../getting-started/first-agent.md) — Launch a chat session
- [Extensions Guide](extensions.md) — Install pre-configured MCP tools
- [Custom MCP Guide](custom-mcp.md) — Register custom MCP servers
- [Skills Guide](skills.md) — Procedural skill workflows
- [Sub-Agents & Orchestration Guide](sub-agents.md) — Multi-agent teams
- [Playbooks Guide](playbooks.md) — Repeatable session templates
