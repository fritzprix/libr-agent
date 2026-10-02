# Handoff & isolation

Load when writing the LibrAgent task text or debugging missing context.

## Quick matrix

| Need | Works by default? | What to do |
| --- | --- | --- |
| Child sees Cursor chat/history | No | Restate everything in `request` / message |
| Child uses Cursor CWD files | No | Pass absolute `workspacePath` to that repo |
| Child inherits repo `agents.md` / `CLAUDE.md` | Only if those files live in the **session** workspace | Set `workspacePath` to that folder, or copy critical rules into the task |
| Child uses LibrAgent assistant-scoped skills | Yes | Pick the right `assistantId` |
| Child uses Cursor-only / workspace-local skills outside that path | No | Inline the procedure or use an assistant that already has it |
| Lineage under an existing LibrAgent session | Yes | Set `parentSessionId` to that exact id |

## Task template

```text
Goal:
- [exact objective]

Scope:
- Only [paths / modules]
- Do not [forbidden actions]

Workspace:
- Work in [absolute path] (must match workspacePath if set)

Deliverable (final assistant text only):
- [format: markdown summary / file paths / commands run]
- Include concrete paths and pass/fail evidence
```

## Acceptance (Cursor owns it)

After Idle:

1. Confirm `GET …/sessions/:id` is Idle (or handle Error/Paused)
2. Read latest assistant message text from `GET …/messages`
3. Spot-check cited paths/commands yourself in this Cursor workspace if relevant
4. Re-steer with another `POST …/messages` if incomplete

Do not treat “done” prose without evidence as success.

## Common failures

- Wrong `assistantId` → 404 / create fails — list assistants first
- Relative `workspacePath` → 400 — must be absolute
- Assumed shared workspace without `workspacePath` → child cannot find files
- Spamming status GET → slowdowns / rate limits — backoff
- Forgot LibrAgent not running → connection refused — start the app first
