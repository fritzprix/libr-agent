# Rework Limit Examples

Parent session owns the counter. Default **max reworks: 3**.

## Counter pattern (parent responsibility)

Track per artifact or per creator session:

```text
reworkCount = 0
MAX_REWORKS = 3
```

On **reject** from reviewer:

1. If `reworkCount >= MAX_REWORKS` → escalate to user (do not message creator again).
2. Else → `reworkCount += 1`, forward reject via `agent__messageToSession`.

On **approve** → reset `reworkCount` for next artifact.

## Reject message shape

Send creator actionable, bounded feedback:

```markdown
## Gatekeeper Reject (attempt 2/3)

Failed criteria:
1. [C2] Unit tests missing for auth module
2. [C4] No error handling on network timeout

Files to fix:
- src/auth/login.ts
- src/auth/login.test.ts

Required before resubmit:
- Add tests covering invalid token + timeout
- Run: pnpm test:run -- src/auth/login.test.ts
```

## Approve message shape

```markdown
## Gatekeeper Approve

All criteria passed. Ship artifact.
Summary: ...
```

## When to raise MAX_REWORKS

Only when user explicitly allows extended polish (e.g. security-critical review). Document in `coordination/DECISIONS.md` for org work.
