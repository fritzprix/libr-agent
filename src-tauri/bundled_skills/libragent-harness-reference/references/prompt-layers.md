# Prompt layers

## Assembly

Stable prefix is built in `build_stable_prefix` (plus optional harness lessons) roughly as:

1. Raw assistant `systemPrompt` (if non-empty)
2. `## Agent Runtime Identity` (name, id, session, **external wake POST URL** for this session; sub-agent parent/depth when set)
3. Short `## Session Context` note about optional `<session-context>`
4. `## Core Execution Principles` (hardcoded)
5. `## Persona Template (<filename>)` from SOUL candidates
6. `## Workspace Instructions (<filename>)` from agents.md candidates
7. `## Active Operational Lessons` — from `@harness/LESSONS.active.md` when non-empty (app-local `{appData}/harness-lessons/<scopeId>/`; **not** project git). Scope: `org-<orgRootSessionId>` or `ws-<workspaceHash>`.

Volatile / medium sections (context providers, non-stable service context) are appended separately for the provider request layout.

## Instruction file candidates

**Workspace behavior** (first hit wins):

- `agents.md`
- `AGENTS.md`
- `CLAUDE.md`
- `GEMINI.md`

**Persona** (first hit wins):

- `.github/SOUL.md`
- `SOUL.md`
- `.github/soul.md`
- `soul.md`

Resolved from the session's **effective workspace directory**, not from the app install tree.

## Cache / hot reload

Instruction file contents (workspace `agents.md` / `SOUL.md` **and** `@harness/LESSONS.active.md`) participate in a fingerprint used in the stable prompt cache key. Editing those files should invalidate the cached stable prefix on the next LLM call without requiring a full app restart. If a newly written SOUL/LESSONS does not appear yet, continue from the text just written and expect the next resume/turn to pick it up.

## Harness lessons vs teamwork vs workspace agents.md

| Path | Git | Prompt | When |
| --- | --- | --- | --- |
| `@harness/LESSONS.active.md` | App-data (clean) | Auto-inlined (capped) | Solo **or** org learning via **postmortem-improve** |
| `@teamwork/...` | App-data (clean) | Pointers in org SC only; content not auto-inlined | Multi-agent scaffold |
| Workspace `agents.md` / `SOUL.md` | May dirty project git | Auto-inlined | Project standing orders / persona |

## Bundled `prompt.md` vs DB

- `src-tauri/bundled_assistants/*/prompt.md` is compiled/embedded and used when **creating** missing default assistants.
- `ensure_default_assistants` skips assistants that already exist (preserves user edits).
- Shipping a shorter `prompt.md` does **not** rewrite existing DB `systemPrompt` values.
