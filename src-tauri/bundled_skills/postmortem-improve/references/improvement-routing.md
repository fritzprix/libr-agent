# Improvement Routing Matrix

Map each postmortem actionable to **one** route. Apply Top-1 enzyme first; backlog the rest.

## Primary enzyme (always consider first)

| Symptom / finding | Durable change | Route | Notes |
| --- | --- | --- | --- |
| Agent chose wrong strategy/tool/sequence; tools were honest | Update `@harness/LESSONS.active.md` | **postmortem-improve** (this skill) | Max 5 falsifiable rules; git-safe; org **or** solo |
| Tool success/status lies, schema wrong, handler bug, missing binary | Fix/bug task or code patch | **defect track** | **Forbidden** to "fix" via LESSONS |

## Secondary routes (after enzyme)

| Symptom / finding | Durable change | Skill | Notes |
| --- | --- | --- | --- |
| Role missing, duplicate, or wrong owner | Edit `ROLES.md` / `MISSION.md`; reassign KANBAN | **org-restructure** | Org must already exist |
| Constitution vague (`@teamwork/agents.md` handoff rules) | Tighten agents.md + DECISIONS | **org-restructure** | Refresh next step |
| No teamwork artifacts / wrong framework | Scaffold or re-scaffold | **teamwork** | Then **org** if lineage needed |
| Org identity / spawn / resume confusion | Fix org operating practice | **org** | Do not recreate org casually |
| Assistant has wrong/missing tools for its role | Update config tool lists | **boost** | Full replace of `externalMcpServers` |
| Needed specialist config does not exist | Create assistant | **recruit** | Inventory-first proposals |
| Child claimed done without proof | Harder briefs + parent grading | **delegation-eval-loop** | Codify in role skill / HANDOFF |
| Same class of failure every week | Recurring review wake | **schedule** or **loop** | schedule = global; loop = this chat |
| User **explicitly** wants project workspace guidelines | Patch workspace `agents.md` | **agent-init** | Dirties project git — never default |

## Decision tree

```text
Is the failure a deterministic tool/contract/environment defect?
  yes → defect track (no LESSONS.active write)
  no  → behavior track → update @harness/LESSONS.active.md (Done)

Need secondary structural/config change?
  teamwork missing and user wants multi-agent → teamwork
  org roles/constitution → org-restructure
  assistant tools → boost / recruit
  eval soft-pass habit → delegation-eval-loop
  workspace agents.md (user asked) → agent-init
  cadence only → schedule/loop
```

## Writing action ids

```markdown
- [ ] PM-A1 behavior: LESSONS rule for browser= pin - enzyme: @harness/LESSONS.active.md
- [ ] PM-A2 defect: browser createSession contract_lie - file bug (LESSONS forbidden)
```

## Anti-patterns

- Calling LESSONS Done while only writing POSTMORTEMS/ or KANBAN
- Using LESSONS to paper over `contract_lie` / handler bugs
- Defaulting to workspace `agents.md` (git dirty)
- Applying boost + recruit + org-restructure in one vague "cleanup"
- Requiring org/teamwork before solo learning can land
