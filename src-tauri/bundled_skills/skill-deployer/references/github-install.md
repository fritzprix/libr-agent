# GitHub Install Reference

Supported URL formats and the full `deploy_skill.py` flag list.

## Supported GitHub URL Formats

- Repository root: `https://github.com/<owner>/<repo>`
- Branch/tag/commit + subpath: `https://github.com/<owner>/<repo>/tree/<ref>/<subpath>`
- Direct `SKILL.md` link: `https://github.com/<owner>/<repo>/blob/<ref>/<subpath>/SKILL.md`
- Commit URL: `https://github.com/<owner>/<repo>/commit/<sha>`
- Query parameters: `https://github.com/<owner>/<repo>?ref=<ref>&path=<subpath>`
- Explicit flags: `--ref <branch-tag-or-sha>` and `--subpath <relative-path>` (use with the repository root URL; passing a conflicting `--ref` alongside `/tree/<other-ref>` is rejected)

Repository-level auxiliary files (`README.md`, `CHANGELOG.md`, `INSTALLATION.md`, `.github/`, `.gitignore`, etc.) are automatically omitted during staging and deployment so single-skill repositories pass strict layout validation cleanly.

## `deploy_skill.py` Flags

| Flag | Purpose |
| --- | --- |
| `--inspect` | Fetch/validate and show discovered skills, description, and scripts without installing |
| `--skill <name>` | Select a specific skill when a GitHub repo contains multiple `SKILL.md` folders |
| `--ref <ref>` | Override Git branch, tag, or commit SHA |
| `--subpath <path>` | Override relative subdirectory inside the GitHub repo |
| `--scope <workspace\|global\|assistant>` | Target scope |
| `--workspace <dir>` | Workspace root (required for `--scope workspace`) |
| `--assistant-id <id>` | Assistant ID (required for `--scope assistant`) |
| `--approve-scripts` | Confirm user approval to install a skill containing scripts/executables |
| `--overwrite` | Replace existing target directory |
| `--dry-run` | Strict validate source + print target without copying |
| `--data-dir <dir>` | Override default `com.fritzprix.libragent` data directory |
| `--update <name>` | Re-fetch from recorded provenance and replace the installed copy |
| `--remove <name>` | Delete the installed skill folder from the target scope |

## Provenance

`deploy_skill.py` writes `.skill-provenance.json` inside the installed skill folder (`sourceUrl`, `ref`, pinned 40-char `commit`, `subpath`, `scripts`, `installedAt`, `updatedAt`) and runs post-deploy strict validation with automatic rollback on failure.

When a skill was installed with a raw 40-character commit SHA as `ref`, `--update` re-fetches that same commit unless `--ref <branch>` switches it to tracking a branch.

## Manual Validation

```bash
python "<skill-creator-base-dir>/scripts/validate_skill.py" "<skill-folder>" --strict --detailed
```
