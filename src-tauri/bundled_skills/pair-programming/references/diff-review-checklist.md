# Diff Review Checklist (Navigator)

Both sessions share `workspaceOverride`. Navigator reviews before approving the next turn.

## Quick review steps

1. Identify changed files (git status, workspace search, or Driver's Change Summary).
2. Read diff for scope creep — changes should match the micro-task only.
3. Check tests/lint if the project defines them.
4. Verify no secrets, debug prints, or commented-out blocks left behind.

## Checklist

- [ ] Change matches the stated micro-task
- [ ] No unrelated file edits
- [ ] Naming matches project conventions
- [ ] Error paths handled for new logic
- [ ] Tests added or updated when behavior changed
- [ ] No credentials in code or logs

## Revise vs approve

**Approve** when checklist passes or issues are trivial (typos).

**Revise** when:

- Missing tests for new behavior
- Architectural mismatch with agreed design
- Incomplete micro-task

Keep revise feedback numbered and file-specific.

## Role rotation

Before rotation, Navigator posts **handoff note** summarizing open items. New Driver reads shared workspace + handoff only — not full prior chat.

See [role-rotation.md](role-rotation.md).
