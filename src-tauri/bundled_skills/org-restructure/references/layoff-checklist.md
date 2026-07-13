# Layoff Checklist

Remove a role from a living org without dissolving the org.

## When to use

- User wants to drop a specialist role (e.g. "we don't need a separate researcher anymore")
- Role scope was absorbed by the coordinator
- Duplicate role is being removed in favor of a merge (see [merge-checklist.md](merge-checklist.md) instead if combining)

## Steps

### 1. Record the decision

Append to `@teamwork/coordination/DECISIONS.md`:

```markdown
## Decision: Role layoff — <Role Name>
- Date:
- Removed role: <Role Name>
- Reason:
- Tasks reassigned to: <Role Name or coordinator>
- Sessions affected: <session ids or "none active">
- Skills retired: skills/tf-<slug>/ (or "kept for reference")
```

### 2. Reassign open work

In `@teamwork/coordination/KANBAN.md`:

- Find all tasks owned by the laid-off role
- Reassign `owner:` to coordinator or surviving role
- Move blocked items to **Blocked** with updated owner if needed

In `@teamwork/coordination/HANDOFF.md`:

- Add an entry routing in-flight work to the new owner
- Do not delete historical handoffs

### 3. Update constitution (both files)

**`ROLES.md`:** Remove the role section (or mark `## <Role> (retired)` only if user wants audit trail — prefer full removal + DECISIONS record).

**`MISSION.md`:** Remove the matching role subsection under `## Roles`.

Keep `agents.md` in sync only if it lists roles by name or points to removed skills.

### 4. Role skill

- **Delete** `skills/tf-<slug>/` if the role is gone permanently, or
- Add a one-line deprecated notice in the skill frontmatter if the user wants to keep history

### 5. Child sessions

For each active session mapped to this role:

| Goal | Action |
| --- | --- |
| Work complete, remove from org view | `agent__stopSession` → `agent__deleteSession` |
| Work incomplete, reassign | `agent__messageToSession` with new owner instructions; stop old session if redundant |
| Detach but keep session | **Not supported** — inform user |

Never delete the **org root** session.

### 6. Broadcast refresh

Message the org root (or `agent__messageToSession` to active children):

- Which role was removed
- New owner for former tasks
- Constitution files changed; rules apply on next execution step

### 7. Verify

- `agent__getOrg()` — laid-off sessions gone or stopped
- Grep KANBAN/HANDOFF for old role name
- ROLES.md and MISSION.md agree on remaining roles

## Child sessions (org view only)

Same session rules as step 5. There is no detach-from-org API; deletion is the only way to remove a session from org listing today.
