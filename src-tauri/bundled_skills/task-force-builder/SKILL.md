---
name: task-force-builder
description: Build and scaffold a multi-agent collaboration workspace with the right coordination model, shared operating files, and role-specific skills. Use when a user wants to create a team, crew, task force, asynchronous collaboration loop, specialist handoff workflow, or reusable shared workspace for multiple agents, then route execution to the right teamwork skill.
---

# Task Force Builder

Build a task force only when the work is genuinely multi-track. If one capable agent can finish the job cleanly, use one agent.

## Non-Negotiable Rules

1. **One team, one workspace.** The coordinator and shared files stay in the same workspace.
2. **Scaffold in the current workspace.** Create `./agents.md`, `./MISSION.md`, `./ROLES.md`, and `./coordination/` there.
3. **Keep coordination in that same workspace.**
4. **Use deterministic scaffolding first.** `scripts/init_task_force.py` is the default path.
5. **Refresh happens later.** Changes to `agents.md` or workspace skills do not take effect in the current turn.
6. **Route execution explicitly.** Keep framework-specific operating rules in the specialist skill that matches the chosen substrate.

## Operating Rule

Start by deciding whether the request needs:

1. **No task force** - one agent can do it end-to-end.
2. **Small task force** - 2-4 specialists with explicit handoffs.
3. **Persistent task force** - a shared workspace plus recurring or resumable collaboration.

Default to the smallest structure that can succeed.

## Core Workflow

### 1. Shape the task force

Extract four things before writing files:

1. **Mission** - what must be delivered.
2. **Work shape** - linear, integration-heavy, or open-ended.
3. **Artifacts** - what files or outputs prove progress.
4. **Roles** - only the specialists that materially reduce risk.

Use the decision matrix in [framework-selection.md](references/framework-selection.md).

### 2. Choose the coordination model

Pick exactly one primary model:

- **Sequential** - use for dependency chains like research -> design -> implementation -> review.
- **Hub-and-spoke** - use when one coordinator must integrate outputs from several workers.
- **Swarm** - use when exploration is open-ended and agents can safely work from a shared board.

Use one primary model unless the task clearly needs a hybrid.

### 2.5 Choose the execution substrate separately

Coordination model and execution substrate are not the same thing.

Pick the execution substrate that matches the job:

- **Plain child sessions** - use `startSession(...)` for one-off delegation that does not need org visibility.
- **Explicit org lineage** - use `createOrg(...)` once from the root session, then use `startSession(..., includeCurrentOrg=true)` when the child should appear in Org view. Org-visible children should normally share the coordinator's workspace.
- **Scheduled task groups** - use `createScheduledTask(...)` and the other `scheduled_task` tools for recurring, heartbeat, cron-like, or resumable automation loops.

Keep these separate:

- **Org** is for explicit lineage-based teamwork and org UX.
- **Org** should normally share the coordinator's workspace so every member sees the same SSOT files.
- **Scheduled task groups** are for recurring automation and policy-governed background collaboration.
- A recurring task group may wake a coordinator session, but that does not make the scheduled group an org.

### 2.6 Route to the specialist skill

After choosing the execution substrate, route to the matching specialist skill:

- **Plain child sessions** - stay here and use `subagent-session-delegation` when child-session mechanics matter.
- **Explicit org lineage** - switch to `team-org`.
- **Scheduled task groups** - switch to `team-sprint`.

`task-force-builder` decides and scaffolds. The specialist skill handles the execution-specific operating rules.

### 3. Create the workspace contract

Create the shared files in the current workspace first. At minimum create:

```text
./
├── agents.md
├── MISSION.md
├── ROLES.md
├── .libragent/
│   └── teamwork.json
├── skills/
│   └── {role-skill}/SKILL.md
├── docs/
└── coordination/
    ├── KANBAN.md
    ├── HANDOFF.md
    ├── DECISIONS.md
    ├── RISKS.md
    └── DISCUSSION.md
```

The scaffold must preserve:

- the original user request
- the teamwork objective
- the chosen collaboration framework
- the canonical file ownership and operating rules

