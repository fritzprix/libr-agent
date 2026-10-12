---
title: Sub-agents & orchestration
description: "Coordinate child sessions, delegation, and Org/teamwork patterns with bundled skills."
---

# Sub-agents & orchestration

> A session can start **child sessions (sub-agents)** to split work.  
> Pick parallel, pipeline, or team patterns with bundled **orchestration skills**.

---

## What is a sub-agent?

| | Meaning |
| --- | --- |
| **Is** | A **child session** created by the parent. The parent–child link shows in History. |
| **Is not** | A clone of the parent. By default it uses a **separate work folder**; local skills and folder rules are **not** copied automatically. |
| **Who drives it** | Usually you attach `@skill:delegate` (etc.) so the agent handles handoff, progress, and merge. |

Parents reuse an **idle child with the same role** when possible, and create a new child only when needed. Flow: **hand off → check progress → nudge → merge results**.  
Sibling children do not talk to each other — the parent relays.

You can set **task difficulty** (low / normal / high) when delegating. Under **Settings → AI & Models → Sub-Agent Model Routing**, pick models per level; if unset, the parent or default model is used. Prefer the lowest difficulty that still works.

### Where it shows in the UI

| Screen | Content |
| --- | --- |
| **Chat / History** | Child count on the card. On delete: **Delete all** vs **Delete only this** (keep children) |
| Sidebar **Org** | **Explicit teams** only. One-off delegation does **not** appear here |

![Org view](../../assets/screenshots/guides/org.png)

---

## Isolation rules

Children use their **own work folder** by default. Parent-folder files do not follow automatically.

- When they must see the same deliverables, tell the agent to **reuse the current work folder**.
- Folder rules and project skills are read from **the folder the child uses**.
- Skills tied to **one assistant** follow the assistant chosen for the child.
- Skills for **your whole account** are available to parent and child.
- To point elsewhere, set a session **workspace override**.
- Asking to **reset the child’s conversation** clears chat and plans but keeps folder files. To keep browser tabs, continue without resetting.

If the child “assumes” parent-only files or rules, delegation fails. Put **goal, scope, output format, required paths** in the handoff, or **tell it to reuse the current work folder**.

Follow `@skill:delegate` for operating details.

---

## Which skill when?

Mention `@skill:name` or tell the agent the pattern name.

### 1) One-shot delegation · basics

| Skill | When |
| --- | --- |
| **`delegate`** | Baseline: create / watch / nudge children and get results |
| **`recruit`** | Create a new **specialist assistant** (not session delegation itself) |
| **`boost`** | Trim or strengthen tools on an **existing** assistant |

```
@skill:delegate
Read only the auth module in this repo and summarize vulnerability candidates in Markdown.
```

### 2) Work-splitting (ephemeral children)

| Skill | Pattern | One line |
| --- | --- | --- |
| **`divide-conquer`** | Parallel split | Non-overlapping pieces → merge |
| **`hub-spoke`** | Hub–spoke | Coordinator steers workers; no worker-to-worker chat |
| **`pipeline`** | Sequential | A output → B input → C … |
| **`consensus-delegation`** | Multi-view | Same question to several experts → parent reconciles |
| **`gatekeeper`** | Quality gate | Writer ↔ reviewer loop |
| **`pair-programming`** | Pair | Two roles on a **shared work folder** |

```
@skill:divide-conquer
Split frontend, backend, and docs review reports in parallel, then merge into one.
```

```
@skill:consensus-delegation
Review this PR from security, performance, and maintainability angles and reconcile opinions.
```

### 3) Persistent teams · Org

Different from one-off delegation. Use when you need a **team** and the **Org** screen.

| Skill | Role |
| --- | --- |
| **`teamwork`** | Design the task force and roles — **prepare first** |
| **`org`** | Create member sessions visible under Org. Resume from the **Org root** |
| **`org-restructure`** | Change roles or rules on an existing Org |

Flow: `teamwork` prepare → `org` create → continue from sidebar **Org** → change structure with `org-restructure`.

- **Org** ≠ scheduled tasks (`schedule`) ≠ in-session reminders (`loop`) ≠ wake-on-done (`call-me-back`)
- Plain parent–child delegation alone does **not** show under **Org**

```
@skill:teamwork
Create a research / implement / review task force and coordinate with hub-spoke.
```

---

## Quick picker

| Goal | Use |
| --- | --- |
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

Who a skill applies to: [Skills](skills.md).

---

## Tips

1. **Smallest structure first** — do not build a team if one agent is enough.
2. **Self-contained handoffs** — children should not guess parent chat.
3. **Keep the parent free during parallel work** — follow the skill’s async handoff guidance.
4. **Verify child output** before the final answer.
5. **On delete** — choose whether to include sub-agents in History.

---

## Related

- [Skills](skills.md)
- [First agent chat](../getting-started/first-agent.md)
- [Troubleshooting](troubleshooting.md)
