# 💻 Coding Standards & Validation (`coding-standards.md`)

> **Note for AI Agents**: Read this guide when writing, refactoring, or reviewing code, ensuring type safety, handling errors, or running test/build validation pipelines.

---

## 🎨 Code Style & Quality Standards

- **Formatting**: `{formatter_info}` (e.g., Prettier / rustfmt)
- **Linting**: `{linter_info}` (e.g., ESLint / Clippy)
- **Indentation & Naming**:
  - `{indent_and_naming_rules}`
- **Comment Style**: English, clear JSDoc / doc comments for public APIs.

---

## 🛡️ Type Safety Principles

- **No Blind Type Assertions**: Validate shapes before casting.
- **No `any`**: Use precise types, `unknown` with type guards, or Zod schemas for validation.
- **No Unsafe JSON Parsing**: Validate JSON payloads with schemas or typed parsers.
- **Explicit Backend Types**: Keep frontend API interfaces synced with backend models.

---

## 🚨 Error Handling & Logging

- **Backend Commands**: Return structured `Result<T, E>` types.
- **Frontend IPC Calls**: Wrap Tauri/IPC invocations in safe wrappers with centralized error logging.
- **Logging**: Use centralized logger modules (`getLogger('Name')`) instead of raw `console.log`.

---

## 🧪 Testing & Validation Pipeline

Before submitting changes, run validation:

```bash
# Linting & Formatting
{lint_command}

# Unit / Integration Tests
{test_command}

# Complete Pipeline Validation
{validation_command}
```
