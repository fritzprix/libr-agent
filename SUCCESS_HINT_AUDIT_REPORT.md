# LibrAgent Builtin Tool Success Hint & Failure Recovery — Unified Plan

This document merges the Korean semantic audit and the full-codebase `SuccessHint` inventory into **one implementation model**. Status: **implemented** in `hint_headers` (`guidance.rs`) and rolled through central formatters + high-traffic domain handlers.

## Design pattern: semantic & tone separation

| Context                     | Role                                          | Legacy (problematic)                   | Unified header                                | Tone                                         |
| :-------------------------- | :-------------------------------------------- | :------------------------------------- | :-------------------------------------------- | :------------------------------------------- |
| **Failure recovery**        | Error / failed tool execution                 | `💡 Next Steps:`, `💡 Next:`           | `💡 Suggested Recovery:`                      | Corrective — clear recovery steps (numbered) |
| **Informational notice**    | Non-error guidance (timeout, internal notice) | `💡 Next Steps:`                       | `💡 Optional Guidance:`                       | Soft — optional context, not a failure       |
| **Success hint**            | Tool succeeded; optional context              | `💡 Next:`, `Required next:`           | `💡 Suggested Follow-ups:`                    | Optional — bullet list, no `or` join         |
| **Static tool description** | Schema text before invocation                 | `💡 Next Steps:`, `CRITICAL WORKFLOW:` | `💡 Related Actions:`, `💡 Example workflow:` | Reference — cookbook, not a command queue    |

### Implementation rules

1. **Central formatters only** — `SuccessHint::to_mcp_result_with_data`, `ErrorGuidance::to_mcp_result`, `tool_description()`, `tool_execution` parse-error path use `hint_headers::*` constants.
2. **Success follow-ups** — bullet list under `Suggested Follow-ups:`; never join with `or` (that implied one mandatory choice).
3. **No routine edit promotion** — `writeFile` create success does not append generic `strReplace` / `editFile` follow-ups; path collision and overwrite boundary notes stay factual in body or follow-ups when contextually required.
4. **Soften imperative body text** — prefer `X can …` / `— inspect` bullets over `Use X to …` in success paths; errors may still name tools in recovery steps.
5. **Tests** — assert new headers; forbid legacy `💡 Next:` / `Next Steps:` / `Required next:` on read-only success paths.

### Source of truth (Rust)

```text
src-tauri/src/mcp/builtin/error_guidance/guidance.rs  → hint_headers + SuccessHint + ErrorGuidance
src-tauri/src/mcp/builtin/tool_description.rs         → Example workflow + Related Actions
src-tauri/src/agent/llm/tool_execution.rs             → Suggested Recovery (parse errors)
```

### Domain handlers updated in this pass

- `workspace/file_operations/write.rs` — collision / truncation / overwrite boundary (no routine edit promotion)
- `workspace/file_operations/edit_line/response.rs` — factual follow-ups
- `scheduled_task/handlers.rs` — Available operations / Tip / softened verbs
- `agent/handlers.rs` — `Recommended follow-up` (not `Required next`)
- `browser/tools.rs`, `browser/interaction.rs`, `setup_wizard/tools.rs`, `scratchpad/handlers.rs`

### Remaining backlog (optional follow-up)

- Default guidance strings inside `get_default_guidance()` still use `Use X` imperatives — acceptable for **error recovery** content; can be softened incrementally.
- `SuccessHint::for_tool()` suggestion catalog — static strings for browser/planning; not wired into every success response.
- Bundled skills / docs outside `src-tauri` — not governed by `hint_headers`.

---

## Original audit (reference)

The sections below retain the detailed inventory and issue classification from the initial audit. **Line numbers and format strings may be stale** after implementation; refer to `hint_headers` and the formatters above for current behavior.
