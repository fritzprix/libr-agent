# Org Patterns

Use this file for concrete tool call patterns, manifest update rules, and troubleshooting when running explicit org-based teamwork.

## Quick Reference

| Need | Tool / action |
| --- | --- |
| Create the org (once, from root session) | `agent__createOrg(name="...")` |
| Spawn an org-visible child session | `agent__startSession(agentId, task)` from a session already in the org |
| Delegate outside Org view | `agent__startSession(agentId, task)` from a session that is not in an explicit org |
| Identify the org root session | Read `orgLineage.rootSessionId` from `.libragent/teamwork.json` |
| Resume org work | Resume the session matching `orgLineage.rootSessionId`, not a child |
| Inspect org membership | `agent__getOrg(orgId)` if available |

## Pattern: Create Org and First Member

```
// Step 0 — from the governing root session
agent__prepareTeamworkWorkspace()
// -> returns "/absolute/path/to/teamwork-artifacts"

// Step 1 — from the root session
agent__createOrg(
  name: "Research Strike Team"
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
agent__startSession(
  agentId: "<researcher-assistant-id>",
  task: "..."
)
```

## Pattern: Resume From Org Root

When a user clicks on the org or resumes teamwork, the entry point is always the root session.

```
// Read root from manifest
rootSessionId = teamwork.json → executionSubstrate.orgLineage.rootSessionId

// Resume the root, not whichever child was last active
agent__messageToSession(rootSessionId, "Continue with the next phase of the objective.")
```

Do not resume a child session directly and treat it as the org coordinator. Children may lack the full workspace constitution context.

## Pattern: Keep Workspace Shared

Org-visible children should inherit the parent effective workspace unless there is a concrete reason to diverge.

```
agent__startSession(agentId: "...", task: "...")
```

If a child starts outside the org inheritance path, it gets its own workspace and will not automatically see the same implementation context unless you explicitly pass `workspaceOverride`.

## Pattern: Per-Session agents.md and SOUL.md in Org

When org members need different operating rules (e.g., frontend specialist vs. backend specialist) or custom personas, use `workspaceOverride` to give each child its own workspace directory containing a custom `agents.md` or `SOUL.md`.

```
// Each specialist gets its own workspace directory containing a custom agents.md / SOUL.md
agent__startSession(
  agentId: "frontend-expert",
  task: "Implement the React login component",
  workspaceOverride: "/shared-workspace/frontend/"
)
// -> /shared-workspace/frontend/agents.md becomes this child's workspace instructions.
```

> [!WARNING]
> `workspaceOverride` is dual-purpose: it controls both the code output location AND the instruction sources (`agents.md`, `SOUL.md`). If you want different instructions but want to share the codebase:
> 1. Set the child's `workspaceOverride` to a specific subdirectory (e.g. `/shared-workspace/frontend/`) to isolate the instruction scope.
> 2. Direct the agent in its task description to perform operations or read files from the parent's shared codebase directory (e.g., `../src/`).
>
> If the override directory does not contain `agents.md` or `SOUL.md`, the backend will fall back to loading **no rules or persona** for those parts. Do not forget to scaffold or copy these files if custom rules are expected.

## Manifest Update After Org Creation

The backend automatically updates the scaffolded `.libragent/teamwork.json` file with the org identity fields returned by `agent__createOrg`. You do not need to edit it manually. The final manifest will look like this:

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
      "explicitOrgLineage": "inherit-governing-session-workspace-by-default",
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
      "rootAction": "agent__createOrg",
      "childAction": "agent__startSession",
      "childArgs": {},
      "compatibilityAlias": "spawnOrgAgent",
      "workspaceSharing": "inherit-parent-workspace-by-default"
    },
    "scheduledTaskGroups": {
      "intended": false,
      "notes": "Use scheduled task groups for recurring or cron-like collaboration, not org lineage."
    }
  },
  "refreshSemantics": {
    "workspaceInstructions": "Changes to agents.md and related workspace constitution files apply in a later execution step, not in the current turn.",
    "workspaceSkills": "New workspace skills apply in a later execution step, not retroactively in the same turn."
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

Likely cause: child was started outside the org inheritance path or with the wrong `workspaceOverride`.

Fix: restart the child under the org root so it inherits the parent effective workspace, or pass the correct `workspaceOverride`, or copy critical rules into the task text.

### Org view shows unexpected sessions

Likely cause: sessions inherited explicit org membership unintentionally, or lineage-only sessions are being surfaced incorrectly.

Fix: spawn non-org children from a session that is not part of an explicit org. Org membership still requires explicit org inheritance at session creation, not just having a `parentSessionId`.

### Cannot identify org root

Fix: read `executionSubstrate.orgLineage.rootSessionId` from `.libragent/teamwork.json`. If the manifest does not have it, check `coordination/DECISIONS.md` for the org creation record.
