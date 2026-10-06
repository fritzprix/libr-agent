---
name: libragent-harness-reference
description: >
  On-demand LibrAgent runtime facts: system-prompt layers, tool naming (server__tool),
  session/workspace isolation, skills progressive disclosure, when to call agent-init,
  and verification habits. Use when unsure how LibrAgent injects context, how sub-agents
  inherit (or do not inherit) workspace/instructions/skills, which instruction files load,
  or what belongs in assistant systemPrompt vs workspace agents.md vs skills.
  Triggers: harness, system prompt layers, agents.md vs SOUL, session workspace,
  sub-agent isolation, tool naming, bootstrap workspace guidelines,
  external inject, wake session, HTTP /api/sessions messages.
---

# LibrAgent Harness Reference

Load only the reference you need. Do not paste this whole skill into every turn.

## Prompt layers (stable prefix order)

1. **Assistant `systemPrompt`** — shallow identity from the assistant config (bundled seed is one line).
2. **`## Agent Runtime Identity`** — name, agent id, session id, external wake POST URL (and sub-agent parent when applicable). Already injected; do not restate.
3. **`## Session Context`** — note that live `<session-context>` may appear.
4. **`## Core Execution Principles`** — hardcoded acceptance/deliverable rules.
5. **`## Persona Template`** — first non-empty of `.github/SOUL.md`, `SOUL.md`, `.github/soul.md`, `soul.md` in the **effective workspace**.
6. **`## Workspace Instructions`** — first non-empty of `agents.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`.
7. **`## Active Operational Lessons`** — `@harness/LESSONS.active.md` when present (app-local harness-lessons; git-safe). See `references/prompt-layers.md`.
8. Context providers / service tool state (skills catalog, time, etc.).

Missing SOUL / agents.md / LESSONS → that section is omitted. No error.

## Where knowledge should live

| Need | Put it here | Skill / action |
| --- | --- | --- |
| Who this assistant is (1–2 lines) | Assistant `systemPrompt` | Edit assistant; bundled `prompt.md` is seed-only |
| User/project prefs, bans, commands | Workspace `agents.md` (+ modular guides) | **agent-init** (may dirty git) |
| After-action behavioral lessons (git-safe) | `@harness/LESSONS.active.md` | **postmortem-improve** |
| Tone / persona | `SOUL.md` | **soul-awakening** if missing |
| How LibrAgent itself works | This skill + `references/` | Read on demand |
| Multi-agent team constitution | teamwork / org artifacts | **teamwork** / **org** |

Do **not** dump long operating doctrine into every assistant `systemPrompt`. Prefer `@harness` lessons for incident learning, workspace files when the user wants project standing orders, or skills.

## Cold-start routing

- Empty / new workspace and the task needs durable project rules → offer or run **agent-init** (do not invent a fat default `agents.md` silently unless the user wants guidelines).
- Need OS/Python/Node for MCP → **setup-wizard**.
- Need sub-agent handoff rules → **delegate** (and `references/session-isolation.md` here for the matrix).
- Need generator–evaluator / proof-before-done on a child → **delegation-eval-loop** (after `delegate` mechanics).
- Need tool/server install → **tool-installer** / **skill-deployer**.

## Hard runtime rules (always true)

- Builtin tools are named `server__tool` (e.g. `agent__spawnSession`). Use exact names from the current session tool list.
- Child sessions do **not** inherit parent workspace, `agents.md`/`SOUL.md`, or workspace-local skills unless you use org inheritance / `workspaceOverride` / explicit handoff. See `references/session-isolation.md`.
- `scratchpad__*` is session-private; parents do not read child scratchpads. Deliver results in the child's **final text**.
- Bundled assistant `prompt.md` updates apply only to **new** DB seeds; existing assistants keep their stored `systemPrompt` until edited or reset.

## External wake (hooks / webhooks)

Outside processes resume a session only by HTTP inject (not by printing to their own stdout).

**Already in every turn:** `## Agent Runtime Identity` includes
`External wake (this session): POST http://127.0.0.1:<port>/api/sessions/<id>/messages`
with this session’s real port + id. Agents must wire hooks/`--exec`/webhooks to that URL
(body `{"content":"…"}`). Echo-only or file-append stubs do **not** wake the session.

Also available: `POST …/channel` (`serverName` + `content`, optional `meta`).

For event-driven resume vs clock polling, use **`call-me-back`**. Do not list per-app hook CLIs here.

## References

- [prompt-layers.md](references/prompt-layers.md) — assembly details and cache notes
- [session-isolation.md](references/session-isolation.md) — parent/child workspace and instruction inheritance
- [verification-habits.md](references/verification-habits.md) — lightweight evidence habits (not a second system prompt)
