# Reconciliation Guide

Use after the first collection pass when specialist verdicts or material findings conflict.

## What Counts as Disagreement

Treat as **material**:

- approve vs reject on the same scoped question
- contradictory factual claims about the same file, API, or behavior
- one reviewer flags a blocking risk another marks as negligible without evidence

Treat as **non-material**:

- different emphasis or ordering
- extra non-blocking suggestions
- confidence differences with the same verdict

## Parent-Mediated Follow-Up

Children cannot message each other. The parent must:

1. Quote the specific conflict
2. Include both sides with session attribution
3. Ask for a focused re-check—not a full re-review

Template:

```text
Reconciliation round — focused re-check only.

Conflict:
- Session [A-id / perspective]: [claim + evidence]
- Session [B-id / perspective]: [claim + evidence]

Your task:
- Re-examine only the disputed point within the original scope
- Return the same output format as before
- Update verdict only if this conflict changes your conclusion
- If unchanged, explain why the opposing view is wrong or out of scope
```

Send tailored messages to each involved child. If a child already has a long history from round 1, call `agent__compactSessionContext(sessionId)` before sending a large follow-up. Then `agent__checkSession(sessionId, wait=true)` for each.

## Round Limits

Default:

- **Round 1:** independent reviews
- **Round 2:** one targeted reconciliation pass for material conflicts only
- **Stop** unless a new blocking fact appears

Escalate to the user when:

- verdicts still conflict after reconciliation
- evidence is missing from both sides
- scope was insufficient to decide

Do not loop indefinitely. `agent__messageToSession` has no round cap, but cost, timeouts, and context grow quickly.

## Operational Constraints

Keep in mind during multi-child workflows:

- **Fanout:** `maxFanout` on the parent lineage can cap how many direct children you may spawn
- **Depth:** nested delegation consumes `maxDepth`; prefer flat panels under one parent
- **Concurrency:** global active-session slots are capped (default 4, configurable via `maxConcurrentActiveSessions`); async spawn + selective waits is safer than blocking on every child up front
- **Timeouts:** `agent__checkSession(wait=true)` and `agent__messageToSession` waits honor a timeout (default up to 3600 seconds)
- **Truncation:** terminal `agent__checkSession` summarizes recent assistant output; ask for full detail via `agent__messageToSession` when results are long
- **Context growth:** deep reconciliation rounds accumulate history; use `agent__compactSessionContext(sessionId)` before large follow-ups to keep token costs reasonable

## Paused, Error, or Terminated Children

If `agent__checkSession` reports paused, error, or terminated state:

- do not treat the panel as complete
- use `agent__messageToSession` with a recovery instruction referencing the last stable step
- re-check after the child reaches a terminal state

If a child is stuck or no longer needed:

- `agent__stopSession(sessionId)` to free resources
- either replace that perspective or proceed with fewer reviewers and note the gap in the final synthesis

## Final Synthesis Checklist

Before answering the user:

- [ ] All reviewers used the same scope and output format
- [ ] Material conflicts were reconciled or explicitly marked unresolved
- [ ] Evidence was verified for blocking findings
- [ ] Final recommendation states tradeoffs, not just a majority verdict
- [ ] Child session IDs or perspectives are traceable in the explanation
