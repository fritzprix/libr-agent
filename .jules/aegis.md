# Aegis Safety Violation Log

## 2026-01-25 - src-tauri/src/mcp/builtin/browser/interaction.rs **Safety Breach:** `unwrap()` on `serde_json::to_string` inside `click_element` and `input_text`. **Fix:** Replaced with `map_err` to propagate errors properly.

## 2026-01-25 - src-tauri/src/mcp/builtin/browser/content.rs **Safety Breach:** `unwrap()` on `Regex::new` inside `convert_to_markdown` causing repeated recompilation and potential panic. **Fix:** Extracted to `static Lazy<Regex>` constants for compile-time-like validation and performance.

## 2026-01-25 - src/context/DnDContext.tsx **Safety Breach:** `as string` type assertions masking strict union types. **Fix:** Replaced with `as TauriDragDropPayload['type']` to enforce strict union contract.

## 2026-01-25 - src-tauri/src/mcp/builtin/playbook/operations.rs **Safety Breach:** `unwrap()` on `Option` in match guard. **Fix:** Replaced with `map_or(false, ...)` for safe boolean evaluation.

## 2026-01-26 - src/features/agent/AgentDraftChatView.tsx **Safety Breach:** "Lying Type" assertion on backend response (`as Assistant`) masking missing Date conversion for `createdAt`/`updatedAt`. **Fix:** Implemented `src/models/validation.ts` with Zod schemas and `parseAssistant` for strict runtime validation and transformation.
