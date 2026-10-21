---
name: skill-deployer
description: "Deploy, install, update, or remove a validated custom skill in user_skills/ (global), .libragent/skills/ (workspace), or assistant scope from a local folder or a GitHub URL. Use when installing a skill from a GitHub URL, deploying a validated local skill after skill-creator, updating an installed skill from its recorded source, or removing an installed skill. NOT for system_skills or app-bundled skills. GitHub URL rule: a repo or subpath that contains SKILL.md is a skill and uses this skill; an MCP server package or MCP config uses tool-installer instead; if unsure, inspect the repo before installing. Triggers: deploy skill, install skill, install this skill, GitHub skill URL, update skill, remove skill, uninstall skill, global skill, workspace skill, user_skills, 스킬 깔아줘, 스킬 설치해줘, 이거 깔아줘, 스킬 업데이트, 스킬 삭제."
---

# Skill Deployer

Install, update, or remove a **validated custom skill** in `user_skills/` (global), `.libragent/skills/` (workspace), or `assistants/<id>/skills/` (assistant) from a **local directory** or a **GitHub URL** so agents load it on the next turn.

> **Prerequisite**: Every deployment runs `skill-creator` strict validation (`validate_skill.py --strict`) automatically before and after copying, verifying YAML frontmatter and that the folder name matches `name:`.

This skill covers **fetching, reviewing, installing, updating, and removing** user/workspace/assistant skills. It does **not** cover authoring new skills from scratch — use **skill-creator**. It does **not** ship skills inside the LibrAgent binary — see Bundled scope below.

## What This Skill Is NOT

| User intent | Correct skill | Target path |
| --- | --- | --- |
| Install a skill from a GitHub URL or local folder globally | **skill-deployer** (this skill) | `{dataDir}/user_skills/{name}/` |
| Install for this session/project | **skill-deployer** (this skill) | `{workspace}/.libragent/skills/{name}/` |
| Update or remove an installed skill | **skill-deployer** (this skill) | Same scope path as installed |
| Ship skill inside the LibrAgent app | **Developer workflow** (not this skill) | `src-tauri/bundled_skills/{name}/` → synced to `system_skills/` on build |

**Never deploy custom skills to `{dataDir}/system_skills/`.** That folder is a **managed mirror** of packaged bundled skills. Manual additions are **deleted on app startup**. Do not create or edit `.bundled_manifest.json` outside bundled sync — it does not register custom skills.

If the user says "system skill" or points at `system_skills`, they usually mean **global availability**. Deploy to **`user_skills/`**, not `system_skills/`.

## Scope Decision

| Scope | Choose when | Override priority |
| --- | --- | --- |
| **workspace** | Skill is specific to the current project or session | Highest |
| **assistant** | Skill is for one assistant only | Middle |
| **global** | Skill should be available in every session | Lowest |

Default to **workspace** when unsure — safest and easiest to remove.

## Target Paths

### Workspace scope (most common)

```text
{workspace-root}/.libragent/skills/{skill-name}/SKILL.md
```

Read `Workspace Root` from the system prompt (`## Workspace` section).

Example: `C:/Users/alice/AppData/Roaming/com.fritzprix.libragent/workspaces/<session-id>/.libragent/skills/my-skill/SKILL.md`

> **NOT** `{workspace-root}/{skill-name}/` — skills at the session root are never scanned.

### Global scope (user skills)

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

### Assistant scope

```text
{dataDir}/assistants/{assistant-id}/skills/{skill-name}/SKILL.md
```

The user must provide the assistant ID from the Assistants settings panel.

### Agent import scope (IDE / local dev)

Auto-discovered under the workspace root:

- `.agents/skills/`, `.cursor/skills/`, `.gemini/skills/`, `.copilot/skills/`
- `.windsurf/skills/`, `.claude/skills/`, `.cline/skills/`, `.continue/skills/`

```text
{workspace-root}/.cursor/skills/{skill-name}/SKILL.md
```

### Bundled scope — do NOT use this skill

Repo path only; managed by app build mirror and startup sync (not user-deployable):

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

## Installing from a GitHub URL

Always use **`deploy_skill.py`** rather than improvising `git clone` or manual shell copies. Supported URL formats:

