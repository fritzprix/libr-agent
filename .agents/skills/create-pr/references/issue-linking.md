# Issue linking keywords

GitHub itself closes linked issues only when the PR merges into the **default** branch (`main`). In this repo most PRs merge into `dev/*`, so `.github/workflows/close-linked-issues.yml` closes them instead: on merge into **any** branch it closes every same-repo issue referenced by a closing keyword in the PR title/body, comments `Closed by #<PR>`, and sets reason `completed`.

- Use `Fixes #N` for dev-branch PRs too; it closes #N on merge.
- `closingIssuesReferences` / the issue sidebar stay empty for non-default bases; that is expected.
- Ignored: references inside backticks or code fences, `Refs #N` / plain `#N`, and other repos (`owner/repo#N`).

## Closing keywords (auto-close on merge)

Use any of these followed by an issue number:

- `Fixes #123`
- `Closes #123`
- `Resolves #123`
- Same with `Fixed` / `Closed` / `Resolved` (past tense also works)

Full form also works for this repo: `Fixes https://github.com/fritzprix/libr-agent/issues/123`

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

| Signal                                  | Use                                                                      |
| --------------------------------------- | ------------------------------------------------------------------------ |
| User says “for #1641” / “bind #1641”    | That issue                                                               |
| Branch `fix/1641-…` or commit `(#1641)` | Confirm with `gh issue view 1641`                                        |
| Several open bugs touched               | Prefer one PR per issue when possible; otherwise list every closed issue |
| Partial fix                             | `Related to #N` — do not `Fixes`                                         |
