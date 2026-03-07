# Message Context Management

## Overview

LibrAgent manages the LLM context window through a **dual-constraint selection** mechanism: a count-based sliding window combined with a token-budget limit. The goal is to send the most relevant recent messages without exceeding the model's context window or causing API validation errors.

---

## Files Involved

| File                                       | Role                                                                                 |
| ------------------------------------------ | ------------------------------------------------------------------------------------ |
| `src/lib/token-utils.ts`                   | Core selection algorithm (`selectMessagesWithinContext`, `batchToolCallsInMessages`) |
| `src/context/llm/useLLMExecution.ts`       | Caller — computes token budget and invokes selection                                 |
| `src/lib/services/settings-service.ts`     | `Settings.windowSize` definition (default: **20**)                                   |
| `src/lib/ai-service/message-normalizer.ts` | Post-selection sanitization (tool pairing, field stripping)                          |
| `src/lib/ai-service/sanitizer.ts`          | JSON field safety (tool arguments, thinking content)                                 |

---

## Selection Pipeline

```
Input: messages[]  (full conversation history)
         │
         ▼
[1] batchToolCallsInMessages()
    Split assistant messages that exceed maxToolCallsPerMessage (default: 4)
    SKIP if message has thinkingSignature (Gemini atomic turn requirement)
         │
         ▼
[2] Token Budget Calculation
    baseLimit  = min(maxTokens, contextWindow × 0.9)
    reserved   = systemPromptTokens + toolsTokens + pinnedMessageTokens
    tokenLimit = max(1024, baseLimit − reserved)
         │
         ▼
[3] Pin First User Message
    Always include messages[0] if role === 'user' (context anchor)
         │
         ▼
[4] Backward Iteration (newest → oldest)
    For each message:
    ├── [TOKEN CHECK]   totalTokens + msgTokens > tokenLimit → STOP
    └── [COUNT CHECK]   selected.length ≥ windowSize − 1      → STOP
         │
         ▼
[5] removeIncompleteToolChains()   (Anthropic / Gemini / OpenAI / OpenRouter / Groq)
    Remove tool result messages with no matching tool call in selected set
    Trim tool_calls from assistant messages with no corresponding result
         │
         ▼
[6] prependPinnedMessage()
    If pinned message role === 'user' AND first selected role === 'user'
    → Merge content with separator to avoid consecutive user messages
         │
         ▼
[7] MessageNormalizer.sanitizeMessagesForProvider()
    + sanitizeMessage() (JSON escape, thinking field cleanup)
         │
         ▼
Output: contextMessages[]  (sent to LLM)
```

---

## Configuration

Defined in `Settings` (`src/lib/services/settings-service.ts`):

| Setting                           | Type     | Default | Description                                            |
| --------------------------------- | -------- | ------- | ------------------------------------------------------ |
| `windowSize`                      | `number` | `20`    | Maximum number of messages to include (count limit)    |
| `advanced.defaultMaxOutputTokens` | `number` | `8192`  | Reserved output budget, subtracted from context window |

`windowSize` is configurable in-app via **Settings → Chat Interface → Message Window Size**.

---

## Token Budget Formula

```
contextWindow         ← from llmConfigManager or dynamic detection (OpenRouter API / Ollama API)
reservedOutput        = defaultMaxOutputTokens + 100   (safety buffer)
safeInputTokenLimit   = contextWindow − reservedOutput

tokenLimit (in selectMessagesWithinContext)
  = max(1024, safeInputTokenLimit − systemPromptTokens − toolsTokens − pinnedMessageTokens)
```

If no model info is available, falls back to `contextWindow = 64K`.

---

## Provider-Specific Behaviour

| Provider                                    | Tool-chain integrity check            | Batching cap                    |
| ------------------------------------------- | ------------------------------------- | ------------------------------- |
| OpenAI, Anthropic, Gemini, Groq, OpenRouter | ✅ Yes — `removeIncompleteToolChains` | 4 tool calls/turn (Gemini: 100) |
| Ollama, Fireworks, Cerebras                 | ❌ No — break on token overflow       | 4 tool calls/turn               |

Gemini uses `maxToolCallsPerMessage: 100` to prevent splitting turns with a `thinkingSignature`.

---

## Known Limitations

1. **Drop-only, no compression** — messages outside the window are discarded entirely, not summarised.
2. **Single pinned anchor** — only the first user message is anchored; intermediate goal/summary messages can be dropped.
3. **No provider-aware budgeting** — Anthropic prompt caching and Gemini's 2M token window are not exploited differently.
4. **Count-based window is coarse** — a window of 20 may include 5 large tool results or 20 short user turns; token budget is the real guard.
5. **Estimation only** — `estimateTokensBPE` uses `cl100k_base` for all providers; actual token counts differ for Anthropic, Gemini, and Ollama.

---

## Proposed Design: Async Compact-Based Context Management

### Summary of Proposal

- Context length setting: model info first; if unavailable, 8K/16K/32K/64K/128K/256K slider fallback
- When input tokens exceed **90% of the context window**, trigger async compact
- `streamChat` continues processing normally while compact is in progress
- After compact completes: rebuild the stack as `[pinned, compacted_summary, recent_messages]`
- If 100% is reached before compact finishes: wait for compact to complete

