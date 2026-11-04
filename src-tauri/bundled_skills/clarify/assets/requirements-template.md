# Requirements Document Template

Copy this skeleton when you create `requirements/<slug>.md`. Replace the bracketed placeholders. Delete a section only if it does not apply, and record the deletion in the Change Log.

```markdown
---
title: <Title>
version: 0.1
status: draft        # draft | in-review | final
updated: YYYY-MM-DD
---

# <Title> — Requirements

## 1. Problem Statement & Goal
<One paragraph: what problem, for whom, what success looks like.>

## 2. Target Users / Audience / Stakeholders
<Who uses, attends, reads, or approves the result; primary vs secondary.>

## 3. Scope
### In Scope
- <testable requirement statements>
### Non-Goals (Explicitly Out of Scope)
- <what this work will NOT do>

## 4. Core Requirements
<R1, R2, ... — each a testable statement.>

## 5. Deliverables & Materials
<What is produced (documents, event program, purchased items, software) and
what is needed to produce it (materials, sources, venues, equipment).>
<Software work only: inputs, outputs, key data entities, interfaces.>

## 6. Constraints & Dependencies
<Budget, schedule, people, tools, regulations; dependencies on other people,
teams, or systems.>

## 7. Open Questions
<Unresolved items, each with an owner (user/agent) and impact. This list
should shrink to empty.>

## 8. Decisions Log
| Date | Decision | Rationale | Version |
| ---- | -------- | --------- | ------- |

## 9. Change Log
| Version | Date | Summary |
| ------- | ---- | ------- |
| 0.1 | YYYY-MM-DD | Initial draft from first conversation |
```

## Editing Rules

- Sections 1–7 are the living body; sections 8–9 are append-only history.
- When a decision changes an earlier one, edit the body **and** add an entry: `superseded by vX.Y (<short reason>)`.
- `status` moves `draft` → `in-review` (when peer review starts) → `final` (after the user accepts).
- Keep the doc under about 500 lines. Move long detail (tables, diagrams) into an appendix at the bottom.
