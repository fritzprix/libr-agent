---
title: Error codes
---

# Error codes and fixes

Wording may vary — match **symptom → where to check**. UI anchors: **Settings** / **Extensions** / **History** / **Chat**.

---

## API · auth

### `API key is invalid` / `API key is missing`

**Check:** **Settings → AI & Models → Provider API Keys** → re-enter key → **Save Changes**

Re-issue keys in the provider console (Anthropic / OpenAI / Google AI Studio, …).

### `Rate limit exceeded`

Wait and retry. Switch **Default LLM** or the session Model, or reduce concurrent sessions / scheduled tasks.

### `401` / `Authentication failed`

Re-paste the key without spaces. For MCP, re-check env/auth fields under **Extensions**.

---

## Connection · network

### `Connection refused`

Confirm Ollama / local MCP is running and Base URL / port are correct.  
MCP: verify command · args · cwd in **Extensions**, or run the same command in a terminal. → [Custom MCP](../guides/custom-mcp.md)

### `Timeout` / `Request timed out`

Narrow the task, check network / MCP latency, then retry. Long jobs: split with [Sub-agents](../guides/sub-agents.md).

### MCP not responding

**Extensions** → server config → dependencies (`npx` / `uv`, …) → remove and re-add. → [Extensions](../guides/extensions.md)

---

## Sessions · models · tools

### `Session not found`

Confirm in **History**. If deleted, start a new session from **Chat**.

### `Model not found` / `Model unavailable`

**Default LLM** in Settings, or Chat Model picker → **Refresh models**. Also check the assistant’s default model.

### `Tool execution failed`

Paste the tool error back to the agent and ask it to diagnose. Check workspace paths, OS permissions, and approval / YOLO state.

### `Context window exceeded`

Shorten history or lower **Max Input Context** under **Chat Interface**. Start a fresh session for a new topic.

---

## Related

- [Troubleshooting](../guides/troubleshooting.md) · [FAQ](common-questions.md)
