# Gatekeeper Troubleshooting

## Symptom: endless rework loops

**Cause:** Criteria too vague, or parent did not enforce rework limit.

**Fix:**

1. Tighten [criteria.md](criteria.md) with pass/fail checks, not style opinions.
2. Parent tracks `reworkCount`; stop at limit (default 3) and escalate to user.
3. On reject, require **numbered failed criteria** — not open-ended "improve quality."

## Symptom: reviewer approves low-quality work

**Cause:** Reviewer lacks concrete checklist or cannot see changed files.

**Fix:**

- Give reviewer explicit file paths and diff commands.
- Require evidence per criterion (test output, lint result, screenshot path).

## Symptom: creator ignores feedback

**Cause:** Feedback not delivered via `agent__messageToSession`, or creator session ended.

**Fix:**

- Parent forwards structured reject payload to creator session ID.
- Include: failed criteria, file paths, required next action.

## Symptom: context bloat between loops

**Cause:** Full chat history passed each rework.

**Fix:**

- Pass only: criteria snapshot, latest diff summary, failed items, target files.
- See [rework-limit-examples.md](rework-limit-examples.md).

## Escalation template

When rework limit is hit, parent messages user:

```markdown
Gatekeeper loop stopped after N reworks.
- Last reject reason: ...
- Remaining failed criteria: ...
- Options: relax criteria / manual review / ship with known gaps
```
