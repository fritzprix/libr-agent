---
title: Troubleshooting
---

# Troubleshooting

> Symptom → cause → fix using **real UI names**.

---

## 1. API keys & models

### Chat fails immediately

**Cause**: Missing or wrong API key.

**Fix**:

1. Sidebar **Settings** → **AI & Models**
2. Check **API Key** on the provider card under **Provider API Keys**
3. **Save Changes**
4. Send again from **Chat**

Keys: [Anthropic](https://console.anthropic.com/) · [OpenAI](https://platform.openai.com/api-keys) · [Gemini](https://aistudio.google.com/) · [Groq](https://console.groq.com/keys)

> There is no Settings “connected / verifying” badge. Confirm with a real request after save.

### `Invalid API key` / Authentication failed

1. **Settings → AI & Models → Provider API Keys**
2. Re-paste without spaces / truncation
3. Issue a new key in the provider console
4. **Save Changes**

### `Rate limit exceeded`

Wait and retry, or switch **Default LLM** / the session **Model** to another model or provider.

### Slow or expensive replies

| Try | Where |
|-----|--------|
| Smaller / faster model | Chat **Model** or Settings **Default LLM** |
| Shorter context | **Settings → Chat Interface → Max Input Context** |
| Local model | Custom OpenAI Provider (e.g. Ollama) |

### Too random

**Settings → AI & Models → Model Preferences**: enable **Override temperature** and lower **Temperature** (e.g. 0.2–0.5).

---

## 2. Sessions

### Cannot find a session

1. Restart the app  
2. Search **History**  
3. Deleted sessions cannot be restored  

### New session will not open

1. Confirm key + **Default LLM**, then **Save Changes**  
2. **Chat** → click a **Built-in Assistants** card  
3. Send from the **New Session** draft  

> This is not a **「+ New Session」** button flow.

### Session runs a long time

The agent may be using tools or sub-agents. Check progress / pause UI; stop and retry with a smaller model if needed.

---

## 3. Tools · environment · MCP

### Python / Node / uv missing

Not a Settings tab:

1. **Chat → Built-in Assistants → App Wizard**  
2. Ask it to check and guide installs (uses **setup-wizard** / alias `bootstrap`)

### MCP tools missing

1. Sidebar **Extensions** (not Settings)  
2. Enable / configure the server  
3. Attach it to the [Assistant](assistants.md)  
4. Start a new session  

See [Extensions](extensions.md) · [Custom MCP](custom-mcp.md).

### Tool approval stuck

Approve or deny pending tool calls. YOLO mode (if enabled) auto-approves — use carefully.

---

## 4. Sub-agents

### Child has no context / wrong files

Children do not inherit the parent workspace by default. Put paths and goals in the handoff, or use `workspaceOverride`. See [Sub-agents](sub-agents.md).

### Org list empty

**Org** only lists **explicit orgs**. One-off `@skill:delegate` children stay in History.

---

## Still stuck?

- [FAQ](../faq/common-questions.md) · [Error codes](../faq/error-codes.md)  
- [GitHub Discussions](https://github.com/fritzprix/libr-agent/discussions) · [Issues](https://github.com/fritzprix/libr-agent/issues)
