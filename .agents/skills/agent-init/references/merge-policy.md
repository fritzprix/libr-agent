# Merge Policy for Existing Guideline Files

Apply when `agents.md` (or `AGENTS.md` / `CLAUDE.md` / `GEMINI.md`) already exists in the workspace root.

## Overwrite (full replace)

Replace the entire file when **all** of the following are true:

- File is empty, OR
- Fewer than 20 non-blank lines, OR
- Content is clearly a generic stub (only title + placeholder bullets, no project-specific paths or commands)

## Merge (preserve custom content)

When the file has substantive custom content:

1. **Preserve** all existing sections and their order
2. **Append** template sections that are missing (do not duplicate headings)
3. **Do not replace** existing section bodies unless the user explicitly asked to regenerate
4. On heading collision: keep the existing section; add a `<!-- agent-init: suggested -->` comment block below with the new draft, OR ask the user if unsure

## Multiple guideline files

If both `agents.md` and `AGENTS.md` exist:

- Update the file that appears referenced in `README.md` or tooling config
- If unclear, update `agents.md` only and note the other file in a comment at the top

## After merge

- Remove any leftover `{placeholder}` tokens
- Ensure Quick Start / Development Scripts reflect real commands from the repo
