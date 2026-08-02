# AgentChatMessages: Scroll glitch & input overlap caused by Footer sentinel never scrolled into view

## Summary

When new messages are appended, the chat list scrolls to the last **data item** but the **Footer component** (spacer + sentinel node) is never scrolled into view. The result:

1. **Input overlaps messages** — the last ~88px of content sits behind `AgentChatInput`.
2. **"Scroll to latest" FAB shows** even when the user is effectively at bottom.
3. **Visible jump/double-snap** when the DOM settles after async children (Markdown, ThinkingBubble) reflow.

---

## Root Cause

### 1. Dead code path — sentinel fallback never reached

`executeScrollToBottom` in `useAgentChatScroll.ts`:

```typescript
const scrolledWithVirtuoso = scrollVirtuosoToBottom(
  virtuosoRef.current,
  itemCount,
);

if (scrolledWithVirtuoso) {
  return true; // sentinel fallback NEVER reached
}

if (footerEndRef.current) {
  // dead code
  scrollFooterSentinelIntoView(footerEndRef.current);
  return true;
}
```

`scrollVirtuosoToBottom` calls `virtuoso.scrollToIndex({ index: "LAST", align: "end" })` which scrolls to the last **data item** only. The `Footer` component (spacer + sentinel) is rendered _after_ data items by Virtuoso, so it is never scrolled into view.

### 2. Threshold mismatch

| Constant                  | Value                | Purpose                                        |
| ------------------------- | -------------------- | ---------------------------------------------- |
| `VISUAL_BOTTOM_THRESHOLD` | `4px`                | `atBottomThreshold` + `isPinnedToBottom` check |
| Footer spacer height      | `88px` (64px + 24px) | `VirtuosoListComponents.tsx`                   |

`atBottomThreshold` is hardcoded to `4px`, but the Footer spacer is `88px`. When Virtuoso scrolls 88px short, `isPinnedToBottom(88, 4)` returns `false`, so the FAB incorrectly shows and the pinned latch is broken.

### 3. Double-snap on async content

`scrollVirtuosoToBottom` fires synchronously (inside `requestAnimationFrame`), then DOM reflows with actual content heights, then `totalListHeightChanged` triggers a **second** scroll. The two scrolls on consecutive frames cause a visible jump.

---

## Affected Files

| File                                                                                      | Role                                                     |
| ----------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| `src/features/agent/components/agent-chat-messages/hooks/useAgentChatScroll.ts`           | `executeScrollToBottom` — dead code path                 |
| `src/features/agent/components/agent-chat-messages/utils.tsx`                             | `scrollVirtuosoToBottom`, `scrollFooterSentinelIntoView` |
| `src/features/agent/components/agent-chat-messages/components/VirtuosoListComponents.tsx` | Footer spacer height                                     |
| `src/features/agent/components/agent-chat-messages/types.ts`                              | `VISUAL_BOTTOM_THRESHOLD`                                |
| `src/features/agent/AgentChatView.tsx`                                                    | `--agent-chat-composer-overlap` CSS variable             |

---

## Proposed Fix

### Fix 1 — Always scroll sentinel (critical)

```typescript
// executeScrollToBottom — replace early return
const scrolledWithVirtuoso = scrollVirtuosoToBottom(virtuosoRef.current, itemCount);

if (scrolledWithVirtuoso) {
  logScrollState("executeScrollToBottom:virtuoso-scroll", { ... });
}

// Always align to footer sentinel — Virtuoso does not account for Footer height
if (footerEndRef.current) {
  logScrollState("executeScrollToBottom:footer-sentinel-align", { ... });
  scrollFooterSentinelIntoView(footerEndRef.current);
  return true;
}

if (!scrolledWithVirtuoso) {
  logScrollState("executeScrollToBottom:unavailable", { ... });
  return false;
}

return true;
```

### Fix 2 — Unify spacer height constant

Extract and reuse a single constant:

```typescript
// types.ts
export const FOOTER_SPACER_HEIGHT = 88; // derived from 64 + 24

// getVisualBottomThreshold() -> return FOOTER_SPACER_HEIGHT + VISUAL_BOTTOM_THRESHOLD
// VirtuosoListComponents.tsx Footer spacer -> use FOOTER_SPACER_HEIGHT
// AgentChatMessages.tsx FAB bottom offset -> use FOOTER_SPACER_HEIGHT
```

### Fix 3 — Coalesce scroll calls

Add a `pendingScrollFrameRef` flag in `useAgentChatScroll` that prevents a second scroll within the same layout cycle:

```typescript
const pendingScrollFrameRef = useRef<number | null>(null);

const scheduleScrollToBottom = useCallback((reason: string) => {
  if (pendingScrollFrameRef.current !== null) {
    // Already scheduled this cycle — skip
    return;
  }
  pendingScrollFrameRef.current = requestAnimationFrame(() => {
    pendingScrollFrameRef.current = null;
    executeScrollToBottom(reason);
  });
}, [...]);
```

---

## Verification

1. Open a chat with a long response (Streaming + Markdown + Tool calls).
2. Observe the last message should be fully visible above the input — no overlap.
3. The "Scroll to latest" FAB should **not** appear when pinned to bottom.
4. No visible jump when content height changes mid-stream.
5. Run `pnpm refactor:validate` to ensure no regressions.
