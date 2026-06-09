# Skill Validation

Run validation **before** handing off to `skill-deployer`. The Rust scanner silently skips invalid skills — validation catches problems early.

## Command

From the deployed skill-creator Base Directory:

```bash
python scripts/validate_skill.py <path/to/skill-folder>
```

Use `--strict` before deployment (treats path warnings as errors):

```bash
python scripts/validate_skill.py <path/to/skill-folder> --strict
```

Use `--detailed` for error codes and fix instructions (recommended when debugging):

```bash
python scripts/validate_skill.py <path/to/skill-folder> --strict --detailed
```

Each issue prints `[CODE]`, the problem, and a `Fix:` line (e.g. `SKILL-YAML-APOSTROPHE`, `SKILL-PATH-WORKSPACE-ROOT`).

In the LibrAgent repo (authoring bundled skills):

```bash
python src-tauri/bundled_skills/skill-creator/scripts/validate_skill.py src-tauri/bundled_skills/<skill-name>
```

Requires PyYAML: `pip install pyyaml`

## What It Checks

### Errors (must fix)

- Missing `SKILL.md`
- Missing or malformed YAML frontmatter (`---` block)
- Invalid YAML (e.g. broken quoting in `description`)
- Missing or empty `name` / `description`
- Invalid `name` format (must be hyphen-case, max 64 chars)
- Unexpected frontmatter keys (only `name`, `description`, `license`, `allowed-tools`, `metadata` allowed)
- `description` with angle brackets or over 1024 characters
- Forbidden files in skill folder (`.bundled_manifest.json`, `README.md`, etc.)

### Warnings (review before deploy; errors with `--strict`)

- Frontmatter `name` differs from directory name
- `\'` inside single-quoted YAML (use `"Telegram's"` or `Telegram''s`)
- Skill placed under `system_skills/` manually (removed on app startup unless bundled)
- Skill at workspace session root instead of `.libragent/skills/`
- Legacy `skills/` instead of `user_skills/` or `.libragent/skills/`
- Parent directory has `.bundled_manifest.json` (ignored outside `system_skills`)

## Frontmatter Rules

```yaml
---
name: skill-name
description: "What it does and when to use it. Use double quotes when the text contains apostrophes."
---
```

- **name** must match the folder name exactly
- **description** holds all trigger phrases — not the body
- Prefer double-quoted `description` when text contains `'`

## When to Validate

| Stage | Command |
| --- | --- |
| After editing SKILL.md | `validate_skill.py <folder>` |
| Before deploy (skill-deployer) | `deploy_skill.py` runs strict validation automatically; or `validate_skill.py <folder> --strict --detailed` |
| Before bundling in repo | `validate_skill.py src-tauri/bundled_skills/<name> --strict` |

## After Validation

- Fix all errors before continuing
- With `--strict`, fix warnings too before deployment
- Deployment scope and paths → use **skill-deployer**, not this skill
