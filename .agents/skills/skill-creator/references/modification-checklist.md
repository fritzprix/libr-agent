# Skill Modification Checklist

Use when **editing, renaming, merging, or deleting** an existing skill (not only when creating one). Run this before commit / packaging.

Lessons distilled from incomplete renames and mixed skill-tree cleanups (e.g. umbrella merges, pruning unused skill trees).

## 0. Pick the correct tree

| Intent | Path |
| --- | --- |
| Product / runtime bundled skill | `src-tauri/bundled_skills/<name>/` |
| Cursor / agent-dev skill | `.agents/skills/<name>/` |
| Do not use | `.github/skills/` (obsolete duplicate; do not recreate) |

If a skill is listed in `scripts/check-skills-mirror-drift.cjs` (`MIRROR_MANIFEST`), edit `.agents/skills/<name>/` and sync identical content into `src-tauri/bundled_skills/<name>/`.

## 1. Entrypoint completeness (P0)

After rename/merge/move, verify **git index** — not only the working tree:

- [ ] `SKILL.md` is present and **staged** (untracked `SKILL.md` = broken discovery)
- [ ] Frontmatter `name:` equals the **directory name**
- [ ] Every path linked from `SKILL.md` exists (`references/…`, `scripts/…`)
- [ ] New/renamed bundled skills that must refresh managed `system_skills` include `.force_update` when that is the repo pattern
- [ ] Removed skills: no leftover empty dirs or junk logs under the old path

**Failure mode:** scripts/refs renamed and staged while `SKILL.md` stays `??` → commit ships a skill folder without an entrypoint.

## 2. Name residue

Search the repo for the **old skill name** (and aliases):

- [ ] Localized READMEs / product messaging / `docs/user/guides/skills.md` catalogs
- [ ] Other skills that mention the old name in body or description
- [ ] Scripts, CI, or mirror manifests

Ignored paths (e.g. `docs/analysis/` if gitignored) do **not** land in the commit — do not treat disk-only edits as shipped.

## 3. Description vs body

- [ ] `description` still lists accurate triggers for the new scope (merge → broaden; split → narrow)
- [ ] Body stays lean; moved detail lives in `references/`, with links from `SKILL.md`
- [ ] No “when to use” only in the body (triggers belong in frontmatter)

## 4. Validate before packaging / deploy

```bash
python scripts/validate_skill.py <path/to/skill-folder>
python scripts/validate_skill.py <path/to/skill-folder> --strict
```

For bundled skills in this repo, prefer the project hooks when committing:

- `node scripts/audit-bundled-skills.cjs`
- `node scripts/check-skills-mirror-drift.cjs` (mirrored skills only)

## 5. Commit hygiene

- [ ] Skill rename/merge **separate** from unrelated prompt/deps/tooling bumps
- [ ] Do not stage junk dirs (`.aibot/`, stray `.libragent/` copies, session logs)
- [ ] Do not bundle unrelated WIP agent skills into the same commit
- [ ] Final `git status`: no `??` under the skill directory you intended to ship

## 6. Quick behavioral check

- [ ] Available Skills / catalog shows the **new** name only (old name gone)
- [ ] Opening the skill loads `SKILL.md`; following a reference path succeeds
- [ ] Scripts invoked with `<skill-base-dir>` / `@system-skills/…` style paths still work after moves
