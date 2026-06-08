# Anatomy of a Skill

```
skill-name/
├── SKILL.md (required)
│   ├── YAML frontmatter: name, description (required)
│   └── Markdown instructions
└── Bundled Resources (optional)
    ├── scripts/      — deterministic executable code
    ├── references/   — docs loaded into context as needed
    └── assets/       — templates/files used in output (not loaded into context)
```

## SKILL.md

- **Frontmatter**: `name` and `description` only. The description is the primary trigger—put all "when to use" guidance here, not in the body.
- **Body**: procedural instructions loaded after triggering.

## Scripts (`scripts/`)

Include when the same code would be rewritten repeatedly or deterministic reliability is needed. May still be read for patching or environment adjustments.

## References (`references/`)

Include for schemas, API docs, domain knowledge, detailed workflow guides. Keeps SKILL.md lean. Avoid duplicating content between SKILL.md and references—prefer references for detail.

## Assets (`assets/`)

Templates, images, boilerplate code used in output. Not loaded into context.
