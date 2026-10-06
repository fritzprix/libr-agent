---
title: Skills Guide
---

# Skills Guide

> Skills are reusable procedural workflows defined in a `SKILL.md` file. Invoke skills using `@skill:name` in chat, or let agents select them from `<available_skills>`.  
> Built-in skills reside in `bundled_skills` (mirrored to `system_skills` at runtime). Custom skills are stored in distinct directories according to their **scope**.

---

## What is a Skill?

| Item | Description |
| --- | --- |
| **What it is** | A folder containing `SKILL.md` and optional `scripts/`, `references/`, and `assets/`. It specifies standard operating procedures. |
| **What it is not** | It is not a settings menu or an MCP server. It instructs models on how and when to use existing tools. |
| **Who uses it** | Users invoke skills with `@skill:...`, or agents read them contextually to complete specialized tasks. |

Built-in tools provide execution capabilities. Skills provide operational instructions and domain logic.  
To install MCP tools, refer to [Extensions](extensions.md) and [Custom MCP](custom-mcp.md).

---

## Skill Scope and Precedence

When skills with identical names exist in multiple scopes, the scope with higher precedence takes priority (case-insensitive, first-wins matching).

### Precedence Hierarchy (Highest to Lowest)

`workspace` → `agent import` (IDE skills) → `assistant` → `custom` (`additionalSkillPaths`) → `global` (user) → `system` (bundled mirror)

### Scope Overview

**workspace** — Limited to the current session or project repository

- Path: <code v-pre>{workspace}/.libragent/skills/{name}/</code>
- Usage: Recommended for project-specific instructions and easy deletion.

**assistant** — Assigned to a specific assistant profile

- Path: <code v-pre>{dataDir}/assistants/{assistantId}/skills/{name}/</code>
- Usage: Managed directly via the Skills tab in Assistant configuration.

**global** (user) — Shared across all sessions for the current user

- Path: <code v-pre>{dataDir}/user_skills/{name}/</code>
- Usage: Standard location for permanent user-created skills.

**system** (bundled) — Provided out of the box by LibrAgent

- Path: <code v-pre>{dataDir}/system_skills/{name}/</code>
- Usage: Runtime mirror of `bundled_skills`. Do not write custom files here because the app refreshes this folder on launch.

**agent import** — Auto-discovered from existing IDE skills in workspace

- Path: `.cursor/skills/`, `.agents/skills/`, etc.
- Usage: Automatically indexed from supported coding assistants.

> [!NOTE]
> Typical `dataDir` locations:
> - Linux: `~/.local/share/com.fritzprix.libragent/`
> - macOS: `~/Library/Application Support/com.fritzprix.libragent/`
> - Windows: `%APPDATA%\com.fritzprix.libragent\`

### Common Misconceptions

| Statement | Reality |
| --- | --- |
| "I want to install a global custom skill." | Place it in **user_skills (global)**, not **system_skills**. |
| "I want to edit bundled skills directly." | Bundled skills update through source code in `src-tauri/bundled_skills/`. |
| "I placed a skill at the workspace root (`my-skill/`)." | Scanners ignore it. Skills must reside in <code v-pre>.libragent/skills/{name}/</code>. |

### Accessing Scopes in the UI

| Goal | Location in UI |
| --- | --- |
| Manage local skills | Sidebar: **Extensions → Skills** |
| Configure skills per assistant | **Assistants → Edit → Skills** tab |
| Deploy via chat command | Use `@skill:skill-deployer` |

---

## How to Use Skills (`@skill:`)

1. In the chat input, type `@` and select `@skill:`.
2. Choose a skill name from the auto-complete menu and state your objective.

```
@skill:deep-research
Compare recent product releases from Vendor A and Vendor B into a Markdown report.
```

Reference files with `@file:path`. The available skill index updates on the subsequent turn.

---

## Create and Deploy Skills (Meta-Skills)

| Phase | Skill | Role |
| --- | --- | --- |
| 1. Author and Validate | `@skill:skill-creator` | Drafts frontmatter and runs `validate_skill.py --strict` |
| 2. Deploy to Scope | `@skill:skill-deployer` | Copies validated skill folders to workspace, global, or assistant paths |

**Example workflow:**

```
@skill:skill-creator
Create a 'weekly-notes' skill and validate it with --strict.
```

```
@skill:skill-deployer
Deploy the 'weekly-notes' skill into the workspace scope for this session.
```

Never deploy custom skills to `system_skills` or `bundled_skills`. When uncertain, choose **workspace** scope.

---

## Bundled Skills Catalog

Bundled skills ship with the app (`bundled_skills` → `system_skills`). Auto-complete suggestions in chat reflect the latest available catalog.

### Setup and Environment

`setup-wizard`, `tool-installer`, `agent-init`, `computer-diagnosis`

### Multi-Agent and Orchestration

To learn about child sessions and multi-agent coordination patterns, refer to the [Sub-Agents & Orchestration Guide](sub-agents.md).

`delegate`, `teamwork`, `org`, `org-restructure`, `divide-conquer`, `hub-spoke`, `pipeline`, `consensus-delegation`, `gatekeeper`, `pair-programming`, `recruit`, `boost`

### Scheduling and Timer Triggers

`schedule`, `loop`, `call-me-back`

### Context and History

`context-recall` — Recovers conversation history and architectural decisions from pre-compaction epochs (`.libragent/pre_compaction_epoch_{N}.md`).

### Research and Documentation

`deep-research`, `knowledge-distiller`, `to-md`, `docx`, `pptx`, `visualize`, `workspace-indexer`, `repo-wiki`, `soul-awakening`

### Development and Integrations

`git-workflow`, `bench`, `fine-tune`, `email-integration`, `calendar-mgmt`, `telegram-cli`, `x-cli`, `ig-cli`, `skill-creator`, `skill-deployer`, `tool-creator`, `playbook-creator`

### Common Workflows

| Objective | Recommended Approach |
| --- | --- |
| Install runtimes (Python, Node) | `@skill:setup-wizard` or App Wizard |
| Install recommended MCP servers | [Extensions Guide](extensions.md) |
| Install custom MCP servers | [Custom MCP Guide](custom-mcp.md) or `@skill:tool-installer` |
| Turn procedures into skills | `@skill:skill-creator` → `@skill:skill-deployer` |
| Orchestrate multi-agent teams | [Sub-Agents & Orchestration Guide](sub-agents.md) |
| Recover lost session context | `@skill:context-recall` |
| Email triage with browser fallback | `@skill:email-integration` |
| Configure custom assistants | [Assistants Guide](assistants.md) |
| Automate recurring tasks | [Playbooks Guide](playbooks.md) and [Automation Guide](automation.md) |

---

## Related Documentation

- [Sub-Agents & Orchestration Guide](sub-agents.md) — Multi-agent execution patterns
- [Extensions Guide](extensions.md) — Pre-configured MCP presets
- [Custom MCP Guide](custom-mcp.md) — Manual MCP server registration
- [5-Minute Quickstart](../getting-started/5-minute-tutorial.md) — Core concepts
- [First Agent Chat](../getting-started/first-agent.md) — Prompting basics
- [Troubleshooting Guide](troubleshooting.md) — Common error resolution
