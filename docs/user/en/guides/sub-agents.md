---
title: Sub-agents & orchestration
---

# Sub-agents & orchestration

> A session can spawn **child sessions (sub-agents)** to split work.  
> Pick a pattern with bundled **orchestration skills**.

---

## What is a sub-agent?

| | Meaning |
|---|--------|
| **Is** | A **child session** created by the parent (e.g. via `agent__startSession`). Parent–child **lineage** is kept. |
| **Is not** | A clone of the parent runtime. Workspace, local skills, and `agents.md` are **not** copied automatically. |
| **Who drives it** | Usually you attach `@skill:delegate` (etc.) so the agent manages spawn / poll / merge — not manual tool calls. |

Parents typically **start → poll/`checkSession` → correct with `messageToSession` → merge results**.  
Sibling sessions do not talk to each other (the parent hub relays).

### Where it shows in the UI

| Screen | Content |
|--------|---------|
| **Chat / History** | Child count on the session card. On delete: **with sub-agents** vs **keep children** |
| Sidebar **Org** (`/org`) | **Explicit orgs** only. One-off delegation sessions do **not** appear here |

![Org view](../../assets/screenshots/guides/org.png)

---

## Isolation rules

Children use their **own workspace** by default.

- Workspace guidance (`agents.md` / `AGENTS.md` / `CLAUDE.md` / `GEMINI.md`) and workspace skills are read **on the child**.
- **Assistant**-scoped skills follow the assistant chosen for the child.
- **Global** skills are available to parent and child.
- To share a folder, skills/agents set `workspaceOverride` (not an automatic copy).

If the child “assumes” parent-only files, local `skills/`, or workspace rules, delegation fails. Put **goal, scope, output format, required paths** in the handoff, or use a shared workspace / `workspaceOverride`.

Tool names and troubleshooting: follow the bundled `@skill:delegate` body.

---

## Which skill when?

Mention `@skill:name` or tell the agent the pattern name.

### 1) One-shot delegation · basics

| Skill | When |
|-------|------|
| **`delegate`** | Create / watch / nudge children and workspace isolation — **baseline** for other patterns |
| **`recruit`** | Create a new **specialist assistant config** (not session delegation itself) |
| **`boost`** | Trim / strengthen tools on an **existing** assistant |

```
@skill:delegate
Read only the auth module in this repo and summarize vulnerability candidates in Markdown.
```

### 2) Work-splitting patterns (ephemeral children)

| Skill | Pattern | One line |
|-------|---------|----------|
| **`divide-conquer`** | Parallel split | Non-overlapping pieces → merge |
| **`hub-spoke`** | Hub–spoke | Coordinator steers workers; no worker-to-worker chat |
| **`pipeline`** | Sequential | A output → B input → C … |
| **`consensus-delegation`** | Multi-view | Same question to 2–4 experts → parent reconciles |
| **`gatekeeper`** | Quality gate | Creator ↔ Reviewer loop (retry limit) |
| **`pair-programming`** | Pair | Driver / Navigator on a **shared workspace** |

```
@skill:divide-conquer
Split frontend, backend, and docs review reports in parallel, then merge into one.
```

```
@skill:consensus-delegation
Review this PR from security, performance, and maintainability angles and reconcile opinions.
```

### 3) Persistent teams · Org

Different from one-off delegation. Use when you need **team artifacts** and **Org UX**.

| Skill | Role |
|-------|------|
| **`teamwork`** | Scaffold task force, artifact dirs, coordination model — **prepare first** |
| **`org`** | After `agent__createOrg`, member sessions appear under Org. Resume from the **Org root** |
| **`org-restructure`** | Add/trim roles or edit charters (`agents.md`, …) on an existing Org |

Flow: `teamwork` scaffold → `org` create/spawn → resume from sidebar **Org** → change structure with `org-restructure`.

- **Org** ≠ scheduled automation (`schedule`) ≠ in-session reminders (`session-schedule`)
- Plain parent–child delegation alone does **not** show under **Org**

```
@skill:teamwork
Create a research / implement / review task force and coordinate with hub-spoke.
```

---

## Quick picker

| Goal | Use |
|------|-----|
| One child, get the result | `@skill:delegate` |
| Parallel independent pieces | `@skill:divide-conquer` |
| Central coordinator + workers | `@skill:hub-spoke` |
| Ordered stages | `@skill:pipeline` |
| Same topic, many viewpoints | `@skill:consensus-delegation` |
| Write ↔ strict review loop | `@skill:gatekeeper` |
| Two agents pair on code | `@skill:pair-programming` |
| Long-lived team + Org UI | `@skill:teamwork` → `@skill:org` |
| Hire a new specialist assistant | `@skill:recruit` (then `delegate`) |
| Slim tools on an existing assistant | `@skill:boost` |

Skill scopes and install: [Skills](skills.md).

---

## Tips

1. **Smallest structure first** — do not build a team if one agent is enough.  
2. **Self-contained handoffs** — children should not guess parent chat.  
3. **Async spawn by default** — for parallelism, skills often use `waitForResult=false`.  
4. **Verify child output** — paths, scope, missing workspace rules — before the final answer.  
5. **On delete** — choose whether to include sub-agents ([Sessions](sessions.md)).

---

## Related

- [Skills](skills.md) · [First agent chat](../getting-started/first-agent.md) · [Troubleshooting](troubleshooting.md)
