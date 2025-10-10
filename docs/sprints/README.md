# Sprint Archives

This folder stores completed sprint logs and release-focused notes.

How we use this directory:

- Add a markdown file for each sprint with a filename format: `YYYY-MM-DD-sprint.md`.
- Include a short YAML frontmatter with `title`, `date`, and `status`.
- Keep the content concise: goals, completed items, blockers, and follow-ups.

Current archive:

- [2025-01-10 Sprint](./2025-01-10-sprint.md)

## Sprint template

```md
---
title: 'Sprint Log — YYYY-MM-DD'
date: YYYY-MM-DD
status: completed
---

# Sprint Log

## YYYY-MM-DD

- [x] **Title of completed task** — short description

## Blockers

- None

## Follow-ups

- Tasks to carry into next sprint
```

## Automation ideas

- CI job to collect sprint files and generate release notes.
- GitHub Actions to tag releases from sprint dates.
