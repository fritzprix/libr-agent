# Impact Analysis (Pre-flight)

Run this audit before any org restructure. Do not skip when the change looks "small."

## 1. Org identity

```
agent__getOrg()                    # or agent__getOrg(orgId="...")
```

Capture:

- `orgId`, `orgName`, `rootSessionId`
- All member sessions (id, status, assistant/agent label)
- Which sessions are root vs children

Map each active child to a **role name** by cross-checking:

- `@teamwork/ROLES.md` agent IDs
- Session `assistantId` / task text / recent HANDOFF entries

There is no automatic role↔session API. Infer from ROLES.md IDs and session metadata.

## 2. Constitution files

Read in the teamwork artifact root:

| File | Check for |
| --- | --- |
| `agents.md` | Operating rules, file paths, anti-conflict rules |
| `MISSION.md` | Role list, deliverables, constraints |
| `ROLES.md` | Per-role ID, tools, inputs/outputs, handoffs |
| `.libragent/teamwork.json` | `orgLineage`, refresh semantics |

Flag **drift**: role in ROLES.md but missing/different in MISSION.md.

## 3. Coordination references

| File | Search for removed/merged role names |
| --- | --- |
| `coordination/KANBAN.md` | `owner:` lines |
| `coordination/HANDOFF.md` | role names in headings and body |
| `coordination/DECISIONS.md` | prior structure decisions |
| `coordination/RISKS.md` | `Owner:` fields |

## 4. Role skills

List `skills/tf-*/SKILL.md` (or custom role skill dirs referenced in ROLES.md).

Note which skill directory maps to which role — layoff/merge must update or retire these.

## 5. Session workspace overrides

For each org child from `getOrg`:

- If spawned with `workspaceOverride`, it may have a **different** `agents.md` / `SOUL.md`.
- Constitution changes in the shared artifact root do not automatically change override workspaces.
- Plan separate edits or respawn if override dirs need updating.

## 6. Output: change plan

Before editing, write a short plan (in chat or DISCUSSION.md):

1. Operation type (add / layoff / merge / constitution / child removal)
2. Files to touch (ordered)
3. Sessions to stop, delete, respawn, or message
4. KANBAN/HANDOFF reassignments
5. Who needs a refresh notice (usually all active org members via root)
