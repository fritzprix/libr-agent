---
name: skill-creator
description: Guide for creating effective skills. This skill should be used when users want to create a new skill (or update an existing skill) that extends Claude's capabilities with specialized knowledge, workflows, or tool integrations.
license: Complete terms in LICENSE.txt
---

# Skill Creator

Guidance for building lean, effective agent skills.

## Quick Process

1. **Understand** — gather concrete usage examples (see [creation-process.md](references/creation-process.md) Step 1)
2. **Plan** — identify scripts, references, and assets to bundle
3. **Initialize** — `scripts/init_skill.py <skill-name> --path <output-directory>` (new skills only)
4. **Implement** — build resources, write SKILL.md, test scripts
5. **Package** — `scripts/package_skill.py <path/to/skill-folder>`
6. **Iterate** — refine after real usage

## Core Rules

- **Concise body**: SKILL.md under ~150 lines; move detail to `references/`
- **Triggers in frontmatter**: all "when to use" guidance goes in `description`, not the body
- **No duplicate reads**: information lives in SKILL.md *or* references, not both
- **Imperative voice** in body instructions
- **No auxiliary docs**: no README, CHANGELOG, or INSTALLATION_GUIDE in the skill folder

## Structure

See [anatomy.md](references/anatomy.md) for directory layout (`SKILL.md`, `scripts/`, `references/`, `assets/`).

## Design Guidance

- [design-principles.md](references/design-principles.md) — concision, degrees of freedom, progressive disclosure
- [creation-process.md](references/creation-process.md) — full 6-step walkthrough
- [workflows.md](references/workflows.md) — sequential and conditional workflow patterns
- [output-patterns.md](references/output-patterns.md) — template and example patterns

## Frontmatter Checklist

```yaml
---
name: skill-name
description: What it does + specific triggers/contexts for when to use it.
---
```

Only `name` and `description` in YAML. The description is the primary trigger mechanism.
