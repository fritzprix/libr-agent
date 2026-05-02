---
name: org
description: Run explicit org-based teamwork in LibrAgent. Use when collaboration needs durable org identity, org-visible child sessions, org-root resume behavior, or clear parent/sibling org context while preserving normal parent workspace inheritance.
---

# Org

Org is explicit lineage teamwork. It is not generic delegation and it is not scheduled automation.

Use `teamwork` first when the workspace constitution is not ready.

## Workflow

1. Confirm org is the right substrate.
   - Use org when the user wants org visibility, durable org identity, coordinator/specialist lineage, or root-session resume behavior.
   - If the real need is recurring or cron-like automation, stop and use `schedule`.
   - If the governing root session has not prepared the teamwork artifact directory yet, stop and use `teamwork` to call `prepareTeamworkWorkspace()` first.
2. Read `.libragent/teamwork.json` before acting.
   - Confirm `executionSubstrate.mode` is `"org"` and `orgLineage.intended` is `true`.
   - If the manifest says a different substrate, reconcile before proceeding.
3. Keep one shared workspace.
   - Treat the app-local teamwork artifact directory as the SSOT for orchestration files.
   - The governing root session and org-visible children should keep the normal parent/override workspace inheritance model.
4. Create the org once from the root session.
   - Use `createOrg(name="...")` from the governing root session.
   - Record the returned `orgId` and `orgName` in `.libragent/teamwork.json` and `coordination/DECISIONS.md`.
   - The session that calls `createOrg` becomes the org root. Do not call `createOrg` again.
5. Spawn org-visible members explicitly.
   - Use `startSession(agentId, task)` for org-visible children when you want the default parent-workspace inheritance. If the current session already belongs to the org, inheritance is automatic.
   - One-off delegated children that should stay out of Org view must set `includeCurrentOrg=false`.
6. Resume through the org root.
   - The org root session is the canonical entry point. Org view should resume the root, not whichever child was last active.
   - If you need to identify the root, read `orgLineage.rootSessionId` from `.libragent/teamwork.json`.
7. Keep work anchored in the workspace constitution.
   - Read `agents.md`, `MISSION.md`, `ROLES.md`, and the coordination files before directing members.
   - Org context refines execution; it does not replace the shared workspace contract.

## Guardrails

- Do not invent a separate org-only workspace. Org members should inherit the parent effective workspace unless a task explicitly needs a different `workspaceOverride`.
- Do not use org identity for scheduled task groups or recurring automation.
- Do not treat arbitrary child-session resume as org resume. The org root is the entry point.
- Do not infer org membership from parent/child lineage alone — membership requires explicit org inheritance at session creation. Under an explicit org root that inheritance is automatic unless `includeCurrentOrg=false`.
- After `createOrg`, update `.libragent/teamwork.json` with the actual `orgId` and `rootSessionId`.

## References

- [Org patterns and tool call examples](references/org-patterns.md)
