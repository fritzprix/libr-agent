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

## 2025-02-19 - Redundant Map Creation in Render Loop

**Learning:** `AgentChatMessages` was creating a new `Map` for `toolResults` inside the render loop for every tool group message, even though `useMessageGrouping` already provided a global map. This caused unnecessary object allocations and potentially triggered re-renders in children if they relied on prop identity.
**Action:** Utilize the global lookup map provided by the data processing hook (`useMessageGrouping`) instead of reconstructing partial maps in the render loop. Pass the global map to child components, allowing them to look up what they need.

## 2025-05-21 - Unstable Hook Return Values

**Learning:** `useRustBackend` was returning a new object literal on every render, causing consumers like `AgentMessageRenderer` (which renders frequently during streaming) to recreate callbacks (`handleUIAction`) and unnecessarily re-render heavy children (`UIResourceRenderer` with iframes).
**Action:** When a hook returns an object composed entirely of static or stable values (like imported module functions), define the object constant outside the hook to ensure referential stability and prevent cascading re-renders.
