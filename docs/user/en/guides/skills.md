---
title: Skills
---

# Skills

> Skills are reusable procedures you inject with `@skill:…` mentions.

---

## How to use

In Chat, type `@skill:` and pick a skill (e.g. `@skill:docx`, `@skill:setup-wizard`).

The skill content is added to context so the agent follows that procedure.

---

## Scopes

| Scope          | Meaning                                              |
| -------------- | ---------------------------------------------------- |
| Built-in       | Shipped with LibrAgent                               |
| User / project | Skills you add under the configured skills directory |

Manage local skills folder in sidebar **Extensions → Skills**.

---

## Examples

| Skill           | Use                            |
| --------------- | ------------------------------ |
| `setup-wizard`  | Runtime / environment guidance |
| `docx` / `pptx` | Document workflows             |
| Domain skills   | Your team procedures           |

---

## Tips

- Mention only skills relevant to the current task.
- Keep custom skills short and actionable.
- Pair with [Assistants](assistants.md) that already expect those skills.

---

## Related

- [Sub-agents](sub-agents.md) · [5-minute tutorial](../getting-started/5-minute-tutorial.md)
