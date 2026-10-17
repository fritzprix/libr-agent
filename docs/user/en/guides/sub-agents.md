---
title: Sub-agents & orchestration
description: 'Coordinate child sessions, delegation, and Org/teamwork patterns with bundled skills.'
---

# Sub-agents & orchestration

> A session can start **child sessions (sub-agents)** to split work.  
> Pick parallel, pipeline, or team patterns with bundled **orchestration skills**.

---

## What is a sub-agent?

|                   | Meaning                                                                                                                               |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| **Is**            | A **child session** created by the parent. The parent–child link shows in History.                                                    |
| **Is not**        | A clone of the parent. By default it uses a **separate work folder**; local skills and folder rules are **not** copied automatically. |
| **Who drives it** | Usually you attach `@skill:delegate` (etc.) so the agent handles handoff, progress, and merge.                                        |

Parents reuse an **idle child with the same role** when possible, and create a new child only when needed. Flow: **hand off → check progress → nudge → merge results**.  
Sibling children do not talk to each other — the parent relays.

You can set **task difficulty** (low / normal / high) when delegating. Under **Settings → AI & Models → Sub-Agent Model Routing**, pick models per level; if unset, the parent or default model is used. Prefer the lowest difficulty that still works.

### Where it shows in the UI

| Screen             | Content                                                                                    |
| ------------------ | ------------------------------------------------------------------------------------------ |
| **Chat / History** | Child count on the card. On delete: **Delete all** vs **Delete only this** (keep children) |
| Sidebar **Org**    | **Explicit teams** only. One-off delegation does **not** appear here                       |

![Org view](../../assets/screenshots/guides/org.png)

**Org export / import**: You can export a team as a `.libragent-org` bundle file from the Org view to back it up or move it to another machine. Use **Export Org** on an Org card to choose a destination folder, and **Import Org** at the top of the view to pick a bundle file. Import **merges** with what you already have, and sensitive data such as API keys is not included. Encrypted bundles cannot be imported from this screen yet, so export without a password.

---

## How it works at a glance

```text
             ┌──────────────────── Parent session ────────────────────┐
             │ ① split  ② hand off  ③ check progress  ④ merge  ⑤ verify │
             └───────┬───────────────┬───────────────┬─────────────────┘
                     ▼               ▼               ▼
                Child A          Child B          Child C
        (own assistant · work folder · complexity)   ← siblings never talk directly
```

The parent drives children through the **built-in tools** below. You do not call them yourself — attach a skill and the agent runs them in order.

| What it does                        | Built-in tool                                                          | Good to know                                                                               |
| ----------------------------------- | ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| Look up assistants and sessions     | `agent__listAgents`                                                    | Checks for an **idle child with the same role** before spawning a new one                  |
| Create a child                      | `agent__spawnSession`                                                  | `waitForResult=true` blocks until done; `false` lets the parent keep working               |
| Assign new work / follow up         | `agent__messageToSession`                                              | `reset=true` clears conversation and plan but **keeps work-folder files**                  |
| Check progress and results          | `agent__checkSession`                                                  | `wait=true` blocks until done. Metadata shows whether the folder is `SHARED` or `ISOLATED` |
| Stop                                | `agent__stopSession`                                                   | Aborts a hung child                                                                        |
| Compact / clean up                  | `agent__compactSessionContext`, `agent__deleteSession`                 | Shrinks long sessions or removes unused children                                           |
| Create / edit specialist assistants | `agent__createAgent`, `agent__updateAgent`                             | Used by `recruit` and `boost`                                                              |
| Prepare a team / create an Org      | `agent__prepareTeamworkWorkspace`, `agent__createOrg`, `agent__getOrg` | Used by `teamwork` and `org`                                                               |

### How children return results

