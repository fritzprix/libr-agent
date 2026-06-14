# Branch Strategy

Defaults for **user repositories**. Detect repo conventions before inventing new ones.

## Detect existing conventions

Check for:

- `main` vs `master` vs `develop` (use `repo_context` default_branch)
- Protected branch rules (ask user if unsure)
- CONTRIBUTING.md / README branching notes

## Recommended defaults

| Change type | Branch prefix | Example |
| --- | --- | --- |
| Feature | `feat/` | `feat/calendar-sync` |
| Bug fix | `fix/` | `fix/login-timeout` |
| Chore/docs | `chore/` | `chore/deps-bump` |
| Release prep | `release/` | `release/1.2.0` |

Branch from `default_branch` unless the user specifies otherwise.

## When to ask

- Multiple long-lived branches (`develop` + `main`)
- User says "hotfix to production"
- Repo uses GitFlow or custom naming

## Do not

- Force-push `main` / `master` without explicit user approval
- Create branches with spaces or unicode slugs
