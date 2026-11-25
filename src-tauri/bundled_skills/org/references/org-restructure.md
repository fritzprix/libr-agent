# Org Restructure (org reference)

Tune an existing explicit org. This is evolution, not creation and not teardown.

Use the main `org` workflow when the org does not exist yet or you need runtime coordination (spawn, resume, getOrg).
Use `teamwork` when the teamwork artifact directory or constitution files do not exist yet.
Use `postmortem-improve` when the change set comes from an after-action review (postmortem → classify → then restructure/boost/recruit).

## Path conventions

Teamwork files live in the app-local artifact directory (Teamwork SSOT). Access them via:

- `@teamwork/...` aliases in agent tools
- `./.libragent/teamwork/...` from shell in the org root workspace

Do not edit repo-root copies unless the user explicitly wants repo pollution.

## Workflow

1. **Confirm this is restructure, not bootstrap.**
   - `.libragent/teamwork.json` must exist with `executionSubstrate.mode: "org"` and `executionSubstrate.orgLineage.intended: true`.
   - An `orgId` must already be set (from `agent__createOrg` or manifest).
   - If not, stop and use `teamwork`, then the main `org` workflow.
2. **Run pre-flight audit** — see [impact-analysis.md](org-restructure-impact-analysis.md).
3. **Pick one primary operation** (add, layoff, merge, constitution update, child removal). Load the matching reference checklist.
4. **Apply constitution + coordination changes** in dependency order (decision record → ROLES/MISSION/agents → KANBAN/HANDOFF → sessions).
5. **Propagate refresh** — constitution edits apply on a later execution step, not the current turn.
6. **Verify** with `agent__getOrg` and a quick coordination-file scan.

## Operation routing

| User intent | Read first | Primary edits |
| --- | --- | --- |
| Add a role | [constitution-update.md](org-restructure-constitution-update.md) | ROLES.md, MISSION.md, optional `skills/tf-*`, spawn child |
| Remove a role (layoff) | [layoff-checklist.md](org-restructure-layoff-checklist.md) | ROLES.md, KANBAN owners, HANDOFF, child sessions |
| Merge duplicate roles | [merge-checklist.md](org-restructure-merge-checklist.md) | ROLES.md, MISSION.md, skills, session mapping |
| Change operating rules | [constitution-update.md](org-restructure-constitution-update.md) | agents.md, DECISIONS.md |
| Remove child from org view | [layoff-checklist.md](org-restructure-layoff-checklist.md) § Child sessions | stop/delete session; no detach API yet |

## Core rules

1. **ROLES.md is the role contract; MISSION.md must stay in sync** when roles change. Update both in the same change set.
2. **Record every structural change** in `@teamwork/coordination/DECISIONS.md` before or alongside file edits.
3. **Reassign before you delete.** Move KANBAN owners and HANDOFF targets off a removed role before layoff or merge.
4. **Prefer org root for coordination.** Resume or message the root (`executionSubstrate.orgLineage.rootSessionId`) to broadcast constitution refresh.
5. **Do not call `agent__createOrg` again.** Restructure edits files and sessions; it does not recreate org identity.
6. **Refresh semantics:** after `agents.md` / `ROLES.md` edits, tell active members the new rules apply on their next execution step.

## Child session removal (current platform limits)

There is no `agent__detachFromOrg` tool today. To remove a session from org view:

- **Retire work:** `agent__stopSession(sessionId)` then `agent__deleteSession(sessionId)` if permanent removal is OK.
- **Keep work, hide from org:** not supported — document the limitation and ask the user to choose stop/delete or leave the session idle.

## Guardrails

- Do not dissolve the org, delete the org root, or archive the entire teamwork artifact directory — out of scope.
- Do not remove the org root session from the org; the root is the canonical entry point.
- Do not edit only ROLES.md while leaving stale role sections in MISSION.md.
- Do not lay off a role while its tasks remain assigned to that role in KANBAN.md.
- Do not merge roles without picking one canonical agent ID and one primary artifact owner per merged responsibility.

## References

- [Impact analysis (pre-flight)](org-restructure-impact-analysis.md)
- [Layoff checklist](org-restructure-layoff-checklist.md)
- [Merge checklist](org-restructure-merge-checklist.md)
- [Constitution update](org-restructure-constitution-update.md)
