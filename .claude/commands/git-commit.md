---
argument-hint: '[commit message]'
description: Create a single git commit from current changes with a clear, conventional message
allowed-tools: Bash(git add:*), Bash(git status:*), Bash(git commit:*), Bash(git diff:*), Bash(git branch --show-current:*), Bash(git log:*), Bash(git rev-parse:*)
---

# Create a git commit

## Context

- Current branch: !`git branch --show-current`
- Recent commits: !`git log --oneline -10`
- Status: !`git status`
- Diff (staged and unstaged): !`git diff HEAD`

## Your task

Based on the changes above, create one cohesive commit.

- If an explicit message is provided as arguments, prefer it and polish (preserve intent)
- Otherwise, synthesize a concise, conventional commit message (type(scope): subject)
- Explain what was changed in 1–3 bullets in the body when helpful
- Keep line length to ~72 for subject

Then stage all relevant files and create the commit. Use:

- Stage: `git add -A`
- Commit: `git commit -m "<message>"`

## Commit message guidance

- Use present tense ("add", "fix", "refactor")
- Reference scripts or docs when relevant (e.g., pnpm refactor:validate)
- Avoid noisy or redundant wording
