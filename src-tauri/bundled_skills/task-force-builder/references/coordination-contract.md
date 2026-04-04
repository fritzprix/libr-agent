# Coordination Contract

These files are the minimum shared operating system for a durable task force.

## `MISSION.md`

Must contain:

- the overall objective
- hard constraints
- definition of done
- required deliverables

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
