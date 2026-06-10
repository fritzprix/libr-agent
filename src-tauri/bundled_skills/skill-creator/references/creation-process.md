# Skill Creation Process

Follow these steps in order unless clearly inapplicable. **Do not deploy until Step 5 passes.**

## Step 1: Understand with Concrete Examples

Clarify how the skill will be used—direct user examples or validated generated examples. Ask focused questions; avoid overwhelming the user in one message.

Conclude when the supported functionality is clear.

## Step 2: Plan Reusable Contents

For each example, identify scripts, references, and assets that would help on repeat execution:

- Repeated code → `scripts/`
- Schemas/API docs → `references/`
- Output templates → `assets/`

## Step 3: Scaffold the Skill Directory

Create `{skill-name}/SKILL.md` with valid frontmatter. The folder name and frontmatter `name` must match.

Use hyphen-case names only (e.g. `my-new-skill`).

## Step 4: Edit the Skill

1. Implement scripts/references/assets first (test scripts before finalizing).
2. Delete unused template or example files.
3. Write SKILL.md in imperative form.

**Frontmatter**: `name` + comprehensive `description` (triggers + use cases). See [validation.md](validation.md) for YAML rules.

**Body**: procedural instructions only — no deployment paths (that belongs in skill-deployer).

Consult [workflows.md](workflows.md) for multi-step patterns and [output-patterns.md](output-patterns.md) for output templates.

## Step 5: Validate

```bash
python scripts/validate_skill.py <path/to/skill-folder>
```

Fix all errors. Before deployment, run with `--strict` and fix warnings too:

```bash
python scripts/validate_skill.py <path/to/skill-folder> --strict
```

See [validation.md](validation.md) for the full checklist.

## Step 6: Deploy

Use **skill-deployer** to install the validated skill into the correct scope (`workspace`, `assistant`, or `global`).

Do not skip validation — invalid frontmatter causes the Rust scanner to silently ignore the skill.

## Step 7: Iterate

1. Use on real tasks
2. Note struggles or inefficiencies
3. Update SKILL.md or bundled resources
4. Re-run `validate_skill.py` after edits
