# Pull Request Template

Use for `--body` in `pr_create` (escape newlines in shell or use a temp file + `gh pr create --body-file`).

```markdown
## Summary

- What changed and why (1–3 bullets)

## Test plan

- [ ] Unit / integration tests run
- [ ] Manual scenario: ...

## Risk

- Breaking changes: none / describe
- Rollback: revert PR #N
```

Keep **title** short (`feat:`, `fix:`, `chore:`). Body can match the user's language.
