---
name: libragent-harness-reference
description: >
  On-demand LibrAgent runtime facts: system-prompt layers, tool naming (server__tool),
  session/workspace isolation, skills progressive disclosure, when to call agent-init,
  and verification habits. Use when unsure how LibrAgent injects context, how sub-agents
  inherit (or do not inherit) workspace/instructions/skills, which instruction files load,
  or what belongs in assistant systemPrompt vs workspace agents.md vs skills.
  Triggers: harness, system prompt layers, agents.md vs SOUL, session workspace,
  sub-agent isolation, tool naming, bootstrap workspace guidelines.
---

# LibrAgent Harness Reference

Load only the reference you need. Do not paste this whole skill into every turn.

## Prompt layers (stable prefix order)

1. **Assistant `systemPrompt`** — shallow identity from the assistant config (bundled seed is one line).
2. **`## Agent Runtime Identity`** — name, agent id, session id (and sub-agent parent when applicable). Already injected; do not restate.
3. **`## Session Context`** — note that live `<session-context>` may appear.
4. **`## Persona Template`** — first non-empty of `.github/SOUL.md`, `SOUL.md`, `.github/soul.md`, `soul.md` in the **effective workspace**.
5. **`## Workspace Instructions`** — first non-empty of `agents.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`.
6. Context providers / service tool state (skills catalog, time, etc.).

Missing SOUL / agents.md → that section is omitted. No error.

## Where knowledge should live

| Need | Put it here | Skill / action |
| --- | --- | --- |
| Who this assistant is (1–2 lines) | Assistant `systemPrompt` | Edit assistant; bundled `prompt.md` is seed-only |
| User/project prefs, bans, commands | Workspace `agents.md` (+ modular guides) | **agent-init** |
| Tone / persona | `SOUL.md` | **soul-awakening** if missing |
| How LibrAgent itself works | This skill + `references/` | Read on demand |
| Multi-agent team constitution | teamwork / org artifacts | **teamwork** / **org** |

Do **not** dump long operating doctrine into every assistant `systemPrompt`. Prefer workspace files (when the user wants them) or skills.

## Cold-start routing

- Empty / new workspace and the task needs durable project rules → offer or run **agent-init** (do not invent a fat default `agents.md` silently unless the user wants guidelines).
- Need OS/Python/Node for MCP → **setup-wizard**.
- Need sub-agent handoff rules → **delegate** (and `references/session-isolation.md` here for the matrix).
- Need tool/server install → **tool-installer** / **skill-deployer**.

## Hard runtime rules (always true)

- Builtin tools are named `server__tool` (e.g. `agent__startSession`). Use exact names from the current session tool list.
- Child sessions do **not** inherit parent workspace, `agents.md`/`SOUL.md`, or workspace-local skills unless you use org inheritance / `workspaceOverride` / explicit handoff. See `references/session-isolation.md`.
- `scratchpad__*` is session-private; parents do not read child scratchpads. Deliver results in the child's **final text**.
- Bundled assistant `prompt.md` updates apply only to **new** DB seeds; existing assistants keep their stored `systemPrompt` until edited or reset.

## References

- [prompt-layers.md](references/prompt-layers.md) — assembly details and cache notes
- [session-isolation.md](references/session-isolation.md) — parent/child workspace and instruction inheritance
- [verification-habits.md](references/verification-habits.md) — lightweight evidence habits (not a second system prompt)
