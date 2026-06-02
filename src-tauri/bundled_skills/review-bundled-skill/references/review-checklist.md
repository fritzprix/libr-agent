# Bundled Skill Review Checklist

Use this checklist when the target skill touches deployment, path guidance, or helper scripts.

## Backend grounding

- `src-tauri/build_support/bundled_skills.rs`
  - Only directories with top-level `SKILL.md` are mirrored as bundled skills
- `src-tauri/src/lifecycle/app_setup.rs`
  - Managed `system_skills` sync compares per-skill hashes and replaces changed directories
- `src-tauri/src/services/skill_service/directories.rs`
  - The managed system skill path is different from user, assistant, and workspace skill paths
- `src-tauri/src/agent/references/skill.rs`
  - `@skill:name` injects an absolute Base Directory and tells the agent how to resolve relative paths

## Red flags

### Path-domain bugs

- `scripts/...` or `references/...` presented as if they were workspace-relative
- `python scripts/run.py --root .` without clarifying that the script is skill-relative but `--root .` is workspace-relative
- Mentions of repo-only paths such as `src-tauri/bundled_skills/...` or `.github/skills/...`
- Vague wording like "project root" when the actual runtime path domain is the skill Base Directory

### Deployment/update bugs

- No `.force_update` after a change where stale installed copies matter
- Runtime-generated hints still emit obsolete commands or paths
- References to packaged or managed storage that contradict the real sync flow

### Bundle hygiene bugs

- Template TODOs or example files still present
- Resource paths cited in `SKILL.md` that do not exist
- Large reference material duplicated verbatim in `SKILL.md`

## Suggested report shape

| Severity | File | Issue | Why it matters | Fix |
| --- | --- | --- | --- | --- |
| major | `SKILL.md` | `python scripts/run.py --root .` is ambiguous | Script path and workspace target use different bases | Use `<skill-base-dir>/scripts/run.py --root .` or explicitly change directories |

If there are no issues, say the skill is clean and mention the exact surfaces you checked.
