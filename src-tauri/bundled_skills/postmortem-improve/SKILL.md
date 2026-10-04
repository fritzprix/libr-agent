---
name: postmortem-improve
description: >
  Run a postmortem → improve loop so LibrAgent sessions learn without dirtying
  project git. Capture evidence-backed failures, then apply a Top-1 update to
  `@harness/LESSONS.active.md` (app-local; injected into the system prompt) for
  behavioral defects, or route deterministic tool/code defects to a fix/bug task
  (never paper over with lessons). Works for solo sessions and org/teamwork.
  Use after incidents, repeated failures, mission retros, stuck KANBAN/Blocked
  items, or when the user asks for continuous improvement / after-action review.
  Not for one-off task execution, initial team bootstrap (use teamwork), or
  first-time org create (use org).
---

# Postmortem → Improve

Close the loop: **observe failure → write truth → change what the next prompt sees**.

Primary enzyme (org **or** solo):

```text
@harness/LESSONS.active.md
```

App-local under `{appData}/harness-lessons/<scopeId>/` (outside project git).  
Scope: `org-<orgRootSessionId>` when in org lineage, else `ws-<workspaceHash>`.  
Harness injects non-empty content into system prompt as `## Active Operational Lessons` on the next LLM turn.

Specialist skills own optional secondary mutations (boost, org-restructure, …).  
Do not invent a parallel org model.

## Not This Skill

| Skill | Use for |
| --- | --- |
| **teamwork** | First-time scaffold / choose substrate |
| **org** | Create org, spawn/resume org members |
| **org-restructure** | Apply role/constitution structural changes |
| **delegation-eval-loop** | Grade one child sprint (not fleet learning) |
| **boost** / **recruit** | Tune or create assistant configs only |
| **schedule** / **loop** | Recurring wake-ups (cadence only) |
| **agent-init** | Workspace `agents.md` (dirties project git — **not** default here) |

## Core Rules

1. **Evidence before narrative.** Prefer tool errors, session status, diffs, coordination files over memory.
2. **Two-track triage (mandatory).**
   - **Deterministic / tool / contract defect** (`contract_lie`, wrong schema, handler bug, missing binary): **do not** write LESSONS. File a fix/bug task or apply a product patch if the user asked for code changes.
   - **Behavioral / decision failure**: update `@harness/LESSONS.active.md` with falsifiable rules.
3. **Prompt enzyme or it did not happen.** Chat-only conclusions and KANBAN-only backlog are **deferred**, not Done.
4. **Git-clean by default.** Never treat workspace `agents.md` / `SOUL.md` / tracked repo edits as the learning channel unless the user explicitly asks.
5. **Bounded ambition.** Max **5** active lesson rules; ≤30 lines / 2KB (harness truncates beyond that). Evict before adding a 6th.
6. **Write from the governing session only.** Harness **rejects** `@harness` writes from children (`parent_session_id` / depth>0) and from org members whose `org_root_session_id !=` this session. Propose candidate rules in final text; root merges into `@harness/LESSONS.active.md`.

## When to Run

- A mission, sprint, or objective finished (success or failure)
- The same failure class repeats (≥2 times)
- User asks for postmortem, retro, after-action, continuous improvement, or "learn from this"
- Optional: org/teamwork with stuck `coordination/KANBAN.md` Blocked items

Teamwork/org is **optional**. Solo sessions skip `@teamwork` archive steps.

## Workflow

### 1. Gather evidence (short)

Collect only what supports root-cause claims:

- Failed acceptance criteria / eval rejects
- Session terminals: cancelled, timeout, circuit-break, empty final text
- Tool/config mismatches via `agent__listAgents` / `tool__listServers` when relevant
- Optional: session trace excerpts — do not dump entire traces into the archive

If `@teamwork/` exists, also skim `MISSION.md` → `ROLES.md` → `agents.md` → `KANBAN.md` → `HANDOFF.md`.

### 2. Write the archive (optional but recommended)

**With teamwork:** `@teamwork/coordination/POSTMORTEMS/YYYY-MM-DD-<slug>.md`  
**Solo / no teamwork:** `@harness/POSTMORTEMS/YYYY-MM-DD-<slug>.md`

Use [postmortem-template.md](references/postmortem-template.md).

Optional index line in `@harness/LESSONS.md` (or `@teamwork/coordination/LESSONS.md` if teamwork exists):

```markdown
- YYYY-MM-DD — <slug> — <one-line lesson> — track: behavior|defect
```

### 3. Triage Top-1

Pick **one** primary actionable. Classify:

| Track | Meaning | Done when |
| --- | --- | --- |
| `defect` | Tool/schema/handler/environment contract broken | Bug/fix task filed **or** code patched (user-authorized); **LESSONS write forbidden** |
| `behavior` | Agent chose wrong strategy/tool/sequence despite honest tools | `@harness/LESSONS.active.md` updated with ≤5 structured rules |

Routing helpers (secondary, after enzyme): [improvement-routing.md](references/improvement-routing.md).

### 4. Apply (behavior track)

Read current `@harness/LESSONS.active.md` (may be missing). Merge Top-1 rule(s).

Rule syntax (one line each):

```markdown
- [YYYY-MM-DD] TRIGGER: <when> | FORBIDDEN: <anti-pattern> | REQUIRED: <correct action>
```

Constraints:

- Plain text only — no HTML/XML, no "ignore previous instructions"
- Prefer concrete tool names / args / acceptance checks
- If already at 5 rules, remove or merge the least relevant before adding

Write with either:
- `workspace__writeFile` to `@harness/LESSONS.active.md` with **`mode: "overwrite"`** (default `create` renames to `LESSONS.active-1.md` and the prompt will ignore it), or
- `workspace__editFile` / replaceLines on the existing file.

Never rely on default create-mode for updates.

### 5. Verify learning stuck

Before declaring the cycle complete:

- [ ] Archive exists (or user waived) with evidence pointers
- [ ] Track decided: `defect` **or** `behavior`
- [ ] **behavior:** `@harness/LESSONS.active.md` updated; next turn should show `## Active Operational Lessons` in system prompt
- [ ] **defect:** LESSONS not used as a workaround; fix/bug path recorded
- [ ] No learning left only in chat
- [ ] Workspace git not dirtied for the learning step

### 6. Optional secondary routes

After the enzyme step, top remaining actions may go to boost / org-restructure / delegation-eval-loop / schedule — see routing table. Keep ≤2 secondary actions.

## Cadence (optional)

- After each major mission: run this skill even on success (near-misses)
- Weekly: scan open defect tasks + LESSONS.active staleness

Use **schedule** for org-wide cadence; **loop** only for in-conversation reminders.

## Guardrails

- Do not rewrite history to protect a role — blame systems (contracts, tools, handoffs).
- Do not dissolve the org or recreate `agent__createOrg` as "improvement".
- Do not apply every idea — prefer reversible, measurable changes.
- Do not use LESSONS to paper over broken tools.
- Do not edit workspace `agents.md` as the default learning path.

## References

- [Postmortem template](references/postmortem-template.md)
- [Improvement routing matrix](references/improvement-routing.md)
- [LESSONS.active format](references/lessons-active.md)
