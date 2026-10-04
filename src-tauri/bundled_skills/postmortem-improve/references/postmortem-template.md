# Postmortem Template

Save as:

- With teamwork: `@teamwork/coordination/POSTMORTEMS/YYYY-MM-DD-<slug>.md`
- Solo: `@harness/POSTMORTEMS/YYYY-MM-DD-<slug>.md`

Keep it short. Every claim needs a pointer (file path, session id, command, or quote).
This archive is **not** auto-injected into the prompt — learning lands in `@harness/LESSONS.active.md`.

```markdown
# Postmortem: <slug>

- Date: YYYY-MM-DD
- Mission / objective:
- Severity: low | medium | high
- Outcome: success | partial | failure
- Facilitator session: <session id>
- Track (Top-1): behavior | defect

## Timeline (compressed)

1. <t0> — what started
2. <t1> — first friction / error
3. <t2> — decision or workaround
4. <t3> — terminal state

## What happened

2–5 sentences. Facts only.

## Evidence

- Sessions: `<sessionId>` status / final text excerpt (≤10 lines)
- Tools / configs: server ids or assistant ids if relevant
- Coordination (if any): `KANBAN.md` / `HANDOFF.md` lines
- Commands / diffs: exit codes, authorized-path violations

## Impact

- User / mission impact:
- Wasted retries / cycles:

## Root causes (systems, not people)

List 1–3 causes. Prefer contract gaps:

- Soft acceptance / no eval layers
- Tool inventory mismatch
- Schema/guidance mismatch (`schema_guidance`)
- Contract lie / handler bug (`defect` track)
- Handoff without artifact path
- Wrong strategy despite honest tools (`behavior` track)

## What went well

1–3 items worth preserving.

## Actionables

| ID | Track | Change | Owner | Target | Done when |
| --- | --- | --- | --- | --- | --- |
| A1 | behavior | … | coordinator | @harness/LESSONS.active.md | rule landed; next prompt shows Active Operational Lessons |
| A2 | defect | … | coordinator | bug/fix task | LESSONS **not** used as workaround |

## Decision

Top-1 = A? — applied / deferred (why)
```
