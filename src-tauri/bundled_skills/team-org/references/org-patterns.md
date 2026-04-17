# Org Patterns

Use this file for concrete tool call patterns, manifest update rules, and troubleshooting when running explicit org-based teamwork.

## Quick Reference

| Need | Tool / action |
| --- | --- |
| Create the org (once, from root session) | `createOrg(orgName, description)` |
| Spawn an org-visible child session | `startSession(agentId, task, includeCurrentOrg=true, workspaceOverride=<path>)` |
| Spawn a one-off child that stays out of Org view | `startSession(agentId, task)` — no `includeCurrentOrg` |
| Compatibility alias for org-visible spawn | `spawnOrgAgent(agentId, task, workspaceOverride=<path>)` |
| Identify the org root session | Read `orgLineage.rootSessionId` from `.libragent/teamwork.json` |
| Resume org work | Resume the session matching `orgLineage.rootSessionId`, not a child |
| Inspect org membership | `getOrg(orgId)` if available |

## Pattern: Create Org and First Member

```
// Step 1 — from the root session
createOrg(
  orgName: "Research Strike Team",
  description: "Coordinator + specialist org for the current research objective"
)
// → returns { orgId: "abc-123", ... }

// Step 2 — update .libragent/teamwork.json with orgId and rootSessionId
// executionSubstrate.orgLineage.orgId = "abc-123"
// executionSubstrate.orgLineage.rootSessionId = <current session id>

// Step 3 — spawn an org-visible researcher
startSession(
  agentId: "<researcher-assistant-id>",
  task: "...",
  includeCurrentOrg: true,
  workspaceOverride: "<coordinator-workspace-path>"
)
```

## Pattern: Resume From Org Root

When a user clicks on the org or resumes teamwork, the entry point is always the root session.

```
// Read root from manifest
rootSessionId = teamwork.json → executionSubstrate.orgLineage.rootSessionId

// Resume the root, not whichever child was last active
messageToSession(rootSessionId, "Continue with the next phase of the objective.")
```

Do not resume a child session directly and treat it as the org coordinator. Children may lack the full workspace constitution context.

## Pattern: Keep Workspace Shared

Org-visible children should work in the coordinator's workspace unless there is a concrete reason to diverge.

```
startSession(
  agentId: "...",
  task: "...",
  includeCurrentOrg: true,
  workspaceOverride: "/absolute/path/to/coordinator/workspace"
)
```

If a child starts without `workspaceOverride`, it gets its own workspace and will not automatically see `agents.md`, `MISSION.md`, or role skills.

## Manifest Update After Org Creation

After calling `createOrg`, update `.libragent/teamwork.json`:

```json
{
  "executionSubstrate": {
    "mode": "org",
    "specialistSkill": "team-org",
    "orgLineage": {
      "intended": true,
      "orgId": "<returned-org-id>",
      "orgName": "<org-name>",
      "rootSessionId": "<current-session-id>",
      "rootAction": "createOrg",
      "childAction": "startSession",
      "childArgs": { "includeCurrentOrg": true },
      "compatibilityAlias": "spawnOrgAgent",
      "workspaceSharing": "inherit-root-workspace-by-default"
    }
  }
}
```

Also record the org creation decision in `coordination/DECISIONS.md`:

```markdown
## Decision: Org created
- orgId: <id>
- orgName: <name>
- rootSessionId: <id>
- Reason: explicit org lineage required for org-visible specialist coordination
```

## Troubleshooting

### Child session cannot see workspace constitution

Likely cause: child started without `workspaceOverride`.

Fix: restart child with `workspaceOverride` pointing to the coordinator workspace, or copy critical rules into the task text.

### Org view shows unexpected sessions

Likely cause: sessions were created with `includeCurrentOrg=true` unintentionally, or lineage-only sessions are being surfaced incorrectly.

Fix: confirm that sessions not intended for Org view were started without `includeCurrentOrg`. Org membership requires explicit opt-in, not just having a `parentSessionId`.

### Cannot identify org root

Fix: read `executionSubstrate.orgLineage.rootSessionId` from `.libragent/teamwork.json`. If the manifest does not have it, check `coordination/DECISIONS.md` for the org creation record.
