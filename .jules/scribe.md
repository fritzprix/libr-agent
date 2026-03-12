**Reality:** Updated `@param` to include `{T}` and `{number}` and documented `@template T`.

## 2026-03-08 - src/hooks/useThrottle.ts

**Drift:** JSDoc `@param callback` and `@param delay` did not accurately reflect the TypeScript generic signature `T` and missing parameter types in comments.
**Reality:** Updated `@param` to include `{T}` and `{number}` and documented `@template T`.

## 2026-03-08 - src-tauri/src/[mcp/builtin/utils.rs, mcp/service_proxy_manager/creation.rs, repositories/in_memory_session_repository.rs, utils/json.rs]

**Drift:** Rust documentation generated warnings due to unresolved links (`[validate_path]`, `[call_tool]`) and unclosed HTML tags (`Arc<RwLock>`, `Option<String>`, `Option<T>`).
**Reality:** Fixed the links to correctly resolve to their respective struct methods and wrapped the HTML-like types in backticks (e.g., `Arc<RwLock>`) to eliminate rustdoc warnings.

## 2024-11-20 - src/lib/ai-service/anthropic.ts **Drift:** `@param messages` on `convertMessages` **Reality:** `messages, systemPrompt`
## 2024-11-20 - src/lib/ai-service/base-service.ts **Drift:** `@param options` with `@param options.config` on `mergeConfig` **Reality:** `options` does not have `config` property
