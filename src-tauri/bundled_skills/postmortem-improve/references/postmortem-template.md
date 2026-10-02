# Postmortem Template

Save as `@teamwork/coordination/POSTMORTEMS/YYYY-MM-DD-<slug>.md`.

Keep it short. Every claim needs a pointer (file path, session id, command, or quote).

```markdown
# Postmortem: <slug>

- Date: YYYY-MM-DD
- Mission / objective: <from MISSION.md>
- Severity: low | medium | high
- Outcome: success | partial | failure
- Facilitator session: <org root or coordinator session id>

## Timeline (compressed)

1. <t0> — what started
2. <t1> — first friction / error
3. <t2> — decision or workaround
4. <t3> — terminal state

## What happened

2–5 sentences. Facts only.

## Evidence

- Coordination: `KANBAN.md` / `HANDOFF.md` / `RISKS.md` lines or sections
- Sessions: `<sessionId>` status / final text excerpt (≤10 lines)
- Tools / configs: server ids or assistant ids if relevant
- Commands / diffs: exit codes, authorized-path violations

## Impact

- User / mission impact:
- Wasted retries / cycles:
- Wrong artifacts produced:

## Root causes (systems, not people)

List 1–3 causes. Prefer contract gaps:

- Missing or vague role ownership
- Wrong execution substrate
- Soft acceptance / no eval layers
- Tool inventory mismatch
- Handoff without artifact path
- Constitution stale after prior change

## What went well

1–3 items worth preserving (so the next change does not delete them).

## Actionables

| ID | Change | Owner role | Target skill | Done when |
| --- | --- | --- | --- | --- |
| A1 | … | coordinator | org-restructure | ROLES.md + DECISIONS updated |
| A2 | … | coordinator | boost | assistant externalMcpServers match role |
| A3 | … | implementer | delegation-eval-loop | next brief has checkbox criteria |

## Non-goals this cycle

Items deferred on purpose (link KANBAN backlog ids).
```

## Facilitation notes

- Prefer the **org root** (or teamwork coordinator) to author the postmortem.
- Quarantine blame language; rewrite as contract/tool/process gaps before saving.
- If evidence is missing, say so — do not invent a root cause.
