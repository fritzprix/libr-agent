---
name: review-bundled-skill
description: Review bundled skills under src-tauri/bundled_skills for deployment correctness, path semantics, and stale-install risks. Use when auditing an existing bundled skill, especially for Base Directory versus workspace cwd confusion, repo-only path leaks, misleading script examples, bundled resource issues, or missing .force_update markers.
---

# Review Bundled Skill

Audit a bundled skill against how LibrAgent actually ships, syncs, and exposes it to agents.

## Review workflow

1. Read the target `SKILL.md` and list its top-level files.
   - Inspect any helper script that prints runtime hints, sample commands, or reports.

2. Check metadata and bundle hygiene.
   - `name` should be hyphen-case.
   - `description` should clearly state triggers.
   - No TODO blocks, example files, or dead resources should remain.

3. Check path semantics.
   - Internal bundled paths should be skill-Base-Directory-relative.
   - Workspace paths should be called out separately.
   - Repo-only paths like `src-tauri/bundled_skills/...` or `.github/skills/...` should not leak into instructions.
   - If examples mix bundled scripts and workspace targets, they should use `<skill-base-dir>/...` or an explicit directory change.

4. Check deployment and update behavior.
   - Remember the repo copy is not the live runtime path; managed `system_skills` is.
   - Confirm whether `.force_update` is present when the installed managed copy likely needs a forced refresh.
   - Treat `.force_update` as a hash bump, not a magical runtime flag.

5. Validate focused surfaces.
   - Run `python .github\skills\skill-creator\scripts\quick_validate.py <skill-dir>`
   - Syntax-check any touched helper script
   - Read the diff instead of trusting the docs by vibes

6. Report findings clearly.
   - Use severity labels like `critical`, `major`, or `minor`
   - For each finding, name the file, the concrete issue, why it matters at runtime, and the fix
   - If the skill is clean, say so plainly instead of inventing noise

## Review grounding

Read `references/review-checklist.md` when you need the backend file map, deployed path model, or concrete red flags.
