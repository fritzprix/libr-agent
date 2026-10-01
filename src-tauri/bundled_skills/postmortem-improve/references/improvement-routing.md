# Improvement Routing Matrix

Map each postmortem actionable to **one** specialist skill. Apply top actions; backlog the rest.

## Routing table

| Symptom / finding | Durable change | Skill | Notes |
| --- | --- | --- | --- |
| Role missing, duplicate, or wrong owner | Edit `ROLES.md` / `MISSION.md`; reassign KANBAN | **org-restructure** | Org must already exist |
| Constitution vague (`agents.md` handoff rules) | Tighten agents.md + DECISIONS | **org-restructure** | Refresh next step |
| No teamwork artifacts / wrong framework | Scaffold or re-scaffold | **teamwork** | Then **org** if lineage needed |
| Org identity / spawn / resume confusion | Fix org operating practice | **org** | Do not recreate org casually |
| Assistant has wrong/missing tools for its role | Update config tool lists | **boost** | Full replace of `externalMcpServers` |
| Needed specialist config does not exist | Create assistant | **recruit** | Inventory-first proposals |
| Non-org workspace guidelines outdated | Regenerate/patch workspace docs | **agent-init** | Not for LibrAgent harness facts |
| Child claimed done without proof | Harder briefs + parent grading | **delegation-eval-loop** | Codify in role skill / HANDOFF |
| Same class of failure every week | Recurring review wake | **schedule** or **loop** | schedule = global; loop = this chat |
| Role needs durable operating guidance | Add `skills/tf-*/SKILL.md` | **teamwork** template + **org-restructure** | Expert skill template |

## Decision tree

```text
Is the teamwork artifact directory ready?
  no  → teamwork (then return here)
  yes → Is the change structural (roles / constitution / org children)?
          yes → org-restructure (use org only for spawn/resume mechanics)
          no  → Is it an assistant config inventory problem?
                  yes → existing config? boost : recruit
                  no  → Is it eval / acceptance soft-pass?
                          yes → delegation-eval-loop
                          no  → Is it workspace agents.md outside org artifacts?
                                  yes → agent-init
                                  no  → cadence only? schedule/loop
```

## Writing KANBAN actions

```markdown
## Backlog
- [ ] PM-A1 Tighten Implementer acceptance criteria in role skill - owner: coordinator - skill: delegation-eval-loop
- [ ] PM-A2 Remove github MCP from Finance assistant - owner: coordinator - skill: boost
```

Prefix ids with `PM-` + postmortem actionable id so LESSONS.md can track closure.

## Anti-patterns

- Applying boost + recruit + org-restructure in one undifferentiated "cleanup"
- Editing only chat memory without POSTMORTEMS/ + DECISIONS.md
- Calling `agent__createOrg` again to "reset culture"
- Scheduling retros without ever writing a postmortem file
- Changing ROLES.md without updating MISSION.md / KANBAN owners (org-restructure owns that order)
