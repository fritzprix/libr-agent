# Authorization scopes

Default when the user does not name a scope: **`read-only`**.

| Scope | User must say (examples) | GitHub | Delegate children | Local artifacts |
| --- | --- | --- | --- | --- |
| `read-only` | “plan만”, “dry-run”, “read only” | Read/search only | No | `pins.json`, hints, draft plan |
| `plan` | “케이스 목록 만들어” | Read/search only | No | `plan.md`, `dedupe.md` draft |
| `github-write` | “tracking 이슈 올려”, “create/update the ticket” | Create/update **one** tracking issue + result comments | No | Same + issue URL |
| `execute-e2e` | “self E2E 실행”, “unattended run” | Read (optional comment if also `github-write`) | Yes, serialized | Full case evidence |

Scopes compose: `execute-e2e` does **not** imply `github-write`. Ticket creation always needs an explicit write phrase.

Standing rules (all scopes):

- No real credentials; synthetic env vars only.
- No autonomous product patches, merges, or settings changes.
- No scheduled cron / routine unless the user adds one later (this skill defines none).