- The child's **final reply text** is the result. A child's scratchpad notes are **invisible** to the parent.
- If a child only says "saved to a file" while `ISOLATED`, the parent cannot read it. Ask for the deliverable **in the final reply**, or share the folder.
- The parent treats "done / tests pass" as a **hypothesis** and checks it (see [Quality checks](#quality-checks-before-you-trust-a-result)).

### Choosing complexity

Every handoff picks `low`, `normal`, or `high`. Under **Settings → AI & Models → Sub-Agent Model Routing** you can map each level to a model, so simple lookups use a light model and hard design work uses a strong one. Unmapped levels follow the parent/default model.

### Concurrency and limits

- By default **4** sessions can be active at once (and up to **8** suspended). Parallel patterns launch work in batches to fit.
- A session lineage can carry **depth** and **fanout** (children per parent) limits. Exceeding them rejects the spawn with an error, which stops runaway chains of children creating children.

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

| Skill          | When                                                                  |
| -------------- | --------------------------------------------------------------------- |
| **`delegate`** | Baseline: create / watch / nudge children and get results             |
| **`recruit`**  | Create a new **specialist assistant** (not session delegation itself) |
| **`boost`**    | Trim or strengthen tools on an **existing** assistant                 |

```
@skill:delegate
Read only the auth module in this repo and summarize vulnerability candidates in Markdown.
```

### 2) Work-splitting (ephemeral children)

| Skill                      | Pattern        | One line                                             |
| -------------------------- | -------------- | ---------------------------------------------------- |
| **`divide-conquer`**       | Parallel split | Non-overlapping pieces → merge                       |
| **`hub-spoke`**            | Hub–spoke      | Coordinator steers workers; no worker-to-worker chat |
| **`pipeline`**             | Sequential     | A output → B input → C …                             |
| **`consensus-delegation`** | Multi-view     | Same question to several experts → parent reconciles |
| **`gatekeeper`**           | Quality gate   | Writer ↔ reviewer loop                              |
| **`pair-programming`**     | Pair           | Two roles on a **shared work folder**                |

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

| Skill                 | Role                                                                   |
| --------------------- | ---------------------------------------------------------------------- |
| **`teamwork`**        | Design the task force and roles — **prepare first**                    |
| **`org`**             | Create member sessions visible under Org. Resume from the **Org root** |
| **`org-restructure`** | Change roles or rules on an existing Org                               |

Flow: `teamwork` prepare → `org` create → continue from sidebar **Org** → change structure with `org-restructure`.

- **Org** ≠ scheduled tasks (`schedule`) ≠ in-session reminders (`loop`) ≠ wake-on-done (`call-me-back`)
- Plain parent–child delegation alone does **not** show under **Org**

```
@skill:teamwork
Create a research / implement / review task force and coordinate with hub-spoke.
```

---

## Patterns in detail

Each pattern below summarizes the procedure the bundled skill actually follows. For more end-to-end prompts see [Orchestration examples](orchestration-examples.md).

### `delegate` — the basics

1. Decide whether delegation fits (bounded scope, parallelizable, or needs a specialist)
2. **Reuse** an idle child with the same role if one exists; otherwise create one
3. Choose the child's **work folder** — isolated by default, or "share the current folder" when it must see the same files
4. Write a **self-contained brief**: objective, scope, output format, required paths
5. Check progress, re-steer if needed, then merge the result

A well-formed request:

```text
@skill:delegate
Read only the auth module (src/auth/**) and look for vulnerability candidates.
- Scope: no code changes, nothing outside src/auth
- Output: a table "severity | file:line | description | suggestion" in your final reply
- Share the current work folder
```

### `divide-conquer` — independent pieces in parallel

Six stages: **decompose → assign → spawn → monitor → merge → verify**. If pieces need each other's output, use `pipeline` instead.

- Pieces must **not overlap** (two children editing one file will collide).
- In a shared folder, give each child **unique file names or subfolders**.
- After merging, run build/tests as an **integration check** and re-run only the failed pieces.

```text
@skill:divide-conquer
Translate the 12 Korean guides in docs/ into English: three parallel batches of four,
follow docs/glossary.md as the common rule, and verify no links broke at the end.
```

### `hub-spoke` — central coordinator plus specialists

The hub owns planning, the task list, and merging. Spokes talk **only to the hub**. If B needs A's output, the hub passes a summary of A's artifacts to B.

- Keep the hub's context small: request **summaries and file lists**, not full logs.
- For code work share the hub's folder; for research or writing, take results in the final reply text.

```text
@skill:hub-spoke
Evaluate adopting a new payment module. You are the hub; spokes: ① market & pricing
② integration difficulty ③ legal & security. Get a one-page summary from each and
produce a final comparison table with a recommendation.
```

### `pipeline` — an assembly line

For `research → draft → review → polish`, each stage's output feeds the next. All stages use the **same work folder**, and the next stage receives a **summary + file paths**, not the whole conversation. A failed stage is retried on its own.

```text
@skill:pipeline
Produce a blog post on "local agent adoption stories".
Stage 1 research (5+ sources) → 2 outline & draft → 3 fact & tone review → 4 final Markdown.
Save each stage's output under drafts/.
```

### `consensus-delegation` — one question, several viewpoints

Two to four children review the **same scope with the same output template** independently (e.g. security, performance, maintainability). They never see each other; the parent compares results and follows up **only where they disagree**. There is no voting algorithm — the parent weighs the evidence.

Template every child must fill:

```text
Verdict: approve | approve-with-caveats | reject | inconclusive
Confidence: high | medium | low
Findings: - [finding]: [evidence: path, line, command output]
Risks:    - [risk]: [why it matters]
```

Tip: do not create a new assistant per viewpoint. Inject the lens in the brief ("You are reviewing as a security reviewer") and reuse existing assistants.

### `gatekeeper` — a write ↔ review quality gate

The creator submits; the reviewer **approves or rejects** against a rubric. A rejection returns the failed rules and what to fix, and the creator reworks. To avoid endless loops over style nits, the parent enforces a **maximum rework count (default 3)**.

```text
@skill:gatekeeper
Have the creator draft the release notes; the reviewer checks ① no factual errors
② user impact first ③ two sentences per item. If it fails after 3 rounds,
summarize what is still wrong.
```

### `pair-programming` — driver and navigator

Two children work in the **same folder** in turns. The driver implements and leaves a change summary; the navigator reads files or `git diff`, flags structure and bugs, and gives the next instruction. The parent relays turns and **swaps roles** when moving to tests.

```text
@skill:pair-programming
Fix the timezone bug in src/utils/date.ts. Driver implements, navigator reviews the diff.
Swap roles for the test-writing step.
```

---

## Quality checks: before you trust a result

For important work, add **`delegation-eval-loop`** after `delegate`. The **child generates; the parent decides acceptance**.

1. **Contract first** — before delegating, fix the objective, allowed/forbidden paths, checkbox **acceptance criteria** (`- [ ] tests pass`), and where the result must appear.
2. **Four layers** (fail fast on the earliest failure)

   | Layer                       | Checks                                                | How                                                  |
   | --------------------------- | ----------------------------------------------------- | ---------------------------------------------------- |
   | 1. Deterministic            | Build, types, tests, lint                             | Parent runs the same commands or inspects raw output |
   | 2. Invariant                | Edits outside allowed paths                           | `git status` / `git diff --stat`                     |
   | 3. Trajectory / reliability | Did it really run? cancelled, incomplete, no evidence | Compare result text with actions                     |
   | 4. Semantic                 | User intent and edge cases                            | Parent judgment, only after 1–3 pass                 |

3. **Reject precisely** — send the actual failure output, not "try again".
4. **Bound the loop** — after 3–5 cycles, escalate to you with diagnostics.

```text
@skill:delegate @skill:delegation-eval-loop
Fix the rounding bug in payments/.
- Allowed paths: src/payments/**, tests/payments/**  (no other edits)
- Acceptance: - [ ] pnpm test payments passes  - [ ] at least one new regression test
- Paste the commands you ran and their raw output in your final reply
```

---

## Not the same as time- and event-based features

Similar-sounding, but these are **not** delegation or Org.

| Need                                                                              | Skill              | Notes                                                                 |
| --------------------------------------------------------------------------------- | ------------------ | --------------------------------------------------------------------- |
| "Remind me in 5 minutes", "check every morning" — time-based **inside this chat** | `loop`             | Wakes this session at set times                                       |
| "Wake me when the build finishes", webhooks, ticket status                        | `call-me-back`     | Resumes this session on a **completion signal**, not the clock        |
| Cron automation that runs even with the app closed                                | `schedule`         | App-wide, session-independent ([Scheduled Tasks](scheduled-tasks.md)) |
| Long-lived multi-agent team                                                       | `teamwork` → `org` | Shown in the Org view; resumed from the root session                  |

---

## Running a persistent team (Org)

1. **`teamwork`** — first decides whether the work is genuinely multi-track (one agent is preferred if it suffices). Defines the mission, roles, and coordination model, and prepares constitution files (`MISSION.md`, `ROLES.md`, `agents.md`, `coordination/`, …) in an app-local folder. The model is one of **Sequential**, **Hub-and-spoke**, or **Swarm** (open-ended exploration from a shared board).
2. **`org`** — creates the Org once from the root session and spawns members **inside the Org**. Members inherit the team's work folder. Later you resume from the **Org root**; resuming an arbitrary child is not an Org resume.
3. **`org-restructure`** — add, remove, or merge roles, edit rules, and reassign owners without dissolving the team.
4. **`postmortem-improve`** — after a mission or repeated failures, capture lessons and route improvements to `org-restructure`, `boost`, or `recruit`.

You can move an Org to another machine with [export / import](#where-it-shows-in-the-ui).

---

## Quick picker

| Goal                                | Use                                |
| ----------------------------------- | ---------------------------------- |
| One child, get the result           | `@skill:delegate`                  |
| Parallel independent pieces         | `@skill:divide-conquer`            |
| Central coordinator + workers       | `@skill:hub-spoke`                 |
| Ordered stages                      | `@skill:pipeline`                  |
| Same topic, many viewpoints         | `@skill:consensus-delegation`      |
| Write ↔ strict review loop         | `@skill:gatekeeper`                |
| Two agents pair on code             | `@skill:pair-programming`          |
| Long-lived team + Org UI            | `@skill:teamwork` → `@skill:org`   |
| Hire a new specialist assistant     | `@skill:recruit` (then `delegate`) |
| Slim tools on an existing assistant | `@skill:boost`                     |

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
