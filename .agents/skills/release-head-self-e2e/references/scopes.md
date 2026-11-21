# Authorization scopes

Default when the user does not name a scope: **`read-only`**.

| Scope          | User must say (examples)                         | GitHub                                                                       | App testing                       | Local artifacts                |
| -------------- | ------------------------------------------------ | ---------------------------------------------------------------------------- | --------------------------------- | ------------------------------ |
| `read-only`    | “plan만”, “dry-run”, “read only”                 | Read/search only                                                             | No                                | `pins.json`, hints, draft plan |
| `plan`         | “케이스 목록 만들어”                             | Read/search only                                                             | No                                | `plan.md`, `dedupe.md` draft   |
| `github-write` | “tracking 이슈 올려”, “create/update the ticket” | Create/update **one** tracking issue + results comment + bug issues (step 5) | No                                | Same + issue URL               |
| `execute-e2e`  | “external E2E 실행”, “unattended run”            | Read (optional comment if also `github-write`)                               | Yes, serialized, isolated profile | Full per-row evidence          |

Scopes compose: `execute-e2e` does **not** imply `github-write`. Tracking-issue creation,
the results comment, and bug filing all need an explicit write phrase.

Standing rules (all scopes):

- No real credentials; dummy/synthetic env vars only.
- No autonomous product patches, merges, or settings changes.
- No scheduled cron / routine unless the user adds one later (this skill defines none).
- Isolated profile only: never the primary data dir.