---

### Assessment

The overall direction is correct. The key advantage is **alignment with prompt caching**. Anthropic's `cache_control` breakpoint yields cache hits when "the prefix is stable." If the compacted summary is anchored before the recent messages, cache hit rates improve significantly.

However, there are issues that need to be addressed in the concrete implementation.

---

### Issues to Address

#### 1. Blocking on 100% is a UX problem

> "If 100% is reached before compact finishes, wait for compact"

In this case, the user sees a non-responsive state. A **graceful fallback** is needed instead of a hard block.

**Recommendation:** When 100% is reached while compact is pending, use the existing `selectMessagesWithinContext` sliding window as a temporary fallback, and apply the compact result from the next request onwards.

#### 2. Lower the compact trigger threshold below 90%

A 90% trigger leaves almost no room for compact to finish before 100% is reached. The compact call itself is an LLM request and can take seconds to tens of seconds.

**Recommendation:** Lower the trigger to **75–80%**. If compact finishes in this range, the 100% blocking scenario effectively never occurs.

#### 3. Re-compact problem (lossy × lossy)

If the compact result is placed back on the message stack, the next compact cycle will try to compact that summary again. Summarising a summary degrades quality rapidly.

**Recommendation:** Add a `compacted: true` flag to compacted messages and always exclude them from compact selection. In other words, compact applies only to messages where `compacted: false`.

```typescript
// To be added to the Message model
interface Message {
  // ... existing fields
  compacted?: boolean; // marks this message as a compact summary
}
```

#### 4. Tool chain / thinking signature integrity

The current `batchToolCallsInMessages` prohibits splitting messages that have a `thinkingSignature`. Compact must enforce the same constraint.

When selecting "compaction target messages" to send to the compact LLM:

- Assistant messages with `tool_calls` and their tool results **must be compacted as a single unit** (splitting mid-chain causes Anthropic/Gemini API errors).
- Turns with a `thinkingSignature` must be handled atomically.

**Recommendation:** Always align compact boundaries to complete turn units (user → assistant → [tool → tool_result]*).

#### 5. Model to use for compact

If the user's primary model is used for compact:

- High-cost models like Claude Opus or GPT-4o make compact expensive.
- Compact latency can spill into primary stream latency.

**Recommendation:** Allow configuring a separate model for compact (default: cheapest model from the same provider, or user-configurable via settings). For compact, speed and cost matter more than summarisation quality.

#### 6. Anthropic prompt cache breakpoint placement

To maximise cache hits on Anthropic, the `cache_control: { type: "ephemeral" }` breakpoint must be placed immediately after the compact summary message. The current `AnthropicService` does not handle this field.

**Recommendation:** Add a provider-specific metadata field to compact summary messages, and in `AnthropicService.convertToAnthropicMessages()` insert the `cache_control` breakpoint after that message.

---

### Revised Data Flow

```
Before each streamChat call:

[TOKEN MEASURE]
  │
  ├── < 75%   → proceed normally
  │
  ├── 75–99%  → start async compact (skip if already running)
  │             process current request using the existing stack
  │
  └── ≥ 100% → check compact completion
                ├── done    → use rebuilt stack
                └── pending → use sliding window fallback (non-blocking)


After compact completes, rebuild message stack:

  [system]                              ← always anchored
  [pinned_first_user]                   ← always anchored (existing structure preserved)
  [compacted_summary, compacted:true]   ← compact result; place cache breakpoint here
  [recent_messages...]                  ← messages not included in compact
```

---

### Context Length Settings UI

The proposed 8K → 256K fallback slider is reasonable, with two additions:

1. When model info is available, show **model-info-based automatic value** instead of the slider, with an override option
2. The current `getContextWindow()` is async (OpenRouter API call), so the UI needs loading state handling. On cache hit, display immediately; on cache miss, show the fallback first and update once the API call returns.

---

---

## Prompt Cache Analysis: Dynamic Elements vs. Stable Prefix

### Current System Prompt Assembly Order

`build_system_prompt()` in `src-tauri/src/agent/llm/prompt.rs`:

```
[1] Agent Identity & Strategy        ← agent_config.system_prompt
[2] Workspace Instructions           ← agents.md / CLAUDE.md / soul.md (file contents)
[3] Session Context                  ← session.metadata.name
[4] ContextRegistry providers        ← registry.build_context() result
    ├── TimeLocationContextProvider  (priority: 1000, last)
    └── (other providers, in priority order)
[5] Service Contexts                 ← proxy.get_service_contexts() result
    ├── bootstrap   (platform info)
    ├── planning    (current goal, todo list)
    ├── browser     (current URL, session)
    ├── mcp_manager (connected MCP server list)
    ├── assistant   (agent description)
    └── skills      (available skills)
```

### Volatility Map

