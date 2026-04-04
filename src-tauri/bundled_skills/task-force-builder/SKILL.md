---
name: task-force-builder
description: Build and scaffold a multi-agent collaboration workspace with the right coordination model, shared operating files, and role-specific skills. Use when a user wants to create a team, crew, task force, asynchronous collaboration loop, specialist handoff workflow, or reusable shared workspace for multiple agents.
---

# Task Force Builder

Build a task force only when the work is genuinely multi-track. If one capable agent can finish the job cleanly, do not manufacture a committee.

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

Do not mix models unless the user explicitly needs a hybrid. Hybrid systems get messy fast.

### 3. Create the workspace contract

Create the workspace and the shared files first. At minimum create:

```text
{workspace-root}/
├── MISSION.md
├── ROLES.md
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

1. Read `MISSION.md` and `ROLES.md`.
2. Check `coordination/KANBAN.md` before starting.
3. Claim or update a task before doing meaningful work.
4. Write findings or status changes to the proper coordination file.
5. Leave a concrete handoff in `coordination/HANDOFF.md`.

Do not let agents freestyle their own coordination rules. That is how teams turn into soup.

### 6. Handle persistence honestly

If recurring execution is needed, prepare the workspace and role skills so they can be driven by scheduled sessions later. If scheduled-task creation is not available to the active agent, still build the workspace to be scheduler-ready and tell the user which loops should be scheduled manually.

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

## Bootstrap Options

For deterministic scaffolding, run:

```bash
python scripts/init_task_force.py \
  --output /absolute/path/to/workspace \
  --objective "Build a reusable research and implementation team" \
  --framework hub-and-spoke \
  --role "Coordinator:Own planning, prioritization, and integration" \
  --role "Researcher:Collect evidence and update docs/RESEARCH.md" \
  --role "Implementer:Turn approved plans into code and tests"
```

After scaffolding, edit the generated files so they match the real task instead of shipping boilerplate nonsense.

## Quick Checks Before You Finish

Before announcing success, verify:

1. the chosen framework matches the task shape
2. each role has a non-overlapping responsibility
3. the coordination files tell agents exactly where to read and write
4. every important artifact has an owner
5. the handoff path between roles is obvious

## References

- [Framework selection matrix](references/framework-selection.md)
- [Coordination file contracts](references/coordination-contract.md)
- [Expert skill template](references/expert-skill-template.md)
