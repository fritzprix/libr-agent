---
name: skill-deployer
description: "Deploy, install, update, or remove a validated custom skill in user_skills/ (global), .libragent/skills/ (workspace), or assistant scope from a local folder or a GitHub URL. Use when installing a skill from a GitHub URL, deploying a validated local skill after skill-creator, updating an installed skill from its recorded source, or removing an installed skill. NOT for system_skills or app-bundled skills. GitHub URL rule: a repo or subpath that contains SKILL.md is a skill and uses this skill; an MCP server package or MCP config uses mcp-installer instead; if unsure, inspect the repo before installing. Triggers: deploy skill, install skill, install this skill, GitHub skill URL, update skill, remove skill, uninstall skill, global skill, workspace skill, user_skills, 스킬 깔아줘, 스킬 설치해줘, 이거 깔아줘, 스킬 업데이트, 스킬 삭제."
---

# Skill Deployer

Install, update, or remove a **validated custom skill** in `user_skills/` (global), `.libragent/skills/` (workspace), or `assistants/<id>/skills/` (assistant) from a **local directory** or a **GitHub URL** so agents load it on the next turn.

Every deployment runs `skill-creator` strict validation (`validate_skill.py --strict`) before and after copying. Authoring new skills is **skill-creator**'s job. Shipping skills inside the app is a developer workflow (`src-tauri/bundled_skills/`), not this skill.

**Never deploy custom skills to `{dataDir}/system_skills/`.** It is a managed mirror of bundled skills; extras are deleted on app startup. A user asking for a "system skill" usually means **global** — deploy to `user_skills/`.

## Scope Decision

| Scope | Choose when | Target | Override priority |
| --- | --- | --- | --- |
| **workspace** | Specific to the current project or session | `{workspace-root}/.libragent/skills/{name}/` | Highest |
| **assistant** | For one assistant only (user supplies the ID) | `{dataDir}/assistants/{id}/skills/{name}/` | Middle |
| **global** | Available in every session | `{dataDir}/user_skills/{name}/` | Lowest |

Default to **workspace** when unsure — safest and easiest to remove. Read `Workspace Root` from the system prompt. To resolve `{dataDir}`, OS defaults, IDE import folders, and forbidden locations, see [references/paths.md](references/paths.md).

## GitHub URL Routing

- The repo or subpath contains `SKILL.md` → it is a skill; continue with this skill.
- It is an MCP server package or MCP config (npm/PyPI package, `@modelcontextprotocol/sdk` or `mcp` dependency, README with an MCP config block) → use **mcp-installer**.
- Unsure → run `--inspect` first; never guess.

Always use **`deploy_skill.py`** — never improvise with `git clone` or manual shell copies. URL formats and every flag: [references/github-install.md](references/github-install.md).

## Install Flow (GitHub URL)

### Step 1 — Inspect (`--inspect`)

Fetches at a pinned 40-character commit SHA, discovers `SKILL.md` folders, runs strict validation, and lists scripts without writing to disk:

```bash
python "<skill-deployer-base-dir>/scripts/deploy_skill.py" "<github-url>" --inspect
```

- **Multiple skills**: present the listed skills (`name`, `subpath`, `description`, validation, scripts); the user picks one; pass `--skill <name>` next.
- **Single skill**: read the `=== Skill Review ===` summary.
- **No `SKILL.md` (`[DEPLOY-GITHUB-003]`)**: not a skill. Switch to **mcp-installer** if it is an MCP server; otherwise tell the user what the repo is and ask.

### Step 2 — Review and Approve

Show the user: skill `name` and `description`, source URL and pinned commit SHA (warn for unknown repositories — `SKILL.md` becomes prompt instructions), target scope, and any **scripts or executables**.

> **Safety Gate**: if the skill contains scripts or executables, `deploy_skill.py` refuses to install (exit code `4`, `[DEPLOY-APPROVAL-001]`) unless `--approve-scripts` is passed. **Never pass `--approve-scripts` until the user explicitly approves the script list.**

### Step 3 — Deploy

```bash
# Prompt-only skill
python "<skill-deployer-base-dir>/scripts/deploy_skill.py" "<github-url>" --scope workspace --workspace "<workspace-root>" --skill "<skill-name>"

# Skill with scripts (ONLY after user approval)
python "<skill-deployer-base-dir>/scripts/deploy_skill.py" "<github-url>" --scope workspace --workspace "<workspace-root>" --skill "<skill-name>" --approve-scripts

```

For **global** scope, install into the workspace first, then publish with skill-creator's `skill.py deploy` (user skills are written only that way):

```bash
python3 "<skill-creator-base-dir>/scripts/skill.py" deploy "<workspace-root>/.libragent/skills/<skill-name>"
```

The script records `.skill-provenance.json` (source URL, ref, pinned commit, scripts, timestamps) and rolls back automatically if post-deploy validation fails.

## Deploying a Local Skill Folder

```bash
python "<skill-deployer-base-dir>/scripts/deploy_skill.py" "<skill-source-dir>" --scope workspace --workspace "<workspace-root>"
python "<skill-deployer-base-dir>/scripts/deploy_skill.py" "<skill-source-dir>" --scope assistant --assistant-id "<id>"
```

For **global** scope use `python3 "<skill-creator-base-dir>/scripts/skill.py" deploy "<skill-source-dir>"`; `deploy_skill.py --scope global` refuses new installs.

Add `--dry-run` to validate and print the target without copying, or `--overwrite` to replace an existing install.

## Update and Remove

```bash
# Re-fetch from recorded provenance, validate, replace (rolls back on failure)
python "<skill-deployer-base-dir>/scripts/deploy_skill.py" --update "<skill-name>" --scope workspace --workspace "<workspace-root>"

# Delete the installed folder (never touches system_skills/)
python "<skill-deployer-base-dir>/scripts/deploy_skill.py" --remove "<skill-name>" --scope global
```

If an updated skill contains scripts, get user approval and pass `--approve-scripts`.

## Post-Install Verification

On the **next agent turn**, check `<available_skills>` and report to the user:

1. The skill name appears (or is gone after `--remove`).
2. `source` matches the intended scope (`workspace`, `assistant`, or `global`).
3. `<location>` matches the deployed path (`user_skills` for global, not `system_skills`).
4. Source URL and pinned commit SHA are recorded in `.skill-provenance.json`.

Skills merge first-wins by lowercase name — workspace > assistant > agent import > global > system — so a workspace skill silently overrides a global one with the same name.

## Common Mistakes

- **Improvising with `git clone` or `workspace__runShell`** — use `deploy_skill.py` so commits are pinned, validation runs, script approval is enforced, and provenance is recorded
- **Passing `--approve-scripts` without asking** — show the script list and wait for confirmation
- **Deploying to `system_skills/` or creating `.bundled_manifest.json`** — global custom skills go to `user_skills/`; custom skills use `.skill-provenance.json`
- **Workspace root deploy** — must be `.libragent/skills/{name}/`, not `{workspace}/{name}/`
- **Wrong global path** — use `com.fritzprix.libragent/user_skills/`, not legacy `LibrAgent/skills/`
- **Skipping validation** — invalid YAML is silently ignored by the Rust scanner
- **Name ≠ folder name** — breaks `@skill:name` lookup and fails `--strict` validation
- **Expecting immediate UI refresh** — skills reload at the start of the next agent turn
