# Tool Loop Recovery

## Problem

During agent workflows, the model can repeat the same tool call with identical
arguments and outcomes (for example snapshot polling with `checkSession(wait=false)`
or `waitForProcess(timeout=0)`). Unchecked loops waste context, latency, and user
trust.

The backend circuit breaker detects trailing identical `(tool, args, outcome)`
signatures and intervenes before runaway execution.

## Policy (default)

Configured under **Settings → Experimental → Tool-loop recovery**.

| Setting                         | Default | Behavior                                                                                                                                             |
| ------------------------------- | ------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `toolLoopLegacyGuidanceEnabled` | `false` | **Clean resample** — discard the looping assistant turn and request a fresh LLM completion without injecting loop-prevention text into tool results. |
| `toolLoopMaxResampleRetries`    | `2`     | Resample attempts allowed while count stays in `[threshold, hard_break)`.                                                                            |
| (legacy ON)                     | —       | Restores previous behavior: loop-prevention guidance is injected as tool-error text.                                                                 |

Advanced loop thresholds live under **Settings → Advanced**:

- `loopPreventionThreshold` (default **3**)
- `loopPreventionHardBreakOffset` (default **2**) → hard break at count **5**

## Detection layers

| Layer                    | Scope                 | Notes                                                                                    |
| ------------------------ | --------------------- | ---------------------------------------------------------------------------------------- |
| Per-tool outcome streak  | Single `(tool, args)` | Outcome text changes reset the streak (legitimate polling with progress).                |
| Intra-batch duplicate    | Same turn             | Duplicate `(tool, args)` in one assistant batch → immediate short-circuit (no resample). |
| Batch fingerprint streak | Multi-tool batch      | Same ordered batch repeated across turns; escalate step before hard break.               |

Exempt tools: `ui__circuitBreak`, `planning__reflect`.

## Resample path

When resample is enabled and the repeat count is within budget:

1. `preprocess_assistant_tool_calls()` sets `tool_loop_resample`.
2. `response.rs` persists prompt-token checkpoint and resets streaming recovery counters.
3. The assistant message is **not** cached, emitted, or persisted.
4. `request_llm_completion_with_recovery()` runs again (pending user messages are claimed at completion start).

**Session-scoped resample budget:** `AgentSession.tool_loop_resample_attempts`
tracks retries keyed by loop fingerprint (single-tool signature or batch
fingerprint). Because resample discards the assistant turn, DB history length
does not advance — the session budget prevents infinite resample at a fixed
history count.

When the budget is exhausted or hard-break threshold is reached, the workflow
promotes to `ui__circuitBreak` (or text-only fallback if UI builtin is disabled).

## Polling tools

Tools such as `agent__checkSession` and `workspace__waitForProcess` are not
exempt. Progressive outcomes (status/turn changes) reset streaks. Identical
snapshot polling is intentionally discouraged — prefer blocking wait
(`wait=true` or non-zero timeout).

### Structured loop fingerprints

Tool results may include `metadata.structuredContent.loopFingerprint` (or
canonical fields like `status`, `turnCount`, `responseStatus`, `process_id`).
The circuit breaker prefers these over raw text when building outcome
signatures, so legitimate polling progress resets streaks even when human-readable
text stays similar.

### In-band PollTracker

Both polling tools use the shared `PollTracker` helper:

| Tool                        | Tracker location                  | Key                  |
| --------------------------- | --------------------------------- | -------------------- |
| `workspace__waitForProcess` | `ProcessEntry.poll_tracker`       | process id           |
| `agent__checkSession`       | `AgentSession.tool_poll_trackers` | `{tool}:{sessionId}` |

When consecutive identical snapshots exceed `LIBRAGENT_POLL_THRESHOLD` (default 5) in snapshot mode (`timeout=0` / `wait=false`), the tool returns in-band
guidance before the global circuit breaker threshold (3).

Tools declare this contract via the `x-libragent-wait` schema extension on
`MCPTool` (see [builtin_tool_bp.md](../guides/builtin_tool_bp.md#24-wait-capable-tool-contract-x-libragent-wait)).

## Related docs

- [Text Loop Recovery](./text-loop-recovery.md) — streaming text repetition (separate pipeline)
- [Soul Lounge Recovery Loop (Experimental)](./soul-lounge-recovery-loop.md) — server-driven recovery pacing
