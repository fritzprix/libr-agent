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
2. Read `.libragent/teamwork.json` before acting.
   - Confirm `executionSubstrate.mode` is `"org"` and `orgLineage.intended` is `true`.
   - If the manifest says a different substrate, reconcile before proceeding.
3. Keep one shared workspace.
   - Treat the coordinator's current workspace as the SSOT.
   - Org-visible children should normally work in that same workspace.
4. Create the org once from the root session.
   - Use `createOrg(orgName, description)` from the governing root session.
   - Record the returned `orgId` and `orgName` in `.libragent/teamwork.json` and `coordination/DECISIONS.md`.
   - The session that calls `createOrg` becomes the org root. Do not call `createOrg` again.
5. Spawn org-visible members explicitly.
   - Use `startSession(agentId, task, includeCurrentOrg=true, workspaceOverride=<coordinator-workspace>)` for org-visible children.
   - Treat `spawnOrgAgent(...)` as a compatibility alias, not the primary path.
   - One-off delegated children that should stay out of Org view are plain child sessions — omit `includeCurrentOrg`.
6. Resume through the org root.
   - The org root session is the canonical entry point. Org view should resume the root, not whichever child was last active.
   - If you need to identify the root, read `orgLineage.rootSessionId` from `.libragent/teamwork.json`.
7. Keep work anchored in the workspace constitution.
   - Read `agents.md`, `MISSION.md`, `ROLES.md`, and the coordination files before directing members.
   - Org context refines execution; it does not replace the shared workspace contract.

## Guardrails

- Do not split org members into separate workspaces unless the task truly needs it.
- Do not use org identity for scheduled task groups or recurring automation.
- Do not treat arbitrary child-session resume as org resume. The org root is the entry point.
- Do not infer org membership from parent/child lineage alone — membership requires `includeCurrentOrg=true` at session creation.
- After `createOrg`, update `.libragent/teamwork.json` with the actual `orgId` and `rootSessionId`.

## References

- [Org patterns and tool call examples](references/org-patterns.md)
