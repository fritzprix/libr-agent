---
name: boost
description: Strengthen existing LibrAgent assistant configurations by adding role-appropriate MCP servers and optional builtins from live inventory. Use when the user wants to boost, augment, tune, or strengthen agents, fix missing tools on specialists (e.g. finance agents with empty MCP lists), or align role-tool gaps. Not for creating new assistants (recruit), registering MCP servers (mcp-installer), or workspace agents.md (agent-init).
---

# Boost

Add **missing tools** to **existing** assistant configurations by comparing each agent's role to what is actually installed in this environment.

Boost updates assistants via `agent__update`. It does not create new configs, register MCP servers, or install bundled skills.

## Not This Skill

| Skill | Use for |
| --- | --- |
| **recruit** | Create a **new** specialist when none exists |
| **mcp-installer** | Register or import MCP servers that are not in inventory yet |
| **skill-deployer** / **skill-creator** | Install or author bundled skills (`docx`, `deep-research`, etc.) |
| **agent-init** | Generate workspace `agents.md` |
| **delegate** | Run work with an existing assistant in a child session |

Default seeded assistants (**Coding Expert**, **App Wizard**, **Libr Assistant**, **Master Mind**) are common boost targets. Prefer strengthening them over duplicating roles with **recruit**.

## Critical API Rules

### `externalMcpServers` replaces the full list

`agent__update` **replaces** `externalMcpServers` when provided — it does not merge.

Always compute:

```
next_external = union(current_externalMcpServers, recommended_server_ids)
```

Never pass only the missing IDs, or existing servers will be removed.

### Server IDs, not display names

Resolve names like `fred`, `exa`, or `github` to **cuid2 server IDs** via:

```
tool__list({ "availability": "inventory", "query": "<name>" })
```

### Skills are not MCP servers

`docx`, `deep-research`, `email-integration`, etc. are **bundled skills**. Suggest `@skill:<name>` or **skill-deployer** — never put skill names in `externalMcpServers`.

### Self-modification blocked

An agent cannot `agent__update` the configuration it is currently running as. Run boost from a root/parent session or a different assistant (e.g. **App Wizard**).

## Modes

| Mode | When | Behavior |
| --- | --- | --- |
| **Review** (default) | User says "boost", "check gaps", "what's missing" | Produce gap report only; ask before `agent__update` |
| **Apply** | User says "apply", "fix it", "boost them all" | Update after report, or skip report if scope is explicit |
| **Single** | User names one assistant | Only analyze/update that config |

If the user does not specify Apply, stay in **Review**.

## Workflow

### 1. Scope targets

Determine which assistants to analyze:

- **All configs** — no name given ("boost my agents")
- **Single** — user names one assistant ("boost Coding Expert")
- **Filtered** — domain keyword ("boost finance agents")

```
agent__list({ "type": "configs", "verbose": true })
```

Optional filter with `query`. Record for each target: `id`, `name`, `externalMcpServers`, `externalMcpServerLabels`, `builtinCapabilities`.

### 2. Inventory installed tools

```
tool__list({ "availability": "inventory" })
```

Build a lookup: **server display name / slug → server ID**. Only recommend servers that appear in this inventory.

If a heuristic names a server that is missing, mark it **not installed** and suggest **mcp-installer** — do not fail the whole run.

### 3. Recommend additions

Use [strengthen-heuristics.md](references/strengthen-heuristics.md) as **hints**, not hard rules.

For each assistant:

1. Match **exact name** to a known profile row when possible (seeded assistants, common finance/research titles).
2. Else infer from `name` + `description` + `systemPrompt` keywords conservatively — avoid broad tokens like `expert` or `analyst` alone.
3. Collect **recommended MCP server IDs** (inventory-resolved).
4. Optionally collect **recommended `builtinCapabilities`** additions if the role clearly needs them and they are not already effective.
5. Collect **related bundled skills** as user-facing suggestions only (not via `agent__update`).

**Additive-only rule:** Recommend servers/builtins the role needs but lacks. Do not remove tools the user already attached unless they explicitly ask to strip access.

```
missing_mcp = recommended_ids \ current_externalMcpServers
```

If `missing_mcp` is empty and no builtin gaps → report **already aligned**.

### 4. Produce gap report (Review)

Use this structure per assistant:

```markdown
### {name} (`{id}`)
- **Current MCP:** {labels or "(none)"}
- **Recommend add:** {resolved names} ✓ | {unresolved names} ✗ (not installed)
- **Current builtins:** {effective list}
- **Recommend builtins:** {if any}
- **Skills to mention:** {e.g. @skill:deep-research} — not via agent__update
- **Action:** agent__update with externalMcpServers=[full merged ID list]
```

End with a summary: how many agents need changes, how many servers are missing from inventory, and whether **Apply** is needed.

### 5. Apply updates (Apply mode only)

For each assistant with non-empty `missing_mcp` (or builtin gaps the user approved):

```
agent__update({
  "id": "<assistant-id>",
  "externalMcpServers": ["<id-1>", "<id-2>", "..."]
})
```

Rules:

- Pass the **full merged** `externalMcpServers` array (current ∪ recommended).
- Omit `externalMcpServers` entirely if only suggesting skills or mcp-installer — do not send an empty array unless intentionally clearing access (never do this in boost by default).
- Only include `builtinCapabilities` when intentionally changing optional builtins; merging semantics also **replace** the list — merge with current configured builtins before sending.

Skip assistants where every recommended server is not installed unless the user wants partial apply.

### 6. Verify

```
agent__list({ "type": "configs", "query": "<name>" })
```

Confirm `externalMcpServerLabels` reflect the intended servers.

### 7. Hand off

Tell the user:

- which assistants were updated (name + ID)
- which MCP servers were added (human-readable names)
- which servers were skipped (not installed) → **mcp-installer**
- which bundled skills fit the role → `@skill:` or **skill-deployer**
- existing sessions pick up assistant changes on the next `resolve_agent_config` call; start a **new session** if they need the new tools immediately

Do **not** auto-spawn test sessions unless asked.

## Guidelines

- **Review by default** — confirm before mutating configs.
- **Inventory-first** — never attach servers that are not registered.
- **Merge before update** — union current + recommended MCP IDs; never replace-with-subset by mistake.
- **Partial success** — apply what is installed; report the rest.
- **No permission sprawl** — do not attach all inventory servers to any agent (including test configs) unless the user explicitly requests it.
- **Prefer seeds** — strengthen **Coding Expert**, **App Wizard**, **Libr Assistant**, **Master Mind** before creating overlapping specialists via **recruit**.
- **English prompts unchanged** — boost adjusts tools, not `systemPrompt`, unless the user separately asks to rewrite instructions.

## Builtin Tools

| Step | Tool |
| --- | --- |
| List assistants | `agent__list({ type: "configs", verbose: true })` |
| Inventory MCP | `tool__list({ availability: "inventory" })` |
| Resolve server ID | `tool__list({ availability: "inventory", query: "..." })` |
| Apply changes | `agent__update({ id, externalMcpServers, builtinCapabilities? })` |
| Register missing MCP | **mcp-installer** (separate workflow) |
| Create new specialist | **recruit** |

## References

- [strengthen-heuristics.md](references/strengthen-heuristics.md) — known assistant profiles and MCP name hints
