---
name: consensus-delegation
description: Orchestrate multiple specialized child sessions to evaluate the same question from different perspectives, collect their independent conclusions, and reconcile disagreements through parent-mediated follow-ups. Use for code review triangulation, architecture or risk assessment, security vs performance tradeoff analysis, or any bounded decision that benefits from 2-4 distinct expert viewpoints—not for persistent teams (teamwork/org) or single one-off delegation (delegate).
---

# Consensus via Delegation

Use this skill when one parent session should **compare independent expert opinions** on the same scoped question, then synthesize a final answer.

This is parent-mediated orchestration: child sessions do not talk to each other. The parent spawns specialists, collects results, detects disagreement, and sends targeted follow-ups.

For general spawn/monitor mechanics and workspace isolation rules, follow `delegate` first. Read this skill when you need the **multi-reviewer workflow**, not a single child task.

## When to Use

Good fits:

- 2-4 specialists reviewing the **same** change, design, or proposal
- architecture evaluation with distinct lenses (structure, security, performance)
- risk analysis where tradeoffs must be explicit
- bounded questions with a clear deliverable the parent can compare

Avoid this skill when:

- one capable agent can finish cleanly alone → stay in the parent session
- you need a persistent multi-agent workspace → use `teamwork` or `org`
- children must debate each other directly → not supported; parent must mediate
- the work is open-ended implementation with no comparison step → use `delegate` once

## Core Rules

1. **Same question, same scope.** Every child must receive the same objective, boundaries, and output shape. Change only the perspective or assistant specialization.
2. **Shared evidence when code matters.** If reviewers must inspect the same repo, use `workspaceOverride` with the parent's effective workspace path—or paste critical excerpts into every handoff. Children do not inherit parent workspace or `agents.md` automatically.
3. **Parent is the hub.** Only the parent can `agent__checkSession` or `agent__messageToSession` its descendants. Sibling sessions cannot see or message each other.
4. **Structured handoffs, human synthesis.** Ask each child for a fixed section layout (verdict, findings, risks, confidence). The parent compares text results; there is no built-in vote or merge algorithm.
5. **Do not blindly trust children.** Verify file paths, claims, and scope before presenting a final answer.

## Workflow

### 1. Frame the decision

Define before spawning anything:

- the exact question or artifact under review
- in-scope paths/modules and hard boundaries
- the output sections every child must return
- what counts as agreement vs material disagreement

### 2. Pick specialists

- `agent__list(type="configs")` to discover assistants; use returned IDs in `agent__startSession`
- Prefer assistants whose system prompts or assistant-scoped skills match each perspective
- If no suitable assistant exists, `agent__create` a config first, then spawn with the new ID
- Keep the panel small (usually 2-4). More reviewers add noise and hit fanout/concurrency limits

### 3. Prepare identical handoffs

Each task should differ only in **review lens**, not in scope or deliverable format.

Include:

- shared objective and scope
- shared output template (see `references/workflow-templates.md`)
- critical workspace rules copied from the parent when needed
- absolute paths or identifiers the child must use

When all reviewers need the same codebase, start each child with the same `workspaceOverride`.

### 4. Spawn and run

Default pattern:

1. `agent__startSession(agentId="...", task="...", waitForResult=false, workspaceOverride="...")` for each specialist
2. `agent__list(type="sessions")` if you need to confirm child IDs and status—this does not return review results
3. Collect with `agent__checkSession(sessionId)` or `agent__checkSession(sessionId, wait=true)` for actual conclusions

Notes:

- Prefer `waitForResult=false` on spawn so multiple children can run while the parent plans the synthesis
- Tool calls in one turn execute sequentially; children still run in parallel once started, subject to the global active-session slot limit (default 4, configurable via `maxConcurrentActiveSessions`)
- While `agent__checkSession(wait=true)` blocks, the parent releases its active slot so children can finish—use waits deliberately

### 5. Compare results

For each child result, extract:

- verdict or recommendation
- top findings with evidence (paths, symbols, commands)
- risks and confidence level

Flag **material disagreement** when verdicts conflict or findings contradict on the same fact. Minor wording differences are not disagreement.

### 6. Reconcile (only when needed)

When perspectives conflict:

1. State the specific conflict (fact, tradeoff, or recommendation)
2. `agent__compactSessionContext(sessionId)` on children with long histories before large follow-ups
3. `agent__messageToSession` each relevant child with the opposing finding and ask for a focused re-check
4. `agent__checkSession(..., wait=true)` again after follow-ups
5. Stop after 1-2 reconciliation rounds unless new blocking facts appear

Read `references/reconciliation.md` for round templates and stop conditions.

### 7. Synthesize in the parent

Produce the final answer yourself:

- summarize agreed facts
- explain unresolved tradeoffs
- state your recommendation and what each perspective contributed
- cite which child sessions supported each conclusion

Do not present a single child's output as the final answer without comparison.

## Builtin Tools

| Step | Tool |
| --- | --- |
| Discover assistants | `agent__list(type="configs")` |
| Create missing specialist | `agent__create(...)` |
| Spawn reviewer | `agent__startSession(..., waitForResult=false)` |
| Inspect children | `agent__list(type="sessions")` — IDs and status only |
| Poll or wait | `agent__checkSession(sessionId)` / `agent__checkSession(sessionId, wait=true)` — results |
| Reconcile | `agent__messageToSession(sessionId, message)` |
| Stop stuck work | `agent__stopSession(sessionId)` |
| Long histories | `agent__compactSessionContext(sessionId)` |

## Common Mistakes

- **Different scopes per child** — comparisons become meaningless; keep scope identical
- **Assuming shared workspace without `workspaceOverride`** — reviewers inspect different files
- **Expecting children to respond to each other** — repackage opposing views in the parent message
- **Skipping output format** — parent cannot compare unstructured prose reliably
- **Trusting verdicts without evidence** — re-open files or traces before deciding
- **Blocking the parent on every spawn** — use async spawn + targeted waits
- **Ignoring paused/error sessions** — recover with `agent__messageToSession`, not another blind `agent__checkSession`

## References

- `references/workflow-templates.md` — handoff and output templates by scenario
- `references/reconciliation.md` — disagreement handling, follow-up rounds, stop rules
