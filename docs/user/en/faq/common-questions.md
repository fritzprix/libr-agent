---
title: Common questions
---

# Frequently asked questions

> Avoid UI names that do not exist (「LLM Provider」, Settings 「MCP Servers」 tab, …).  
> Menu names follow the **sidebar**.

---

## Getting started

### How do I start LibrAgent?

1. Install from [Releases](https://github.com/fritzprix/libr-agent/releases)
2. **Settings → AI & Models → Provider API Keys** → **Save Changes**
3. **Chat** → assistant card → send from **New Session**

Details: [5-minute tutorial](../getting-started/5-minute-tutorial.md)

### What can I ask the agent to do?

Research, writing, code, files, browsing, scheduled work, and more. Available tools depend on the assistant and **Extensions** (MCP).

### Can I create multiple assistants?

Yes — sidebar **Assistants**. Configure system prompt, default model, builtins/MCP. For a quick start, use **Built-in Assistants** on Chat (e.g. Libr Assistant, App Wizard).

---

## Sessions

### My session disappeared

Conversations are saved. Check **History** / **Bookmarked**, and recent sessions on **Chat**.

### What happens when I delete a session?

History and that session’s context are removed. If there are sub-agents, choose **delete with children** vs **keep**. No restore. → [Sessions](../guides/sessions.md), [Sub-agents](../guides/sub-agents.md)

### Can I continue after quitting the app?

Yes — reopen from **History** or recent sessions.

---

## Models · API keys

### Which models are supported?

Anthropic, OpenAI, Google Gemini, Groq, Fireworks, Cerebras, OpenRouter, Ollama (local), and more.  
Keys / default: **Settings → AI & Models**. Mid-session: Chat Provider/Model pickers change **this session only**. → [Connecting models](../getting-started/connecting-models.md)

### Where are API keys stored?

In local app data. Manage under **Provider API Keys**, then **Save Changes**. Never paste keys into issues or chat.

### Free / local models?

Provider free tiers (e.g. Gemini/Groq) or local endpoints such as **Ollama** via Custom OpenAI-compatible providers. Same Settings tab for key / Base URL.

---

## Tools · Extensions · skills

### What is MCP?

A protocol for external tools. There is **no Settings 「MCP Servers」 tab.** Use sidebar **Extensions** (`/mcp-servers`).

- Catalog: [Extensions](../guides/extensions.md)
- Manual: [Custom MCP](../guides/custom-mcp.md)

### Will the agent delete my files?

Risky actions usually follow **approval** (or YOLO) policy. Destructive deletes typically ask for confirmation. Tool calls appear in the session record.

### Does it use a browser?

When the assistant has browser tools enabled, yes. Scope depends on the tool policy.

### What are skills?

Reusable procedures via `@skill:…`. See [Skills](../guides/skills.md).

---

## Language

### Can I use the app in English?

**Settings → General → Language**. Docs: [한국어](/) · [English](/en/).

---

## Related

- [Troubleshooting](../guides/troubleshooting.md) · [Error codes](error-codes.md)
