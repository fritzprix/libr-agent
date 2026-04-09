# Gemini Request Caching Implementation

This document explains how LibrAgent's Gemini client shapes requests, decides when to use explicit cached-content, and manages cache lifetime across normal request cleanup versus real service teardown.

## Why this exists

Gemini explicit caches only help if the stable part of the prompt stays reusable across turns. In this codebase, that was easy to accidentally break because the frontend routinely disposes the active service between requests. The current implementation keeps prompt caches alive across routine disposal, but still purges them when the factory actually tears Gemini instances down.

## Main files

| File                                                | Responsibility                                                                                     |
| --------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| `src\lib\ai-service\gemini\service.ts`              | Gemini request shaping, cache eligibility, cache creation/reuse, fallback behavior, cache eviction |
| `src\lib\ai-service\gemini\cache-store.ts`          | Shared in-memory registry for Gemini cached-content handles                                        |
| `src\lib\ai-service\factory.ts`                     | Real Gemini teardown hooks that purge shared cache state                                           |
| `src\context\llm\useExecuteCompletion.ts`           | Frontend lifecycle that disposes the previous service before the next turn                         |
| `src\lib\ai-service\__tests__\gemini.cache.test.ts` | Regression coverage for cache lifecycle, request shape, thresholds, and fallback behavior          |

## Mental model

There are two different lifetimes in play:

1. **Request/service lifetime**: the UI may dispose a Gemini service between turns.
2. **Provider cache lifetime**: Gemini explicit caches should survive that routine cleanup so the next turn can reuse them.

The implementation treats cached-content entries as **provider-level assets**, not as request-scoped state.

## Request shaping flow

For each Gemini chat request, `GeminiService` does this:

1. Convert MCP tools into Gemini `functionDeclarations`.
2. Build the Gemini request from:
   - `stablePrefix`: the stable system prompt portion
   - `geminiTools`: serialized tool declarations
   - `geminiMessages`: conversation contents sent for this turn
3. Decide whether the stable prefix is large enough to justify explicit cached-content.
4. Reuse an existing cached-content entry if the same stable prefix + tool declaration payload was already cached.
5. Otherwise create a new Gemini explicit cache and send the request using `config.cachedContent`.

## Stable vs volatile prompt content

The cache only works if we keep volatile context out of the cached prefix.

- **Stable content** stays in the Gemini system instruction / explicit cache.
- **Volatile session context** is moved into a synthetic tail user message so it does not poison cache reuse.

That is why the Gemini tests explicitly check the "synthetic tail message" behavior for session context.

## Cache eligibility

`GeminiService.shouldAttemptContextCache()` estimates whether the cacheable prefix is large enough by combining:

- stable system prompt bytes
- serialized tool declaration bytes
- per-tool overhead

The implementation intentionally counts tool declarations toward cache eligibility because Gemini pays prompt cost for them too.

### Model-aware thresholds

The minimum cacheable prefix is model-aware:

| Model family                         | Minimum estimated prefix tokens |
| ------------------------------------ | ------------------------------- |
| `gemini-2.5-flash`, `gemini-3-flash` | `1024`                          |
| `gemini-2.5-pro`, `gemini-3-pro`     | `4096`                          |
| older / unknown Gemini models        | `32768`                         |

The fallback stays conservative on unknown model IDs so we do not over-create caches for models whose behavior is unclear.

## Cache key and namespace

The cache key is derived from:

- model name
- stable hash of the stable system prompt
- stable hash of the serialized Gemini tool payload

The shared store is namespaced by API key, so different Gemini credentials do not share cached-content handles.

## Shared cache store

`cache-store.ts` is the in-memory registry for Gemini explicit cache handles. Each entry stores:

- Gemini cached-content name
- creation time
- last-used time

The store exists so multiple `GeminiService` instances can reuse the same Gemini cache entry across routine disposal.

## Lifecycle contract

This is the part people are most likely to break by accident.

### Routine `GeminiService.dispose()`

`GeminiService.dispose()` is intentionally **non-destructive**.

Why: `useExecuteCompletion.ts` disposes the previous active service before the next turn. If `dispose()` deleted Gemini caches, cross-turn reuse would die immediately.

### Real teardown in `AIServiceFactory`

Real cache deletion happens only when the factory discards Gemini instances for real:

- replacing an expired instance
- `disposeAll()`
- TTL cleanup

At those points, `AIServiceFactory` calls `GeminiService.purgeSharedContextCache(apiKey)`, which clears the shared in-memory namespace and asynchronously deletes the corresponding remote Gemini caches.

### Entry-level eviction

Inside `GeminiService`, individual cache entries can still be removed when:

- the entry exceeds `CONTEXT_CACHE_TTL_MS` (`55 minutes`)
- the namespace exceeds `MAX_CONTEXT_CACHE_ENTRIES` (`8`), in which case the least-recently-used entry is evicted

## Tool-calling contract

There are three important request-shape rules.

### 1. Forced tool use blocks cached-content

If `forceToolUse === true`, the request is treated as requiring a tool override and explicit cached-content is skipped.

### 2. Disabled tool use still keeps tool visibility

If `disableToolUse === true`, Gemini should still receive:

- `tools`
- `toolConfig.functionCallingConfig.mode = NONE`

That keeps tool declarations visible to the model while still disabling actual function calling for the request.

This matters for compaction and other tool-disabled flows that still want the same prompt layout for cache reuse.

### 3. Cached-content + disabled tool use may need fallback

The happy path now allows `cachedContent` and disabled tool use together. If Gemini rejects that combination as an invalid request, the client retries once without `cachedContent` and keeps the tool visibility / `NONE` contract intact.

## Why compaction benefits now

The Rust side preserves prompt-layout inputs for compaction requests, including tool visibility. The Gemini frontend now matches that design by allowing explicit cached-content reuse when tool use is disabled, instead of throwing cache reuse away just because `disableToolUse` is set.

## Regression tests to read first

If you want the fastest possible orientation, start with these tests in `src\lib\ai-service\__tests__\gemini.cache.test.ts`:

- `retains cached-content entries across GeminiService disposal`
- `survives the frontend factory lifecycle that disposes before the next turn`
- `purges shared cached-content entries on real factory teardown`
- `keeps cached-content mode when tool usage is explicitly disabled`
- `keeps Gemini tool declarations visible when tool usage is disabled and cache is skipped`
- `falls back once without cachedContent when Gemini rejects tool-disabled cached content`
- threshold tests for flash / pro / older / unknown models

Those tests are the contract. If one fails after a refactor, assume the refactor is wrong until proven otherwise.

## Maintenance checklist

When changing Gemini request shaping or lifecycle logic, verify these assumptions still hold:

1. Routine service disposal does **not** delete reusable caches.
2. Real factory teardown **does** purge shared cache state.
3. Cache keys still include both stable prompt content and tool declarations.
4. Volatile session context stays out of the cached prefix.
5. `disableToolUse` keeps tool declarations visible.
6. The fallback path only removes `cachedContent`, not the rest of the request contract.
7. Model-aware thresholds stay conservative for unknown Gemini models.

If you break any of those, cache hit quality will quietly get worse, and that kind of regression is a pain in the neck to notice from UI behavior alone.
