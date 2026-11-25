---
name: org
description: "Run, restructure, or schedule an explicit LibrAgent org: durable org identity, org-visible members, and root-session resume. Use to create or run an org, add/remove/merge roles or update its constitution, or run the org on a recurring tick (orch loop). Triggers: org, 조직, org restructure, add role, layoff role, orch loop, run the org continuously."
---

# Org

An org is a long-lived team: one root session and a stable set of members, reused across runs.

Use `teamwork` first when the workspace constitution is not ready.
Use `postmortem-improve` after missions or repeated failures to capture lessons and route durable org improvements.

## Choose the procedure

| Need | Procedure |
| --- | --- |
| Create the org, spawn or reuse members, resume through the root | Workflow below |
| The org exists and roles or constitution files must change (add, layoff, merge roles; `agents.md` / `MISSION.md` updates; retire members) | [org-restructure.md](references/org-restructure.md) |
| The org must keep working on a recurring tick: the root acts as PM and hands board items to members | [orch-loop.md](references/orch-loop.md) |

Do not restructure during bootstrap; create the org first. Start an orch loop only after the board and roles exist.

## Workflow

1. Confirm org is the right substrate.
   - Use org when the user wants org visibility, durable org identity, coordinator/specialist lineage, or root-session resume behavior.
   - If the real need is app-wide recurring or cron-like automation, stop and use `schedule`.
   - If the user only wants a clock delay or reminder inside the current conversation, stop and use `loop`.
   - If they want to resume when a process or ticket completes, stop and use `call-me-back`.
   - If the governing root session has not prepared the teamwork artifact directory yet, stop and use `teamwork` to call `agent__prepareTeamworkWorkspace()` first.
2. Read `.libragent/teamwork.json` before acting.
   - Confirm `executionSubstrate.mode` is `"org"` and `executionSubstrate.orgLineage.intended` is `true`.
   - If the manifest says a different substrate, reconcile before proceeding.
3. Keep one shared workspace.
   - Treat the app-local teamwork artifact directory as the SSOT for orchestration files.
   - The governing root session and org-visible children should keep the normal parent/override workspace inheritance model.
4. Create the org once from the root session.
   - Use `agent__createOrg(name="...")` from the governing root session.
   - Record the returned `orgId` and `orgName` in `coordination/DECISIONS.md` (the backend automatically updates `.libragent/teamwork.json` with these details).
   - The session that calls `agent__createOrg` becomes the org root. Do not call `agent__createOrg` again.
5. Spawn org-visible members explicitly.
   - Before creating a member for new work, inspect `agent__listAgents(type="sessions")` for an Idle org child with the same assistant ID and compatible workspace, then use `agent__messageToSession(sessionId, complexity, message, reset=true)` when reuse is safe. Always pass `complexity` (`low`|`normal`|`high`).
   - Use `agent__spawnSession(configId, complexity, task)` for org-visible children only when no suitable member exists, a different role/workspace is needed, or additional parallel capacity is required. Always pass `complexity` (`low`|`normal`|`high`). If the current session already belongs to the org, inheritance is automatic.
   - To keep a child out of Org view, spawn it from a session that is not part of an explicit org.
6. Resume through the org root.
   - The org root session is the canonical entry point. Org view should resume the root, not whichever child was last active.
   - If you need to identify the root, read `executionSubstrate.orgLineage.rootSessionId` from `.libragent/teamwork.json`.
7. Keep work anchored in the workspace constitution.
   - Read `agents.md`, `MISSION.md`, `ROLES.md`, and the coordination files before directing members.
   - Org context refines execution; it does not replace the shared workspace contract.

## Guardrails

- Do not invent a separate org-only workspace. Org members should inherit the parent effective workspace unless a task explicitly needs a different `workspaceOverride`.
- Do not treat arbitrary child-session resume as org resume. The org root is the entry point.
- Do not infer org membership from parent/child lineage alone — membership requires explicit org inheritance at session creation. Under an explicit org root that inheritance is automatic.
- After `agent__createOrg`, verify `.libragent/teamwork.json` contains the updated `orgId` and `rootSessionId` (written automatically by the backend).

## References

- [Org patterns and tool call examples](references/org-patterns.md)
- [Org restructure](references/org-restructure.md) — [impact analysis](references/org-restructure-impact-analysis.md), [layoff](references/org-restructure-layoff-checklist.md), [merge](references/org-restructure-merge-checklist.md), [constitution update](references/org-restructure-constitution-update.md)
- [Orch loop (scheduled org tick)](references/orch-loop.md)
- After-action learning / continuous improvement: use bundled skill `postmortem-improve`
