# Prompt layers

## Assembly

Stable prefix is built in `build_stable_prefix` roughly as:

1. Raw assistant `systemPrompt` (if non-empty)
2. `## Agent Runtime Identity` (name, id, session; sub-agent parent/depth when set)
3. Short `## Session Context` note about optional `<session-context>`
4. `## Persona Template (<filename>)` from SOUL candidates
5. `## Workspace Instructions (<filename>)` from agents.md candidates

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

Instruction file contents participate in a fingerprint used in the stable prompt cache key. Editing `agents.md` / `SOUL.md` (etc.) should invalidate the cached stable prefix on a later LLM call without requiring a full app restart. If a newly written SOUL does not appear yet, continue from the text just written and expect the next resume/turn to pick it up.

## Bundled `prompt.md` vs DB

- `src-tauri/bundled_assistants/*/prompt.md` is compiled/embedded and used when **creating** missing default assistants.
- `ensure_default_assistants` skips assistants that already exist (preserves user edits).
- Shipping a shorter `prompt.md` does **not** rewrite existing DB `systemPrompt` values.
