---
title: Skills Guide
description: "Who a skill applies to, how to call @skill, create and deploy with skill-creator / skill-deployer, and a short bundled catalog."
---

# Skills Guide

> A skill is a **reusable procedure** the agent follows. Call it with `@skill:name` in chat, or let the agent pick one when it fits.  
> Some skills ship with the app. Others you create and attach to **this project**, **one assistant**, or **your whole account**.

---

## What is a Skill?

| | Description |
| --- | --- |
| **What it is** | Written steps for *when* and *how* to do a job (close to a playbook) |
| **What it is not** | Not a Settings screen and not an MCP server. It teaches how to use tools you already have |
| **Who uses it** | You via `@skill:…`, or the agent when the task matches |

Built-in and MCP tools **do the work**. Skills say **how to run the job**.  
To install MCP tools, see [Extensions](extensions.md) and [Custom MCP](custom-mcp.md).

---

## Who it applies to

When you place a skill (or ask deployer to place it), choose the **audience**.

| Audience | Meaning | Use when |
| --- | --- | --- |
| **This project (workspace)** | Only this work folder / session side | Default. Try-outs and project-only procedures |
| **This assistant** | Only that assistant profile | Different roles (coding vs research) |
| **Your whole account (global)** | Available in almost every session | Personal procedures you want everywhere |
| **Built into the app** | Skills LibrAgent ships | You do not create these. See the catalog below |

If the same name exists in more than one place, the **narrower** audience wins (project → assistant → account → app built-in).

If the project already has IDE skills (for example Cursor), LibrAgent can discover those too.

### Where to check in the UI

| Goal | Where |
| --- | --- |
| Browse and manage skills | Sidebar **Extensions → Skills** |
| Skills tied to an assistant | **Assistants** → Edit → **Skills** tab |
| Create and install | **Create and deploy** below (`skill-deployer`) |

### Easy mix-ups

| You might think | What to do |
| --- | --- |
| “Put it in the system” | Use **your whole account (global)**. Do not put custom skills in the app’s built-in area. |
| “Edit the skills that came with the app” | The app may overwrite them. Make **your own** skill and deploy it. |
| “I’ll just copy folders myself” | Easy to pick the wrong place. Prefer **`skill-deployer`**. |

---

## How to use (`@skill:`)

1. In Chat, type `@` and choose `@skill:`
2. Pick a name, then say what you want done.

```
@skill:deep-research
Compare recent announcements from vendors A and B into a Markdown report.
```

Attach files with `@file:path`. After a fresh deploy, the list often updates on the **next** message.

---

## Create and deploy

You do not need to memorize disk paths. Use these two skills in order.

| Step | Call | What it does |
| --- | --- | --- |
| 1. Draft and check | `@skill:skill-creator` | Writes the procedure and validates the format |
| 2. Install | `@skill:skill-deployer` | Installs it for the **audience** you choose |

```
@skill:skill-creator
Create a weekly-notes skill and validate it strictly.
```

```
@skill:skill-deployer
Deploy weekly-notes to this project (workspace).
```

- Unsure where it should live? Choose **this project (workspace)**.
- Do **not** deploy into the app’s built-in area — the app may clear it.
- For an assistant-only skill, tell deployer **which assistant**.

---

## Skills that ship with the app (summary)

Chat `@skill:` autocomplete is the freshest list. Common names:

### Setup and environment

`setup-wizard`, `tool-installer`, `agent-init`, `computer-diagnosis`

### Multi-agent and orchestration

How to split work across child sessions: [Sub-agents & orchestration](sub-agents.md).

`delegate`, `teamwork`, `org`, `org-restructure`, `divide-conquer`, `hub-spoke`, `pipeline`, `consensus-delegation`, `gatekeeper`, `pair-programming`, `recruit`, `boost`

### Scheduling and wake-ups

`schedule`, `loop`, `call-me-back`

### Session and context

`context-recall` — Recover earlier decisions or details after the chat was compacted or summarized

### Research and documents

`deep-research`, `science-search`, `knowledge-distiller`, `to-md`, `docx`, `pptx`, `visualize`, `workspace-indexer`, `repo-wiki`, `soul-awakening`

`science-search` — Search scholarly papers, look up DOIs, similar works, and citations (free, no API key)

### Development and integrations

`git-workflow`, `bench`, `fine-tune`, `email-integration`, `calendar-mgmt`, `telegram-cli`, `x-cli`, `ig-cli`, `skill-creator`, `skill-deployer`, `tool-creator`, `playbook-creator`

### Common combinations

| Goal | Try |
| --- | --- |
| Install runtimes | `@skill:setup-wizard` + App Wizard |
| Recommended MCP | [Extensions](extensions.md) |
| Custom MCP | [Custom MCP](custom-mcp.md) · `@skill:tool-installer` |
| Turn a procedure into a skill | `skill-creator` → `skill-deployer` |
| Child sessions / teams | [Sub-agents guide](sub-agents.md) |
| Recover pre-compaction context | `@skill:context-recall` |
| Papers / DOIs | `@skill:science-search` |
| Email | `@skill:email-integration` |
| Assistants | [Assistants](assistants.md) |
| Recurring work | [Playbooks](playbooks.md) · [Automation](automation.md) |

---

## Related

- [Sub-agents & orchestration](sub-agents.md)
- [Extensions (MCP)](extensions.md)
- [Custom MCP](custom-mcp.md)
- [5-minute tutorial](../getting-started/5-minute-tutorial.md)
- [First agent chat](../getting-started/first-agent.md)
- [Troubleshooting](troubleshooting.md)
