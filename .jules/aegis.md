# Aegis Safety Violation Log

## 2026-01-25 - src-tauri/src/mcp/builtin/browser/interaction.rs **Safety Breach:** `unwrap()` on `serde_json::to_string` inside `click_element` and `input_text`. **Fix:** Replaced with `map_err` to propagate errors properly.

## 2026-01-25 - src-tauri/src/mcp/builtin/browser/content.rs **Safety Breach:** `unwrap()` on `Regex::new` inside `convert_to_markdown` causing repeated recompilation and potential panic. **Fix:** Extracted to `static Lazy<Regex>` constants for compile-time-like validation and performance.

## 2026-01-25 - src/context/DnDContext.tsx **Safety Breach:** `as string` type assertions masking strict union types. **Fix:** Replaced with `as TauriDragDropPayload['type']` to enforce strict union contract.

## 2026-01-25 - src-tauri/src/mcp/builtin/playbook/operations.rs **Safety Breach:** `unwrap()` on `Option` in match guard. **Fix:** Replaced with `map_or(false, ...)` for safe boolean evaluation.

## 2026-01-29 - [src-tauri/src/lib.rs, src-tauri/src/state.rs] **Safety Breach:** unsafe { Arc::from_raw(...) } used to recreate Arc from static reference **Fix:** Changed static storage to OnceLock<Arc<MCPServiceProxyManager>> for safe cloning

## 2026-02-04 - src/features/agent/AgentDraftChatView.tsx **Safety Breach:** `as unknown as Assistant` casting used to flatten backend DTO. **Fix:** Implemented `parseAssistant` with Zod validation in `src/models/validation.ts`.

## 2026-02-04 - src-tauri/src/mcp/builtin/workspace/file_operations/edit_replace.rs **Safety Breach:** `unwrap()` on `Option` in boolean logic. **Fix:** Replaced with `map_or(true, ...)` for safe boolean evaluation.

## 2026-02-05 - src/context/AgentChatContext.tsx **Safety Breach:** `as Message` type assertion masking potential partial data in streaming messages. **Fix:** Implemented `isValidMessage` type guard in `src/models/validation.ts` to strictly validate message shape.

## 2026-02-05 - src/context/AgentSessionListContext.tsx **Safety Breach:** `as { ... }` assertions and snake_case fallbacks masking backend contract. **Fix:** Removed assertions and fallbacks, enforcing strict adherence to backend camelCase contract.
