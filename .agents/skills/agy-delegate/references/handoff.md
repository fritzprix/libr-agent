# Handoff & isolation

Load when writing the agy `-p` prompt or debugging missing context.

## Quick matrix

| Need | Works by default? | What to do |
| --- | --- | --- |
| Child sees Cursor chat/history | No | Restate everything in `-p` |
| Child uses Cursor CWD files | Only if you `cd` there or `--add-dir` | Pass absolute `--add-dir` (repeatable) |
| Child inherits repo `agents.md` / skills | Only if those files are in added dirs / cwd | `--add-dir` the repo, or inline critical rules |
| Lineage / follow-up | Partial | `--conversation <id>` or `--continue` |
| Unattended tool use | No | `--dangerously-skip-permissions` |

## Task template

```text
Goal:
- [exact objective]

Scope:
- Only [paths / modules]
- Do not [forbidden actions]

Workspace:
- Work under [absolute path] (must match --add-dir / shell cwd)

Deliverable (stdout / JSON response only):
- [format: markdown summary / file paths / commands run]
- Include concrete paths and pass/fail evidence
```

## Acceptance (Cursor owns it)

After the process exits:

1. Confirm exit code `0`
2. If JSON: confirm `status` is `SUCCESS`; take `response` as the result
3. Spot-check cited paths/commands in this Cursor workspace if relevant
4. Re-steer with another `-p` (optionally `--conversation` / `--continue`) if incomplete

Do not treat “done” prose without evidence as success.

## Common failures

- Stale `--model` id → list with `agy models` and retry with closest **flash-high** (do not silently upgrade to Pro)
- Agent swapped `--model` without user ask → wrong; keep skill default
- Relative `--add-dir` when cwd differs → prefer absolute paths
- Assumed shared chat context → child cannot see Cursor thread
- Interactive `-i` for automation → hang / no captured result; use `-p`
- Forgot permissions flag on tool-heavy tasks → blocked waiting for approval
- Using LibrAgent HTTP endpoints here → wrong skill (`libr-delegate`)