- Repository root: `https://github.com/<owner>/<repo>`
- Branch/tag/commit + subpath: `https://github.com/<owner>/<repo>/tree/<ref>/<subpath>`
- Direct `SKILL.md` link: `https://github.com/<owner>/<repo>/blob/<ref>/<subpath>/SKILL.md`
- Commit URL: `https://github.com/<owner>/<repo>/commit/<sha>`
- Query parameters: `https://github.com/<owner>/<repo>?ref=<ref>&path=<subpath>`
- Explicit flags: `--ref <branch-tag-or-sha>` and `--subpath <relative-path>` (use with the repository root URL; passing a conflicting `--ref` alongside `/tree/<other-ref>` is rejected)

Repository-level auxiliary files (`README.md`, `CHANGELOG.md`, `INSTALLATION.md`, `.github/`, `.gitignore`, etc.) are automatically omitted during staging and deployment so single-skill repositories pass strict layout validation cleanly.

### Step 1 — Inspect and Select (`--inspect`)

Fetch at a pinned 40-character commit SHA, discover `SKILL.md` folders, run strict validation (`validate_skill.py --strict`), and list scripts/executables without writing to disk:

```bash
python "<skill-deployer-base-dir>/scripts/deploy_skill.py" "<github-url>" --inspect
```

- **Multiple `SKILL.md` folders**: `deploy_skill.py` lists each discovered skill (`name`, `subpath`, `description`, validation status, and scripts). Present the list to the user, let them pick one, and pass `--skill <name>` in the next step.
- **Single `SKILL.md` folder**: `deploy_skill.py` prints the `=== Skill Review ===` summary (name, description, pinned commit SHA, and any scripts or executables).
- **No `SKILL.md` (`[DEPLOY-GITHUB-003]`)**: the repo is not a skill. If it is an MCP server (npm/PyPI package, `@modelcontextprotocol/sdk` or `mcp` dependency, README with an MCP config block), switch to **tool-installer**. Otherwise tell the user what the repo is and ask what they want.

### Step 2 — Review and Obtain User Approval (Mandatory for Scripts)

Show the user what will be installed:

1. Skill `name` and `description`
2. Source URL and pinned commit SHA (warn the user when installing from an unknown repository, as `SKILL.md` is injected into prompt instructions)
3. Target scope (`workspace`, `global`, or `assistant`)
4. Any **scripts or executables** detected inside the skill

> **Safety Gate**: If the skill contains any scripts or executables (`scripts/*`, `.py`, `.sh`, `.ps1`, `.js`, shebang files, or executable binaries), `deploy_skill.py` refuses to install (exit code `4`, `[DEPLOY-APPROVAL-001]`) unless `--approve-scripts` is passed. **Never pass `--approve-scripts` until the user explicitly approves the script list.**

### Step 3 — Deploy with Provenance

Once selected (and approved by the user if scripts are present), deploy to the target scope:

```bash
# Prompt-only skill (no scripts)
python "<skill-deployer-base-dir>/scripts/deploy_skill.py" "<github-url>" --scope workspace --workspace "<workspace-root>" --skill "<skill-name>"

# Skill containing scripts (ONLY after user approval)
python "<skill-deployer-base-dir>/scripts/deploy_skill.py" "<github-url>" --scope workspace --workspace "<workspace-root>" --skill "<skill-name>" --approve-scripts

# Global scope
python "<skill-deployer-base-dir>/scripts/deploy_skill.py" "<github-url>" --scope global --skill "<skill-name>"
```

`deploy_skill.py` writes `.skill-provenance.json` inside the installed skill folder (`sourceUrl`, `ref`, pinned 40-char `commit`, `subpath`, `scripts`, `installedAt`, `updatedAt`) and runs post-deploy strict validation with automatic rollback on failure.

## Deploying a Local Skill Folder

```bash
python "<skill-deployer-base-dir>/scripts/deploy_skill.py" "<skill-source-dir>" --scope global
python "<skill-deployer-base-dir>/scripts/deploy_skill.py" "<skill-source-dir>" --scope workspace --workspace "<workspace-root>"
python "<skill-deployer-base-dir>/scripts/deploy_skill.py" "<skill-source-dir>" --scope assistant --assistant-id "<id>"
```

