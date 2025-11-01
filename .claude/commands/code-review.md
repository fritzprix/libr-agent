---
argument-hint: '[optional: focus area]'
description: Review changes for correctness, safety, and consistency with LibrAgent guidelines
---

# Code review checklist

Review the referenced files (or recent changes) and provide a concise, actionable review.

## Focus

- Correctness and edge cases
- TypeScript strictness (no `any`; prefer precise types; use `unknown` if needed)
- Logging: use `getLogger` instead of console methods
- Rust: correct method vs associated function usage (self vs TypeName::function)
- Security: input validation, permission checks (Tauri allowlist), key handling
- Performance: avoid unnecessary re-renders; async/non-blocking; avoid heavy sync I/O
- Styling: Tailwind utility classes, no arbitrary class names unless safelisted

## Output format

- Summary (2–3 lines)
- Findings (bulleted) with file/line refs when possible
- Quick fixes (bulleted)
- Optional refactors (bulleted)

If the user provides a focus area argument (e.g., "rust", "frontend", "mcp"), weigh the review accordingly.
