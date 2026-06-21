# Merge Patterns

This document defines techniques to assemble parallel subtask outputs back into a single coherent final result.

## 🔗 Merge Classifications

| Pattern | Description | Application | Example |
|---|---|---|---|
| **Concatenation** | Link texts or data sequentially | Docs, Translation | Combine multi-language translations into one report |
| **Assembly** | Merge non-overlapping code changes | Code changes | Integrate distinct file updates into the main codebase |
| **Aggregation** | Collect, dedup, and structure results | Audit, Research | Gather vulnerabilities into a unified technical report |
| **Integration** | Align interfaces and glue components | Architecture integration | Bind frontend API calls with the new backend service |

---

## 📝 Subtask Return Format

To ensure consistent output parsing, instruct child sessions to use this return schema:

```markdown
### 1. Subtask Metadata
- **Subtask ID:** [e.g., Subtask-01]
- **Status:** Completed

### 2. Changes & Artifacts
- **Files Changed:**
  - `path/to/modified/file.ts` (modified)
  - `path/to/new/component.tsx` (created)
- **Artifact Paths:** [absolute path to generated output]

### 3. Execution Summary
- **Summary:** [Brief overview of modifications]
- **Verification Result:** [Build / test status]
```

## 🛠️ Merge Step Guide

1. **Verify Completion:** Ensure all child sessions are in `Completed` or `Terminal` states.
2. **Conflict Resolution:** If shared configuration files (e.g., `package.json`, `Cargo.toml`) conflict, manually align them in the parent session.
3. **Integration Test:** Run the build/lint script (`pnpm refactor:validate`) to verify merge integrity.