| Flag | Purpose |
| --- | --- |
| `--inspect` | Fetch/validate and show discovered skills, description, and scripts without installing |
| `--skill <name>` | Select a specific skill when a GitHub repo contains multiple `SKILL.md` folders |
| `--ref <ref>` | Override Git branch, tag, or commit SHA |
| `--subpath <path>` | Override relative subdirectory inside the GitHub repo |
| `--approve-scripts` | Confirm user approval to install a skill containing scripts/executables |
| `--overwrite` | Replace existing target directory |
| `--dry-run` | Strict validate source + print target without copying |
| `--data-dir <dir>` | Override default `com.fritzprix.libragent` data directory |

Inspect validation failures manually when needed:

```bash
python "<skill-creator-base-dir>/scripts/validate_skill.py" "<skill-folder>" --strict --detailed
```

## Updating and Removing Installed Skills

### Update from Recorded Provenance (`--update`)

Re-fetch the skill from the source recorded in `.skill-provenance.json`, validate strictly, and replace the installed copy (rolls back to the previous version if validation fails):

```bash
python "<skill-deployer-base-dir>/scripts/deploy_skill.py" --update "<skill-name>" --scope workspace --workspace "<workspace-root>"
python "<skill-deployer-base-dir>/scripts/deploy_skill.py" --update "<skill-name>" --scope global
```

- If the skill was originally installed using a raw 40-character commit SHA as `ref`, `--update` re-fetches that same commit unless you pass `--ref <branch>` to switch to tracking a branch.
- If the updated skill contains scripts or executables, get user approval and pass `--approve-scripts`.

### Clean Removal (`--remove`)

Delete the installed skill folder cleanly from the target scope (never touches `system_skills/`):

```bash
python "<skill-deployer-base-dir>/scripts/deploy_skill.py" --remove "<skill-name>" --scope workspace --workspace "<workspace-root>"
python "<skill-deployer-base-dir>/scripts/deploy_skill.py" --remove "<skill-name>" --scope global
```

## Post-Install Verification

On the **next agent turn**, check `<available_skills>` in the system prompt:

```xml
<skill source="global">
  <name>my-new-skill</name>
  <description>...</description>
  <location>.../user_skills/my-new-skill/SKILL.md</location>
</skill>
```

Confirm and report to the user:

1. Skill name appears in `<available_skills>` (or is gone after `--remove`)
2. `source` matches intended scope (`workspace`, `assistant`, or `global`)
3. `<location>` matches the deployed path (`user_skills` for global, not `system_skills`)
4. Source URL and pinned commit SHA recorded in `.skill-provenance.json`

## Coalesce Behavior

Skills merge with **first-wins by lowercase name**. Priority: workspace > assistant > agent import > global > system.

A workspace skill silently overrides a global skill with the same name.

## Common Mistakes

- **Improvising with `git clone` or `workspace__runShell`** — always use `deploy_skill.py` so commits are pinned, strict validation runs, script approval is enforced, and `.skill-provenance.json` is recorded
- **Passing `--approve-scripts` without asking the user** — always show the user the script list and wait for confirmation first
- **Deploying to `system_skills/`** — use `user_skills/` for global custom skills; ship app-bundled skills via `src-tauri/bundled_skills/` in the repo
- **Confusing `system_skills` with global** — global user skills live in `user_skills/`
- **Creating `.bundled_manifest.json`** — only bundled sync uses this; custom skills use `.skill-provenance.json`
- **Hash-checking against bundled manifest** — irrelevant for custom skills; discovery uses directory scan + valid `SKILL.md`
- **Skipping validation** — invalid YAML is silently ignored by the Rust scanner
- **Workspace root deploy** — must be `.libragent/skills/{name}/`, not `{workspace}/{name}/`
- **Wrong global path** — use `com.fritzprix.libragent\user_skills\`, not legacy `LibrAgent\skills\`
- **Apostrophe in YAML** — fix in skill-creator (`"it's"` not `'it\'s'`)
- **Name ≠ folder name** — breaks `@skill:name` lookup and fails `--strict` validation
- **Expecting immediate UI refresh** — skills reload at the start of the next agent turn
