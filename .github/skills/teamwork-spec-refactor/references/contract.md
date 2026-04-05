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

`task-force-builder` is not just a folder generator.

It must:

1. decide whether a team is needed
2. decide which collaboration pattern fits
3. scaffold the workspace
4. write `agents.md`
5. encode the teamwork constitution

That means the skill, not backend tables, decides what the teamwork operating model looks like for a given task.

## 3. Org contract

`org` applies only to lineage-based sub-agent teamwork.

Use org for:

- coordinator / specialist structures
- parent-child delegation
- durable organization identity
- dedicated org-oriented UI

Do not use org for:

- flat scheduled automation
- generic workspace grouping
- scheduled task identity

## 4. Scheduled collaboration contract

Scheduled collaboration uses **scheduled task groups**, not org identity.

Scheduled task groups are for:

- recurring automation bundles
- horizontal or loosely coupled periodic work
- grouped status display in scheduled task UX

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