The coordinator continues in that same workspace after scaffolding.

Use the file contracts in [coordination-contract.md](references/coordination-contract.md).

### 4. Create role skills only when they add durable value

Generate a workspace skill per role when the role needs persistent operating guidance across many turns or sessions. If the specialization is temporary and obvious, a session-specific prompt may be enough.

When you do create role skills, use [expert-skill-template.md](references/expert-skill-template.md) and customize:

- mission slice
- allowed tool families
- required inputs
- expected outputs
- handoff targets
- stop conditions

### 5. Enforce disciplined collaboration

Every specialist must follow this loop:

1. Read `agents.md`, `MISSION.md`, and `ROLES.md`.
2. Check `coordination/KANBAN.md` before starting.
3. Claim or update a task before doing meaningful work.
4. Write findings or status changes to the proper coordination file.
5. Leave a concrete handoff in `coordination/HANDOFF.md`.

Shared files are the coordination contract. Keep the loop explicit.

### 6. Handle persistence honestly

If recurring execution is needed, switch to `team-sprint` and define the loops explicitly with the `scheduled_task` builtin tools. Use `createScheduledTask(...)` to create the first grouped loop with a clear `groupName`, then use `groupId` plus the other scheduled-task tools to extend, inspect, pause, or retune the group.

Refresh behavior:

- `agents.md` changes do **not** instantly rewrite the current session prompt
- new workspace skills apply in a later execution step, not retroactively in the same turn

After scaffolding or constitution edits, state when the updated rules become effective.

## Tool Hygiene Rules

When you execute the scaffold:

1. Bootstrap in the current workspace.
2. Use portable commands and the available tools.
3. Match the editing primitive to the change size.
4. Use `workspaceOverride` only when a child must work in a different workspace.

## Design Rules

### Keep roles sharp

- Prefer roles with one dominant responsibility.
- Give each role a clear input and output contract.
- Avoid duplicate specialists with fuzzy boundaries.
- Add a coordinator only when integration or prioritization is a real problem.

### Keep artifacts explicit

Every role should update one primary artifact. Examples:

- Researcher -> `docs/RESEARCH.md`
- Architect -> `docs/ARCHITECTURE.md`
- Implementer -> `src/` + `coordination/HANDOFF.md`
- Reviewer -> `docs/REVIEW.md`

### Keep failure visible

Force the team to record:

- blocked tasks in `KANBAN.md`
- unresolved decisions in `DECISIONS.md`
- active risks in `RISKS.md`
- next-owner handoffs in `HANDOFF.md`

If the task force cannot surface blocked state, it is not robust.

## Bootstrap

Prefer deterministic scaffolding:

```bash
python scripts/init_task_force.py \
  --output . \
  --team-name "Research Strike Team" \
  --objective "Build a reusable research and implementation team" \
  --request "Research the space, structure findings, and hand implementation-ready guidance to coding specialists." \
  --framework hub-and-spoke \
  --role "Coordinator:Own planning, prioritization, and integration" \
  --role "Researcher:Collect evidence and update docs/RESEARCH.md" \
  --role "Implementer:Turn approved plans into code and tests"
```

Use `.` for `--output` when you are scaffolding the current teamwork run.

Then review and tighten:

- `agents.md`
- `MISSION.md`
- `ROLES.md`
- `.libragent/teamwork.json`

The scaffold is the constitution. Tighten it before handing work to specialists.

## Quick Checks Before You Finish

Before announcing success, verify:

1. the chosen framework matches the task shape
2. the original user request is preserved in the scaffold
3. each role has a non-overlapping responsibility
4. `agents.md` tells agents exactly where to read and write
5. every important artifact has an owner
6. the handoff path between roles is obvious
7. the execution substrate and follow-up specialist skill are explicit: plain child sessions, `team-org`, or `team-sprint`
8. refresh semantics are written down so agents know that updated rules apply only in a later execution step
9. the governing session is still working in the workspace where it created the constitution

## References

- [Framework selection matrix](references/framework-selection.md)
- [Coordination file contracts](references/coordination-contract.md)
- [Expert skill template](references/expert-skill-template.md)
