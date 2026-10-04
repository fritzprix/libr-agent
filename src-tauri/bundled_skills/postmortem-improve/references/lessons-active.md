# LESSONS.active.md format

Path: `@harness/LESSONS.active.md`  
Storage: `{appData}/harness-lessons/<scopeId>/LESSONS.active.md` (outside project git)

Injected into the system prompt as `## Active Operational Lessons` when non-empty.
Harness truncates beyond 30 lines / 2000 bytes.

## Template

```markdown
<!-- Max 5 rules. Evict before adding more. Behavior track only. No ATX headers. -->

- [YYYY-MM-DD] TRIGGER: <context when this applies> | FORBIDDEN: <anti-pattern> | REQUIRED: <correct action>
```

## Examples

```markdown
- [2026-10-04] TRIGGER: browser createSession without backend pin | FORBIDDEN: omit browser= | REQUIRED: pass browser="userChrome" or browser="sidecar" explicitly
- [2026-10-04] TRIGGER: child claims done | FORBIDDEN: accept without acceptance proof | REQUIRED: require matrix cells + evidence quotes in final text
```

## Write API

- Prefer `workspace__writeFile(..., mode="overwrite")` or `workspace__editFile`.
- Default `mode="create"` will allocate `LESSONS.active-1.md` when the file exists — **that file is not injected**.

## Anti-patterns

- Vague coaching ("be careful", "double-check")
- Rules that paper over tool contract bugs (use defect track instead)
- HTML/XML, ATX headers (`# ...`), or instruction-override phrasing (headers are stripped on inject)
- Copying full postmortem prose into this file
