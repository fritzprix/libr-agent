# Design Principles

## Concise is Key

The context window is a public good. Skills share it with the system prompt, conversation history, other skills' metadata, and the user request.

**Default assumption: the agent is already very smart.** Only add context it does not already have. Challenge each paragraph: "Does this justify its token cost?"

## Degrees of Freedom

Match specificity to task fragility:

- **High freedom** (text instructions): multiple valid approaches, context-dependent decisions
- **Medium freedom** (pseudocode/scripts with parameters): preferred pattern exists, some variation OK
- **Low freedom** (specific scripts, few parameters): fragile operations, consistency critical

## Progressive Disclosure

Three-level loading:

1. **Metadata** (name + description) — always in context
2. **SKILL.md body** — when skill triggers
3. **Bundled resources** — as needed (scripts may run without loading into context)

Keep SKILL.md under ~150 lines. Split detailed content into `references/`. Link references one level deep from SKILL.md.

### Patterns

- **High-level guide + references**: core workflow in SKILL.md, variants in separate files
- **Domain-specific organization**: `references/finance.md`, `references/sales.md`, etc.
- **Conditional details**: basic steps in SKILL.md, advanced in linked files

Avoid deeply nested references. For files >100 lines, add a table of contents at the top.

## What NOT to Include

Do not add README.md, CHANGELOG.md, INSTALLATION_GUIDE.md, or other auxiliary docs. Only include files that help the agent do the job.
