# Skill Creation Process

Follow these steps in order unless clearly inapplicable.

## Step 1: Understand with Concrete Examples

Clarify how the skill will be used—direct user examples or validated generated examples. Ask focused questions; avoid overwhelming the user in one message.

Conclude when the supported functionality is clear.

## Step 2: Plan Reusable Contents

For each example, identify scripts, references, and assets that would help on repeat execution:

- Repeated code → `scripts/`
- Schemas/API docs → `references/`
- Output templates → `assets/`

## Step 3: Initialize the Skill

For new skills, run:

```bash
scripts/init_skill.py <skill-name> --path <output-directory>
```

Creates directory structure with template SKILL.md and example resource folders. Skip if iterating an existing skill.

## Step 4: Edit the Skill

1. Implement scripts/references/assets first (test scripts before finalizing).
2. Delete unused example files from initialization.
3. Write SKILL.md in imperative form.

**Frontmatter**: `name` + comprehensive `description` (triggers + use cases). No other YAML fields.

**Body**: procedural instructions only.

Consult `references/workflows.md` for multi-step patterns and `references/output-patterns.md` for output templates.

## Step 5: Package

```bash
scripts/package_skill.py <path/to/skill-folder> [./dist]
```

Validates frontmatter, naming, structure, then creates a `.skill` zip. Fix validation errors and retry.

## Step 6: Iterate

1. Use on real tasks
2. Note struggles or inefficiencies
3. Update SKILL.md or bundled resources
4. Re-test
