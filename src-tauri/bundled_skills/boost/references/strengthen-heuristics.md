# Strengthen Heuristics

Hints for **boosting existing assistants** — adding MCP servers and optional builtins they should have for their role.

Always validate against live `tool__list({ availability: "inventory" })`. Rows below name **likely server slugs** for inventory search; resolve each to a **server ID** before `agent__update`.

See also **recruit** [role-heuristics.md](../../recruit/references/role-heuristics.md) for create-time intent mapping and layer rules.

## Layer Reminder

| You need… | Configure via… |
| --- | --- |
| Web browsing, knowledge, bootstrap | `builtinCapabilities` on `agent__update` |
| GitHub, search, finance APIs | `externalMcpServers` (IDs only) |
| Document workflows, CLI integrations | bundled **skills** — suggest `@skill:` only |

## Known Assistant Profiles

Match **exact assistant name** first. Add only servers from inventory that are **not already** in `externalMcpServers`.

### Seeded defaults

| Assistant name | Suggested MCP inventory queries | Optional builtins to consider | Related skills (suggest only) |
| --- | --- | --- | --- |
| **Coding Expert** | `context7`, `exa`, `hn`, `jules` (if installed) | `workspace`, `planning` (usually already effective) | — |
| **App Wizard** | `context7`, `exa`, `hn`, `jules` | `bootstrap`, `tool`, `agent` | `mcp-installer`, `system-setup` |
| **Libr Assistant** | `exa`, `hn`, `arxiv`, `grok`, `gemini`, `fred`, `yahoo-finance` | `browser`, `planning`, `workspace` | `deep-research`, `docx`, `to-md` |
| **Master Mind** | `exa`, `grok`, `hn`, `fred` | `planning` | `delegate`, `consensus-delegation` |

### Finance / macro (common user-created titles)

| Assistant name (exact or close) | Suggested MCP inventory queries | Related skills |
| --- | --- | --- |
| **Conservative Asset Allocator** | `fred`, `yahoo-finance`, `exa` | `deep-research` |
| **Growth Sector Analyst** | `context7`, `exa`, `arxiv` | `deep-research`, `knowledge-distiller` |
| **Macro Risk Strategist** | `fred`, `hn`, `exa` | `deep-research` |

### Research / creative (common titles)

| Assistant name (exact or close) | Suggested MCP inventory queries | Optional builtins | Related skills |
| --- | --- | --- | --- |
| **Research Analyst** | `arxiv`, `exa`, `hn`, `fred` | `browser`, `knowledge`, `planning` | `deep-research`, `knowledge-distiller` |
| **Creative AI Specialist** | `gemini`, `grok`, `comfyui`, `huggingface`, `jules` | `media` | — |
| **Meeting** (or meeting-focused names) | `gemini` | — | — |

## Fallback: Name / Description Keywords

Use only when no profile row matches. Require **specific** multi-word or domain signals — avoid matching on `expert`, `analyst`, or `assistant` alone.

| Signal in name or description | MCP inventory queries to try |
| --- | --- |
| `financial`, `asset alloc`, `portfolio`, `macro`, `risk strateg` | `fred`, `yahoo-finance`, `exa` |
| `sector`, `growth equity`, `equity research` | `context7`, `exa`, `arxiv` |
| `coding`, `software`, `developer`, `engineering` | `context7`, `jules`, `exa`, `hn` |
| `research`, `literature`, `paper` | `arxiv`, `exa`, `hn` |
| `creative`, `image`, `generative` | `comfyui`, `huggingface`, `gemini`, `grok` |
| `app wizard`, `setup`, `bootstrap`, `mcp` | `context7`, `exa`, `hn`, `jules` |
| `meeting`, `transcript`, `notes` | `gemini` |

## Resolving Names to IDs

For each suggested slug:

```
tool__list({ "availability": "inventory", "query": "fred" })
```

- If multiple matches, pick the server whose name/description best fits the role.
- If no match → status **not installed**; recommend **mcp-installer**, do not include in `agent__update`.

## What NOT to Attach via `agent__update`

| Name | What it actually is |
| --- | --- |
| `docx`, `pptx`, `deep-research`, `email-integration` | Bundled **skills** |
| `pdf`, `xlsx` | File formats — use `to-md` or `workspace-indexer` skills |
| `code_execution` | Not a builtin alias — use `workspace` |
| Entire inventory | Never attach all servers unless user explicitly requests |

## Test / Sandbox Assistants

Assistants named for testing (`MCP Test`, `Test Agent`, etc.):

- Do **not** default to ALL servers.
- Ask the user which servers the test should cover, or mirror a specific production assistant.

## Merge Checklist (before `agent__update`)

1. `current = agent.externalMcpServers` from `agent__list`
2. `resolved = [ IDs from inventory for each recommended slug ]`
3. `next = unique(current + resolved)`
4. `agent__update({ id, externalMcpServers: next })`

If also updating `builtinCapabilities`, merge with `configuredBuiltinCapabilities` / effective list the same way — the field **replaces** on update.
