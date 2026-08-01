# Issue linking keywords

GitHub closes linked issues when the PR merges into the **default** branch of the repo, or when the closing reference is recognized for the PR's base (behavior can vary by repo settings). Prefer explicit keywords in the PR description.

## Closing keywords (auto-close on merge)

Use any of these followed by an issue number:

- `Fixes #123`
- `Closes #123`
- `Resolves #123`
- Same with `Fixed` / `Closed` / `Resolved` (past tense also works)

Full form also works: `Fixes https://github.com/owner/repo/issues/123`

## Non-closing references

When the PR is related but should **not** close the issue:

- `Related to #123`
- `See also #123`
- `Refs #123`

## Multiple issues

```markdown
Fixes #1641
Fixes #1638
```

Or one line: `Fixes #1641, fixes #1638`

## Bind after PR exists

```bash
gh pr view <n> --json body -q .body
# Edit body to include Fixes #N, then:
gh pr edit <n> --body "…"
```

Do not rely on title-only `#123` for auto-close — put the keyword in the **body**.

## Choosing the right issue

| Signal | Use |
| ------ | --- |
| User says “for #1641” / “bind #1641” | That issue |
| Branch `fix/1641-…` or commit `(#1641)` | Confirm with `gh issue view 1641` |
| Several open bugs touched | Prefer one PR per issue when possible; otherwise list every closed issue |
| Partial fix | `Related to #N` — do not `Fixes` |
