# Teamwork Refactor Contract

Use this file as the binding contract for the Team Workspace / org / scheduled-group refactor.

## 1. Team Workspace contract

The Team Workspace is the durable collaboration substrate.

Canonical teamwork information must be scaffolded into the workspace:

- original user request
- teamwork objective
- role definitions
- workspace instructions
- progress and handoff state

Canonical teamwork state should live in files such as:

- `agents.md`
- `MISSION.md`
- `ROLES.md`
- `coordination/KANBAN.md`
- `coordination/HANDOFF.md`
- `coordination/DECISIONS.md`

The backend may validate, index, and surface this state, but it should not replace the workspace scaffold as the canonical teamwork constitution.

## 2. Meta-meta-skill contract

`teamwork` is not just a folder generator.

It must:

1. decide whether a team is needed
2. decide which collaboration pattern fits
3. scaffold the workspace
4. write `agents.md`
5. encode the teamwork constitution

That means the skill, not backend tables, decides what the teamwork operating model looks like for a given task.

## 3. Org contract

`org` applies only to lineage-based sub-agent teamwork.

But that is no longer enough as a filtering rule by itself.

Use org for:

- coordinator / specialist structures
- parent-child delegation
- durable organization identity
- dedicated org-oriented UI

Org membership must be **explicitly created and persisted**.

Minimum direction:

- `org_id`
- `org_name`
- `org_root_session_id`

Implications:

- a generic sub-agent lineage is **not** automatically an org
- `/org` must show only lineages created through the explicit org path
- a session having `lineage_id` or `parent_session_id` alone is insufficient to qualify
- org view should resume the `org_root_session_id`, not whichever child card was clicked

Minimal org tool direction:

- `createOrg`
- `spawnOrgAgent`
- optional `getOrg`

Do not use org for:

- flat scheduled automation
- generic workspace grouping
- scheduled task identity
- inferred lineage-only grouping without explicit org creation

## 4. Scheduled collaboration contract

Scheduled collaboration uses **scheduled task groups**, not org identity.

Scheduled task groups are for:

- recurring automation bundles
- horizontal or loosely coupled periodic work
- grouped status display in scheduled task UX

They are not a substitute for org identity.

Minimum model direction:

- `groupId`
- `groupName`
- optional role/kind metadata
- provenance

## 5. Operational trigger pattern

Scheduling may still wake a master agent. That is allowed.

Pattern:

1. scheduled task wakes a master agent
2. master checks whether teamwork scaffolding exists
3. if missing, master performs team setup
4. if present, master reads SSOT files and issues directives

This is an operational pattern only. It does not make scheduled groups into orgs.

## 6. Governance contract

Recurring teamwork must be governed through:

- Settings UX
- backend enforcement

Current agreed policy direction:

- user-configurable minimum interval
- user-configurable maximum scheduled task groups
- current preferred cap direction: max 10 groups

Agents may create/manage recurring automation if policy allows. Ordinary agents are not categorically blocked, but they must pass the same backend-enforced limits.

## 7. UX contract

Scheduled Tasks UX:

- group-centric sections/cards for scheduled task groups
- standalone section for non-grouped tasks

Org UX:

- dedicated org surface, not generic session history with a small filter tweak
- org-card / org-chart flavor
- selecting an org should resume the root session
- one-off delegated child sessions must stay out of org view

## 8. Regression contract

Regression coverage should lock these exact promises:

- scheduled collaboration remains grouped automation, not org
- standalone tasks stay outside scheduled groups
- org view excludes one-off sub-agent lineages
- org view includes only explicitly org-created lineages
- org selection resumes the org root session
