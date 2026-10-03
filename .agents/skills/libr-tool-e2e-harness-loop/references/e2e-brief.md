# E2E brief template (libr-delegate `request`)

Paste into Session API `request`. Keep self-contained. No Cursor-chat assumptions.

```text
Goal:
Run a FULL E2E for <TOOL_GROUP> against a live LibrAgent.

Preconditions (already true unless stated):
- App rebuilt from <branch/note>
- <extension Connected / bridge :3847 / …>

Hard rules:
- Use only these tools: <list>
- ALWAYS pass <required explicit args, e.g. browser="userChrome"|"sidecar">
- Harmless pages only: https://example.com , https://example.org
- Do NOT dig the repo / rewrite code
- Do NOT claim Connected/backend without citing tool result text

Procedure:
1) …
2) …
N) close/cleanup sessions

Deliverable — FINAL assistant text ONLY:

## Verdict
PASS | FAIL | PARTIAL

## Runtime
- <backend/path>: PASS/FAIL + evidence quote

## Tool matrix
| tool | result | evidence |

## Failures
- none | bullets

## Contamination check
YES/NO + evidence (wrong browser=/session identity)
```

## Brief quality bar

- Every matrix row must be observable from a tool result, not inferred.
- Include a **negative or distinction** check when two backends exist (no silent fallback).
- Cap scope: prefer one primary path + one short control path over exhaustive product tour.
- Forbid rabbit holes: “no codebase archaeology”, “stop after matrix + deliverable”.
