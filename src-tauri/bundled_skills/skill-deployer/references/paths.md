# Skill Target Paths

Where each scope lives on disk, how to resolve `{dataDir}`, and locations that must never receive custom skills.

## Workspace scope (most common)

```text
{workspace-root}/.libragent/skills/{skill-name}/SKILL.md
```

Read `Workspace Root` from the system prompt (`## Workspace` section).

Example: `C:/Users/alice/AppData/Roaming/com.fritzprix.libragent/workspaces/<session-id>/.libragent/skills/my-skill/SKILL.md`

> **NOT** `{workspace-root}/{skill-name}/` — skills at the session root are never scanned.

## Global scope (user skills)

```text
{dataDir}/user_skills/{skill-name}/SKILL.md
```

Resolve `{dataDir}` from `<available_skills>`:

1. Find a skill with `source="global"` whose `<location>` contains **`user_skills`** (not `system_skills`).
2. Take the parent of `user_skills/` as `{dataDir}`.
3. If none exist, use OS defaults:

| OS | `{dataDir}` |
| --- | --- |
| Windows | `%APPDATA%\com.fritzprix.libragent\` |
| macOS | `~/Library/Application Support/com.fritzprix.libragent/` |
| Linux | `~/.local/share/com.fritzprix.libragent/` |

Example: `C:\Users\alice\AppData\Roaming\com.fritzprix.libragent\user_skills\my-skill\SKILL.md`

> **NOT** `{dataDir}/system_skills/` (bundled mirror), `{dataDir}/skills/` (legacy), or `%APPDATA%\LibrAgent\skills\` (old app ID path).

## Assistant scope

```text
{dataDir}/assistants/{assistant-id}/skills/{skill-name}/SKILL.md
```

The user must provide the assistant ID from the Assistants settings panel.

## Agent import scope (IDE / local dev)

Auto-discovered under the workspace root:

- `.agents/skills/`, `.cursor/skills/`, `.gemini/skills/`, `.copilot/skills/`
- `.windsurf/skills/`, `.claude/skills/`, `.cline/skills/`, `.continue/skills/`

```text
{workspace-root}/.cursor/skills/{skill-name}/SKILL.md
```

## Bundled scope — not deployable with skill-deployer

Repo path only; managed by the app build mirror and startup sync:

```text
src-tauri/bundled_skills/{skill-name}/
```

Runtime mirror (read-only for agents; never write custom skills here):

```text
{dataDir}/system_skills/{skill-name}/
```

## Forbidden Locations

| Path | Why |
| --- | --- |
| `{dataDir}/system_skills/` | Managed bundled mirror — extras deleted on startup |
| `{dataDir}/system_skills/.bundled_manifest.json` | Bundled sync only — does not register custom skills |
| `{workspace-root}/{skill-name}/` | Scanner only reads `.libragent/skills/` |
| `{workspace-root}/.bundled_manifest.json` | Manifest ignored here — does not register skills |
| Legacy `{dataDir}/skills/` | Use `user_skills/` instead |
| Legacy `%APPDATA%\LibrAgent\skills\` | Wrong app data dir — use `com.fritzprix.libragent\user_skills\` |
