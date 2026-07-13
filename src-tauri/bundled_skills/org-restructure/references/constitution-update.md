# Constitution Update

Change org operating rules or add a role without dissolving the org.

## agents.md updates

Use when changing read/write rules, handoff discipline, execution substrate notes, or anti-conflict rules.

1. Read current `@teamwork/agents.md` and `.libragent/teamwork.json` refresh semantics.
2. Append decision to `@teamwork/coordination/DECISIONS.md` with reason and impact.
3. Edit `agents.md` surgically — preserve unrelated sections.
4. Message org root: summarize what changed; rules apply **next execution step**, not current turn.
5. If children use `workspaceOverride`, check whether their local `agents.md` needs the same edit.

Do not use repo-root `agents.md` unless that is the actual teamwork artifact path.

## Add a role

1. Complete impact analysis — confirm the role does not already exist.
2. Record decision in DECISIONS.md.
3. Add section to `ROLES.md` (ID, tools, I/O, handoff).
4. Add matching subsection to `MISSION.md` under `## Roles`.
5. Create `skills/tf-<slug>/SKILL.md` if the role needs durable guidance (see teamwork expert-skill template).
6. Add backlog tasks in `KANBAN.md` with the new role as owner where appropriate.
7. Spawn member from org root: `agent__startSession(agentId, task)` with task citing ROLES.md and first KANBAN item.
8. Verify with `agent__getOrg()`.

## MISSION.md updates

Update when objective, deliverables, constraints, or role roster changes:

- Layoff/merge: always sync with ROLES.md in the same commit/edit batch
- Objective shift: record in DECISIONS.md; check KANBAN still aligns with definition of done

## ROLES.md ↔ MISSION.md sync rule

**ROLES.md** = detailed contract (tools, IDs, handoffs).
**MISSION.md** = mission-level role summary.

Every role in one file must appear in the other. On any restructure, diff both files before finishing.

## DECISIONS.md template (structure changes)

```markdown
## Decision: <short title>
- Date:
- Change type: add role | layoff | merge | constitution
- Files changed: agents.md, ROLES.md, ...
- Sessions affected:
- Effective: next execution step (constitution refresh)
- Revisit when:
```

## After edits

- Root resume or `agent__messageToSession` to active members
- `agent__getOrg()` for membership sanity
- Grep coordination files for stale role names
