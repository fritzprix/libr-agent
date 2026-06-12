# Role Heuristics

Hints for mapping **user intent** to assistant configuration. Always validate against live `tool__list` inventory — never assume a server or skill is installed.

## Layer Reminder

| You need… | Configure via… |
| --- | --- |
| Web browsing, knowledge store, bootstrap | `builtinCapabilities` |
| GitHub, search, custom MCP | `externalMcpServers` (IDs) |
| Document workflows, CLI integrations | bundled **skills** (`@skill:` or skill-deployer) |

## Default Assistants (Prefer Reuse)

| Built-in name | Already covers |
| --- | --- |
| **Coding Expert** | Implementation, refactor, debug, `workspace` + `planning` |
| **App Wizard** | MCP setup, agent config, environment, `tool` + `agent` + `bootstrap` |
| **Libr Assistant** | General field ops, `browser` + `planning` + `workspace` |
| **Master Mind** | Orchestration, delegation, strategy |

Create a new assistant only when the user's domain is **narrower** or needs **specific MCP servers** the defaults lack.

## Intent → Capability Hints

| User intent (examples) | Optional builtins | External MCP (if installed) | Related skills |
| --- | --- | --- | --- |
| GitHub / PR / repo ops | `workspace`, `planning` | GitHub server ID | `git-workflow` |
| Research / competitive intel | `browser`, `knowledge`, `planning` | Search, browser MCP IDs | `deep-research`, `knowledge-distiller` |
| Data charts / dashboards | `workspace` | — | `data-viz` |
| Code implementation | `workspace`, `planning` | — | — (often **Coding Expert**) |
| MCP / environment setup | `bootstrap`, `tool`, `agent` | — | `mcp-installer`, `system-setup` |
| Document authoring | `workspace`, `attachments` | — | `docx`, `pptx`, `to-md` |
| Document ingestion / indexing | `workspace` | — | `workspace-indexer`, `to-md` |
| Wiki / linked docs | `workspace` | — | `repo-wiki` |
| Email operations | `workspace` | Email MCP if registered | `email-integration` |
| Calendar / scheduling (user) | `workspace` | — | `calendar-mgmt` |
| Telegram / X social | `workspace` | — | `telegram-cli`, `x-cli` |
| Multi-perspective review | `planning` | — | `consensus-delegation` (spawn, not create) |
| Scheduled automation | `scheduled_task` | — | `schedule`, `session-schedule` |

## What Is NOT a Skill or Builtin

| Name | What it actually is |
| --- | --- |
| `pdf`, `xlsx` | File formats — use `to-md` or `workspace-indexer` skills |
| `code_execution` | Not a builtin alias — use `workspace` |
| `bootstrap` | Builtin capability, not a bundled skill |
| `swarm` | Legacy builtin alias → `agent` domain |

## Temperature Hints

| Role style | `temperature` |
| --- | --- |
| Code, config, ops | 0.1 – 0.4 |
| Balanced generalist | 0.4 – 0.6 |
| Research, brainstorming | 0.5 – 0.7 |

## Language

- **`systemPrompt`**: English only (required for token efficiency and peak model performance).
- **`description`**: English by default; localize only when the Assistants UI language matters to the user.
- **`name`**: English or short universally readable titles (e.g. `GitHub Specialist`).

## Naming

- Use clear role titles: `GitHub Specialist`, `Research Analyst`, `Document Author`
- Avoid generic names: `Expert`, `Helper`, `Agent 2`
- Check `agent__list` for name collisions before create