| Section                                                  | Position   | Change frequency              | Cache perspective  |
| -------------------------------------------------------- | ---------- | ----------------------------- | ------------------ |
| [1] Agent Identity                                       | front      | on agent edit                 | ✅ Stable          |
| [2] Workspace Instructions                               | front      | on file change                | ✅ Stable          |
| [3] Session Context (name)                               | front      | once per session              | ✅ Stable          |
| [4] **TimeLocation** (priority 1000, **last**)           | **middle** | **every second** (HH:MM:SS)   | 🔴 **Kills cache** |
| [5] planning                                             | back       | every tool call               | 🔴 Volatile        |
| [5] browser                                              | back       | every navigation              | 🔴 Volatile        |
| [5] bootstrap                                            | back       | once per session              | ✅ Stable          |
| [5] mcp_manager                                          | back       | on MCP connection change      | 🟡 Semi-stable     |
| [5] skills                                               | back       | on skills directory change    | 🟡 Semi-stable     |

### Core Problem

**Anthropic prompt caching requires an explicit breakpoint at the "longest stable prefix."**

In the current structure, the stable sections are [1], [2], [3] — typically thousands to tens of thousands of tokens of agent identity and workspace instructions. However, `TimeLocationContextProvider` at [4] includes a **second-granularity timestamp**, causing the prefix to differ on every request.

```
Current effective cache boundary:
  [1][2][3]  ← stable (cacheable range)
       ↓
  [4] TimeLocation "Current Time: 14:23:07"  ← changes every second
                                               ← effective cache prefix boundary
  [5] planning / browser ...
```

Result: **All content including sections [1][2][3] is a cache miss on every request** — the Anthropic SDK currently caches nothing because there is no `cache_control` breakpoint. Even if a breakpoint were added, the stable prefix length would be near zero because [4] comes first.

### Required Structural Changes

To benefit from prompt caching, **stable → volatile ordering must be guaranteed** and a `cache_control` breakpoint placed at the end of the stable range:

```
# Target system prompt structure

[STATIC — cache_control breakpoint up to here]
  [1] Agent Identity
  [2] Workspace Instructions
  [3] Session Context (name)
  [4a] Semi-stable service contexts (bootstrap, mcp_manager, skills, assistant)
─── cache_control: { type: "ephemeral" } ───────────────
[VOLATILE — changes on every request]
  [4b] TimeLocation (time info)
  [5]  Planning (current goal, todos)
  [5]  Browser (current URL)
```

### Remediation Directions

#### 1. Remove seconds from TimeLocation

The fastest fix. Keeping `HH:MM` granularity reduces cache miss frequency to once per minute, but is not a complete solution.

```rust
// time_location.rs — current
let current_time = format!("{:02}:{:02}:{:02} {}", now.hour(), now.minute(), now.second(), ...);
// after fix (remove seconds)
let current_time = format!("{:02}:{:02} {}", now.hour(), now.minute(), ...);
```

#### 2. Move TimeLocation section to the **end** of the prompt

Use the `ContextProvider.priority()` system to place volatile providers at a high number (later order):

```
TimeLocationContextProvider::priority() → current: 5
                                          after:   1000 (last)
```

Alternatively, explicitly separate stable/volatile groups in `build_system_prompt()` to control ordering.

#### 3. Add `cache_control` breakpoint to `AnthropicService`

The current `AnthropicService` does not use the `cache_control` field at all. A breakpoint must be inserted at the end of the stable prefix (between sections [1]–[4a]).

```rust
// in convertToAnthropicMessages or doStreamChat
// split system prompt into stable / volatile blocks
// and insert cache_control at end of stable block
system: vec![
    TextBlockParam {
        text: stable_prefix,
        cache_control: Some(CacheControlEphemeral {}),  // ← breakpoint
    },
    TextBlockParam {
        text: volatile_suffix,
        cache_control: None,
    },
]
```

Anthropic can accept the system prompt as an `array of TextBlockParam` instead of a `string`, making this split possible.

### Implementation Priority

| Priority | Item                                                                                               | Rationale                                              |
| -------- | -------------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| P0       | Change `TimeLocationContextProvider` priority to 1000 → move to end of prompt                     | The root cause **completely invalidating** the cache   |
| P0       | Remove seconds from `TimeLocation` time format                                                     | Complements P0 above; effective standalone too         |
| P0       | `Message.compacted` flag + prevent re-compact                                                      | Prevents compact loop degradation                      |
| P0       | compact boundary turn-alignment (tool chain integrity)                                             | Prevents Anthropic/Gemini API 400 errors               |
| P1       | Add stable/volatile split + `cache_control` breakpoint to `AnthropicService`                      | Required for actual cache hits to occur                |
| P1       | Lower compact trigger to 75–80%                                                                    | Prevents 100% hard blocking                            |
| P1       | Use sliding window as 100% fallback (non-blocking)                                                 | Protects UX                                            |
| P2       | Classify semi-stable service contexts (bootstrap, mcp_manager) into the stable group              | Maximises cacheable range                              |
| P2       | Separate compact model configuration                                                               | Cost/latency optimisation                              |
| P3       | Context length settings UI (auto-detect from model + slider override)                             | User control                                           |
