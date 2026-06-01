---
name: bundled-skill-creator
description: Create or update bundled skills that ship inside the LibrAgent repository under src-tauri/bundled_skills. Use when turning repeated repo-specific workflows into bundled skills, adding a new bundled skill, or fixing bundled-skill deployment, path, Base Directory, reference, or .force_update behavior.
---

# Bundled Skill Creator

Create bundled skills for LibrAgent's repo-shipped `src-tauri/bundled_skills` layer, not user-installed skills.

## Workflow

1. Confirm the skill belongs in the bundled layer.
   - Use a bundled skill when the capability should ship with the app for every install.
   - Do not use this skill for workspace-only or user-installed skills.

2. Scaffold in the repo.
   - Run `python .github\skills\skill-creator\scripts\init_skill.py <skill-name> --path src-tauri\bundled_skills`
   - Use hyphen-case names only.
   - Replace the template immediately. Do not leave TODO blocks or example files behind.

3. Keep the bundle lean.
   - Keep `SKILL.md` short and procedural.
   - Put implementation details in `references/` only when they materially help.
   - Add `scripts/` only for deterministic helpers the agent would otherwise rewrite.
   - Skip `assets/` unless the skill needs literal output resources.

4. Write for deployed reality, not repo illusion.
   - The repo source is `src-tauri/bundled_skills/<skill-name>/...`, but the agent reads the deployed copy from managed `system_skills`.
   - Internal relative paths mean the skill Base Directory, not the workspace root or shell `./`.
   - Never tell the agent to use repo-only paths like `src-tauri/bundled_skills/...` or `.github/skills/...`.
   - If examples mix bundled scripts with workspace targets, write them so the split is obvious. Prefer `python <skill-base-dir>/scripts/run.py --root .`.

5. Add update markers deliberately.
   - Add `.force_update` when you want a changed bundled skill to force one managed-copy refresh on the next app start.
   - `.force_update` is just a hash bump. It is not a dedicated runtime flag.

6. Validate narrowly.
   - Run `python .github\skills\skill-creator\scripts\quick_validate.py src-tauri\bundled_skills\<skill-name>`
   - Run focused checks for any new scripts.
   - Inspect the actual diff for leftover template junk, stale path wording, or missing references.

7. Skip packaging unless explicitly asked.
   - Repo bundled skills ship through the build mirror and runtime sync.
   - Do not create a distributable `.skill` file unless the user explicitly wants an importable package.

## Implementation notes

Read `references/bundled-skill-implementation.md` before writing deployment or path guidance, especially if the skill mentions scripts, managed storage, or review rules.

## Common mistakes

- Leaving template placeholders or example files in the new skill
- Writing path guidance as if workspace `./` equals the skill directory
- Describing repo paths instead of deployed managed paths
- Forgetting `.force_update` when stale installed copies matter
- Adding extra docs instead of keeping the skill lean
