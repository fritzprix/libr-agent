# Request Patterns

| User says | Mode | First actions |
| --- | --- | --- |
| PR 만들어줘 / open a PR | Feature | status → commit? → push → pr_create |
| 브랜치 따서 작업 / feature branch | Feature | create_branch → (code work) → commit → push |
| PR #42 리뷰 | Review | pr_view → pr_checks → gh pr diff |
| PR #42 머지 | Merge | pr_checks → confirm → pr_merge |
| v1.2.0 릴리즈 | Release | log_since_tag → notes → release_create |
| CI 통과했나 | Review/Merge | pr_checks |

## Natural language → branch name

Slugify user feature description: lowercase, hyphens, max ~40 chars.

Example: "Add calendar export" → `feat/calendar-export`
