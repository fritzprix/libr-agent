## 2024-05-22 - React.memo with Unstable Object Props

**Learning:** `SessionList` was re-rendering all items because `sessionsWithHits` recreated all session objects even when hits were zero. Implementing a custom comparator in the child component (`SessionItem`) was rejected as brittle and unsafe (stale closures).
**Action:** The correct fix is to ensure the parent component (`History`) preserves the original object references when the data hasn't effectively changed (e.g. using the original `session` object when `searchHits` is 0/undefined), allowing standard `React.memo` to work correctly without custom comparators.

## 2024-05-23 - Unstable Hook Callbacks

**Learning:** `useThrottle` was returning a new function reference on every render because it depended on the input `callback`, which is often an unstable inline arrow function. This caused consuming effects (like scroll listeners) to constantly detach and re-attach, wasting resources.
**Action:** Use the `useRef` pattern (often called `useLatest`) inside custom hooks to store callbacks. This allows the returned function to depend only on stable values (like `delay`) while still executing the latest callback version.

## 2024-05-24 - Hooks Returning Auxiliary Data

**Learning:** `AgentChatMessages` was performing a redundant O(N) iteration to build `toolResultsMap` on every render (token update), even though `useMessageGrouping` was already iterating the same list (O(N)).
**Action:** List-processing hooks should return derived lookups (like maps) alongside the main result when consumers need them. This avoids redundant passes over the data and simplifies the consumer code.

## 2024-05-25 - Expensive Prop Recreation in Render Loop

**Learning:** `AgentMessageRenderer` was creating new object references for `htmlProps` and spreading `supportedContentTypes` (creating a new array) on every render. This caused `UIResourceRenderer` (which likely contains iframes) to re-render unnecessarily, potentially causing flicker or performance degradation during streaming.
**Action:** Always memoize complex objects or arrays passed as props to expensive child components (like those rendering iframes or heavy UI), especially within a component that renders frequently (like a chat message renderer).

## 2026-01-26 - Stable Message Grouping

**Learning:** `useMessageGrouping` was recreating all `GroupedMessage` objects (and their internal arrays) on every render (even for historical messages) because it rebuilt the array from scratch. This caused `AgentMessageBubble` (and consequently `AgentMessageRenderer`) to re-render the entire chat history on every token stream, defeating `React.memo`.
**Action:** Implemented a `useRef`-based cache in `useMessageGrouping` to reuse `GroupedMessage` objects when the input messages for that group haven't changed reference. Also optimized `AgentMessageBubble` to accept a stable `toolResults` array instead of creating a new `Map` in the parent render loop.
