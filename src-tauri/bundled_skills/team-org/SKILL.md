---
name: team-org
description: Run explicit org-based teamwork in LibrAgent. Use when collaboration needs durable org identity, org-visible child sessions, org-root resume behavior, or clear parent/sibling org context under one coordinator workspace.
---

# Team Org

Org is explicit lineage teamwork. It is not generic delegation and it is not scheduled automation.

Use `task-force-builder` first when the workspace constitution is not ready.

## Workflow

1. Confirm org is the right substrate.
   - Use org when the user wants org visibility, durable org identity, coordinator/specialist lineage, or root-session resume behavior.
   - If the real need is recurring or cron-like automation, stop and use `team-sprint`.
2. Keep one shared workspace.
   - Treat the coordinator's current workspace as the SSOT.
   - Org-visible children should normally work in that same workspace.
3. Create the org once.
   - Use `createOrg(...)` from the governing root session.
4. Spawn org-visible members explicitly.
   - Use `startSession(..., includeCurrentOrg=true)` for org-visible children.
   - Treat `spawnOrgAgent(...)` as a compatibility alias, not the primary path.
5. Keep lineage honest.
   - One-off delegated children that should stay out of Org view are plain child sessions, not org members.
   - Do not infer org membership from parent/child lineage alone.
6. Keep work anchored in the workspace constitution.
   - Read `agents.md`, `MISSION.md`, `ROLES.md`, and the coordination files before directing members.
   - Parent/sibling org context refines execution; it does not replace the shared workspace contract.

## Guardrails

- Do not split org members into separate workspaces unless the task truly needs it.
- Do not use org identity for scheduled task groups or recurring automation.
- Do not treat arbitrary child-session resume as org resume. The org root is the entry point.
