---
name: recruit
description: Analyze installed tools and user intent to create specialized LibrAgent assistant configurations via agent__create. Use when the user wants to recruit, create, or build a domain expert or specialist agent (e.g. GitHub, research, documents, communications) and no suitable assistant already exists. Not for workspace agents.md (agent-init), task-force scaffolding (teamwork/org), skill authoring (skill-creator), or MCP registration (mcp-installer).
---

# Recruit

Create a **named assistant configuration** (specialist) by matching user intent to the tools actually available in this environment.

Recruit configures **assistants** — reusable agent profiles stored in LibrAgent. It does not create bundled skills, MCP servers, or workspace constitution files.

## Not This Skill

| Skill | Use for |
| --- | --- |
| **agent-init** | Generate `agents.md` / workspace guidelines |
| **teamwork** / **org** | Persistent multi-agent task forces |
| **delegate** | Spawn a child session with an existing assistant |
| **consensus-delegation** | Compare 2–4 existing specialists on one question |
| **skill-creator** / **skill-deployer** | Author or install skills |
| **mcp-installer** | Register external MCP servers |
| **boost** | Strengthen **existing** assistants with missing MCP servers (additive `agent__update`) |

Default seeded assistants (**Coding Expert**, **App Wizard**, **Libr Assistant**, **Master Mind**) already cover common roles. Prefer **boost** or `agent__update` before creating near-duplicates with **recruit**.

## Three Layers (Do Not Mix)

| Layer | Examples | Set via `agent__create`? |
| --- | --- | --- |
| **Core builtins** | `planning`, `workspace`, `agent`, `tool`, `skills` | Always on — do not list redundantly |
| **Optional builtins** | `browser`, `knowledge`, `bootstrap`, `media`, `history` | `builtinCapabilities` — restricts/enables optional services |
| **External MCP** | GitHub, search, filesystem servers | `externalMcpServers` — **server IDs** from `tool__list`, not display names |
| **Bundled skills** | `docx`, `deep-research`, `email-integration` | **Not** via `agent__create` — suggest `@skill:name` or **skill-deployer** separately |

There is no builtin alias `code_execution`. Code work uses **`workspace`** (and usually `planning`).

## Workflow

### 1. Clarify intent

Before scanning tools, extract:

- **Domain** — what the specialist owns (e.g. GitHub PR review, competitive research, email ops)
- **Deliverable** — what success looks like
- **Boundaries** — what it should not do

If the request is vague ("make me an expert"), ask one focused question about domain and primary deliverable.

### 2. Check for existing assistants

```
agent__list(type="configs", query="<domain keyword>")
```

- If a close match exists → recommend **reuse** (`agent__startSession`) or **`agent__update`** instead of creating another config.
- If the user names a default assistant (Coding Expert, App Wizard, etc.) → confirm they want a **new** config, not the built-in one.

### 3. Inventory available tools

```
tool__list({ "availability": "inventory" })
```

From the result, collect:

- **External MCP servers** — record each server's **ID** and human-readable name/description
- **Optional builtins** you may need to enable explicitly

Do not assume servers exist because the user mentioned them (e.g. "GitHub"). Verify in inventory first.

### 4. Design the configuration

Use [role-heuristics.md](references/role-heuristics.md) as **hints**, not hard rules. Match intent + inventory dynamically.

Decide:

| Field | Guidance |
| --- | --- |
| `name` | Short, unique, human-readable (e.g. `GitHub Specialist`) |
| `description` | 1–2 sentences for the Assistants picker card |
| `systemPrompt` | **Always write in English** (token efficiency + model performance). Use [prompt-templates.md](references/prompt-templates.md); fill domain, tools, and boundaries |
| `temperature` | Lower (0.1–0.4) for code/config; medium (0.5–0.7) for research/exploration |
| `builtinCapabilities` | Omit to enable all optional builtins; pass a **minimal** list to restrict (e.g. `["browser", "knowledge"]`) |
| `externalMcpServers` | Array of **MCP server IDs** only |

**Minimal-tool rule:** Grant only builtins and MCP servers the role actually needs. Fewer tools → clearer behavior.

### 5. Create the assistant

```
agent__create({
  "name": "...",
  "description": "...",
  "systemPrompt": "...",
  "temperature": 0.3,
  "builtinCapabilities": ["workspace", "planning"],
  "externalMcpServers": ["<mcp-server-id-from-tool__list>"]
})
```

Notes:

- `builtinCapabilities` maps to optional builtin services beyond the always-on core set.
- `externalMcpServers` must be **IDs** (cuid2), not slugs like `github`.
- Model/provider selection is **not** set here — it comes from session or global settings.

### 6. Confirm and hand off

```
agent__list(type="configs", query="<new name>")
```

Tell the user:

- the new assistant **name** and **ID**
- which builtins and MCP servers were attached
- how to use it: `agent__startSession(agentId="...")` or select it in Assistants
- optional: relevant `@skill:name` mentions or **skill-deployer** if a bundled skill fits the role

Do **not** auto-spawn a test session unless the user asks.

### 7. (Optional) Link bundled skills

If the role benefits from a bundled skill (`docx`, `deep-research`, `email-integration`, etc.):

- Mention `@skill:<name>` in the assistant's typical tasks, or
- Offer **skill-deployer** to install an assistant-scoped copy

Skills are separate from `agent__create` — never list skill names in `builtinCapabilities` or `externalMcpServers`.

## Guidelines

- **Duplicate prevention** — always run Step 2 before `agent__create`.
- **Evidence-based** — only attach MCP servers that appear in `tool__list` inventory.
- **Distinct roles** — each new assistant should own a clear purpose; avoid "does everything" configs.
- **English system prompts** — `systemPrompt` must be **English only**, even when the user speaks Korean or another language. User-facing `description` may match the user's locale; instructions sent to the model stay in English.
- **Prompt quality** — include verification protocol and attention-economy rules (see templates); do not only list tool names.
- **No E2E test loop** — listing the config is sufficient confirmation.

## Builtin Tools

| Step | Tool |
| --- | --- |
| Find existing assistants | `agent__list(type="configs", query="...")` |
| Inventory MCP + tools | `tool__list({ availability: "inventory" })` |
| Create specialist | `agent__create(...)` |
| Update existing | `agent__update(...)` |
| Verify creation | `agent__list(type="configs")` |
| Use specialist | `agent__startSession(agentId="...")` |

## References

- [role-heuristics.md](references/role-heuristics.md) — domain → capability mapping hints
- [prompt-templates.md](references/prompt-templates.md) — system prompt skeletons
