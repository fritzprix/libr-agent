# Text Loop Recovery

## Problem

During LLM streaming, the assistant can get stuck repeating the same **text**
content (no tool calls). The existing recovery stack does not catch this:

| Layer                                       | What it detects                             |
| ------------------------------------------- | ------------------------------------------- |
| `repeatedThinkingDetector` (Frontend)       | Repeated **thinking** tail during streaming |
| `handle_thinking_only_completion` (Backend) | Completed turn with thinking only, no text  |
| Circuit Breaker (Backend)                   | Repeated **tool call** signatures           |

A text-only repetition loop completes as a normal assistant message (`has_content
== true`) and is persisted. Users see runaway narrative with no automatic
recovery.

## Decision

Extend the existing streaming-issue pipeline (Frontend detect → IPC report →
Rust `CancelAndRetry`) with a separate **text loop** kind. Keep the same
overlap-based tail algorithm; tune thresholds and guards for text.

```
Frontend (useExecuteCompletion)
  detectRepeatedTextLoop(currentStreamingText)
       ↓
  reportLLMStreamingIssue({ issueKind: REPEATED_TEXT_LOOP })
       ↓
Backend (handle_streaming_issue)
  emit_completion_cancel(reason: "repeated-text-loop")
       ↓
  request_llm_completion_with_recovery (max 2 retries)
```

## Thresholds

| Parameter          | Thinking | Text  | Rationale                                                        |
| ------------------ | -------- | ----- | ---------------------------------------------------------------- |
| `minRepetitions`   | 2        | **3** | Text has more natural surface repetition (lists, JSON, markdown) |
| `minPatternLength` | 64       | 64    | Short patterns are noise                                         |
| `tailChars`        | 1024     | 1024  | Same sliding window as thinking                                  |

## Retry counter policy

**Separate counters** (not shared with thinking):

- `repeated_thinking_retry_count` — thinking loops + thinking-only completions
- `repeated_text_loop_retry_count` — text loops only

Each has an independent budget of **2** retries per workflow turn. Counters
reset on workflow start and after a productive assistant completion.

Sharing one counter caused a real edge case: thinking retry + text retry could
exhaust the budget before a subsequent normal turn.

## Detection guards (Frontend)

Text loop detection runs only when:

1. A **content** chunk is being processed (not inside the thinking block handler)
2. **No tool call** has been seen in the current stream (`hasToolCallInStream`)

Rationale:

- Thinking chunks use the existing thinking detector.
- After tool calls start, cancel cost is high; Circuit Breaker handles tool
  repetition.

## Out of scope (follow-up PR)

**Post-stream safety net** in `response.rs` (detect repeated text after
completion, like `handle_thinking_only_completion`). Deferred because it
requires duplicating the tail algorithm in Rust or sharing via FFI. Streaming
detection is the primary fix.

## Error surface

| Field                | Value                |
| -------------------- | -------------------- |
| `StreamingIssueKind` | `REPEATED_TEXT_LOOP` |
| Error code           | `REPEATED_TEXT_LOOP` |
| Cancel reason        | `repeated-text-loop` |

## Recovery chain guarantees

**Primary goal:** stop an infinite thinking/text loop within the retry budget.

**Secondary goal:** after the budget is exhausted, terminate the workflow with a
`REPEATED_*_LOOP` error via `finalize_workflow_error_with_dispatcher`.

These goals are met on the **normal path**: each detected loop increments the
counter only after a successful cancel, and the third detection (with
`retry_count == max_retries`) triggers hard fallback.

### `emit_completion_cancel` failure — intentional skip

If `emit_completion_cancel` fails, `handle_streaming_issue` restores
`expected_response_id`, returns `Err`, and does **not**:

- increment the retry counter
- call `request_llm_completion_with_recovery`

Rationale:

1. **No dual completion** — starting a retry while the frontend stream is still
   active would run two completions for the same session.
2. **No false retry credit** — incrementing the counter without a successful
   cancel would count a retry that never stopped the loop.
3. **Re-detection** — the frontend may report again on later chunks; the
   counter is unchanged, so the next successful cancel+retry path is unchanged.

This is conservative by design. Hard fallback is **not** required in this case;
the loop may continue until a later report succeeds or the user aborts.

### Other non-goals

| Outcome                                        | Behavior         | Why                                          |
| ---------------------------------------------- | ---------------- | -------------------------------------------- |
| Stale report (`expected_response_id` mismatch) | `Ignored`        | A newer request owns the session             |
| Session not found                              | `Ignored`        | Nothing to recover                           |
| Report IPC failure (frontend)                  | Stream continues | See follow-up: local abort on report failure |

## Test plan

- Unit: `repeatedTailDetector.test.ts` — text threshold (3×), list/JSON false
  negatives where possible, thinking preset unchanged
- Integration: `repeated_thinking_recovery_tests.rs` — text loop action
  evaluation with separate counter semantics
- Manual: provoke text-only repetition in a session without tools; confirm
  cancel + retry up to 2 times, then workflow error

## References

- `src/context/llm/repeatedTailDetector.ts`
- `src-tauri/src/agent/llm/stream_recovery.rs`
- `docs/architecture/session-cancel-isolation.md` (cancel event flow)
