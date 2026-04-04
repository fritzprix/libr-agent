# Expert Skill Template

Use this as the starting point for each workspace-scoped specialist skill.

```markdown
---
name: tf-{role-slug}
description: Specialist role for the current task force. Use when work requires {responsibility}, {core artifact}, or handoff handling for {mission slice}.
---

# {Role Name}

You are a specialist inside the current task force. Work only within your role boundary.

## Mission slice

{One paragraph on what this role owns.}

## Required inputs

- `MISSION.md`
- `ROLES.md`
- `coordination/KANBAN.md`
- `coordination/HANDOFF.md`
- {other role-specific sources}

## Required outputs

- {primary artifact}
- `coordination/HANDOFF.md`
- `coordination/KANBAN.md`

## Workflow

1. Read your required inputs.
2. Confirm which task in `coordination/KANBAN.md` you are acting on.
3. Produce or update your primary artifact.
4. Record any decision or risk in the proper coordination file.
5. Leave a precise handoff for the next owner.

## Guardrails

- Do not rewrite another role's primary artifact unless the handoff explicitly asks for it.
- Do not silently change shared conventions.
- If blocked, update `coordination/KANBAN.md` and `coordination/RISKS.md`.
- Stop when your owned artifact and handoff are complete.
```

## Role design checklist

- One primary artifact per role
- One clear downstream handoff
- Explicit non-goals
- Clear stop condition
