# Coordination Contract

These files are the minimum shared operating system for a durable task force.

## `agents.md`

This file is the workspace constitution.

It lives in the workspace where the governing session is working. Keep coordination in that same workspace.

It should state:

- the objective
- the original user request
- the collaboration model
- the execution substrate
- the canonical files
- the read/write rules
- the handoff discipline
- the anti-conflict rules
- the refresh/resume rules

If this file is missing or vague, the team will improvise. Improvised teamwork is how you get expensive nonsense.

For execution substrate, make the contract explicit:

- plain child sessions via `startSession(...)`, with `subagent-session-delegation` for delegation mechanics when needed
- explicit org lineage via `createOrg(...)` + `startSession(...)`, then `team-org` for org-specific operating rules
- scheduled task groups for recurring automation via `createScheduledTask(...)` and related `scheduled_task` tools, then `team-sprint` for scheduled-group operating rules

Make that choice explicit.

For explicit org lineage, default to the coordinator's workspace as the shared collaboration substrate.

Refresh rule: edits to `agents.md` apply in a later execution step, not in the current turn.

## `MISSION.md`

Must contain:

- the team identity
- the overall objective
- the original user request
- hard constraints
- definition of done
- required deliverables
- execution mode notes if the team will later be driven by explicit org lineage or scheduled task groups

If the mission is vague, every downstream file becomes vague too.

## `ROLES.md`

Use one section per role:

```markdown
## Researcher
- Mission slice: Gather external evidence for the current objective
- Reads: MISSION.md, coordination/KANBAN.md, coordination/HANDOFF.md
- Writes: docs/RESEARCH.md, coordination/HANDOFF.md
- Does not own: architecture decisions, final prioritization
```

## `coordination/KANBAN.md`

Use simple sections:

```markdown
# KANBAN

## Backlog
- [ ] Validate competitor list - owner: unassigned

## In Progress
- [ ] Gather market data - owner: researcher

## Blocked
- [ ] Draft launch memo - owner: writer - blocked on pricing decision

## Done
- [x] Define report sections - owner: coordinator
```

Every meaningful task must show owner and state. Unowned work does not exist.

## `coordination/HANDOFF.md`

Use append-only entries:

```markdown
## 2026-04-04 implementer -> reviewer
- Completed: Added retry handling and tests
- Changed files: src/service.ts, src/service.test.ts
- Needs review on: error message wording and timeout thresholds
- Next recommended step: run targeted regression tests
```

## `coordination/DECISIONS.md`

Record decisions that change downstream work:

```markdown
## Decision: Use hub-and-spoke model
- Date:
- Reason:
- Impact:
- Revisit when:
```

## `coordination/RISKS.md`

Track concrete risks, not drama:

```markdown
## Risk: External API rate limiting
- Severity: high
- Owner: coordinator
- Trigger: research step exceeds quota
- Mitigation: cache results and batch queries
```

## `coordination/DISCUSSION.md`

Use this for short-lived reasoning, unresolved questions, or cross-role notes that do not belong in the canonical docs.

Do not dump final decisions here. Promote durable conclusions into `DECISIONS.md`.

## `.libragent/teamwork.json`

This manifest should expose the execution contract in machine-readable form.

At minimum capture:

- the original user request
- the collaboration model
- the execution substrate
- the workspace sharing policy
- whether explicit org lineage is intended
- whether scheduled task groups are intended
- refresh semantics notes for downstream sessions
- whether the coordinator must rebind or resume in the workspace where the constitution now lives before continuing
