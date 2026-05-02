# Org Patterns

Use this file for concrete tool call patterns, manifest update rules, and troubleshooting when running explicit org-based teamwork.

## Quick Reference

| Need | Tool / action |
| --- | --- |
| Create the org (once, from root session) | `createOrg(orgName, description)` |
| Spawn an org-visible child session | `startSession(agentId, task, workspaceOverride=<path>)` |
| Spawn a one-off child that stays out of Org view | `startSession(agentId, task, includeCurrentOrg=false)` |
| Identify the org root session | Read `orgLineage.rootSessionId` from `.libragent/teamwork.json` |
| Resume org work | Resume the session matching `orgLineage.rootSessionId`, not a child |
| Inspect org membership | `getOrg(orgId)` if available |

## Pattern: Create Org and First Member

```
// Step 0 — from the governing root session
prepareTeamworkWorkspace()
// -> returns "/absolute/path/to/teamwork-workspace"

// Step 1 — from the root session
createOrg(
  orgName: "Research Strike Team",
  description: "Coordinator + specialist org for the current research objective"
)
// -> returns {
//      orgId: "abc-123",
//      orgName: "Research Strike Team",
//      orgRootSessionId: "<current-session-id>",
//      teamworkScaffold: { ... }
//    }

// Step 2 — update .libragent/teamwork.json with orgId, orgName, and rootSessionId
// executionSubstrate.orgLineage.orgId = "abc-123"
// executionSubstrate.orgLineage.orgName = "Research Strike Team"
// executionSubstrate.orgLineage.rootSessionId = <returned orgRootSessionId>

// Step 3 — spawn an org-visible researcher
startSession(
  agentId: "<researcher-assistant-id>",
  task: "...",
  workspaceOverride: "<teamwork-workspace-path>"
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

Org-visible children should work in the dedicated teamwork workspace unless there is a concrete reason to diverge.

```
startSession(
  agentId: "...",
  task: "...",
  workspaceOverride: "/absolute/path/to/teamwork-workspace"
)
```

If a child starts without `workspaceOverride`, it gets its own workspace and will not automatically see the shared teamwork constitution unless org inheritance already points it at the root teamwork workspace.

## Manifest Update After Org Creation

After calling `createOrg`, update the scaffolded `.libragent/teamwork.json` instead of replacing it with a smaller hand-written object. Preserve the existing scaffold fields such as `workspacePolicy`, `specialistSkills`, `refreshSemantics`, and `constitutionAdoption`, then fill in the org identity fields returned by `createOrg`.

```json
{
  "schemaVersion": 2,
  "teamName": "<team-name>",
  "objective": "<objective>",
  "originalUserRequest": "<original-user-request>",
  "framework": "<framework>",
  "executionSubstrate": {
    "mode": "org",
    "specialistSkill": "org",
    "workspacePolicy": {
      "plainChildSessions": "isolated-by-default",
      "explicitOrgLineage": "share-governing-teamwork-workspace-by-default",
      "scheduledTaskGroups": "workspace-defined-per-group"
    },
    "specialistSkills": {
      "plainChildSessions": "delegate",
      "explicitOrgLineage": "org",
      "scheduledTaskGroups": "schedule"
    },
    "orgLineage": {
      "intended": true,
      "orgId": "<returned-org-id>",
      "orgName": "<org-name>",
      "rootSessionId": "<returned-orgRootSessionId>",
      "rootAction": "createOrg",
      "childAction": "startSession",
      "childArgs": {
        "includeCurrentOrg": true
      },
      "compatibilityAlias": "spawnOrgAgent",
      "workspaceSharing": "inherit-root-teamwork-workspace-by-default"
    },
    "scheduledTaskGroups": {
      "intended": false,
      "notes": "Use scheduled task groups for recurring or cron-like collaboration, not org lineage."
    }
  },
  "refreshSemantics": {
    "workspaceInstructions": "Changes to agents.md and related workspace constitution files apply in a later execution step, not in the current turn.",
    "workspaceSkills": "New workspace skills apply in a later execution step, not retroactively in the same turn."
  },
  "constitutionAdoption": {
    "coordinatorMustShareScaffoldRoot": true,
    "rule": "Continue coordination in the dedicated teamwork workspace where the constitution was created."
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

Fix: restart child with `workspaceOverride` pointing to the dedicated teamwork workspace, or copy critical rules into the task text.

### Org view shows unexpected sessions

Likely cause: sessions inherited explicit org membership unintentionally, or lineage-only sessions are being surfaced incorrectly.

Fix: confirm that sessions not intended for Org view were started with `includeCurrentOrg=false`. Org membership still requires explicit org inheritance, not just having a `parentSessionId`.

### Cannot identify org root

Fix: read `executionSubstrate.orgLineage.rootSessionId` from `.libragent/teamwork.json`. If the manifest does not have it, check `coordination/DECISIONS.md` for the org creation record.
