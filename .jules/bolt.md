## 2024-05-22 - React.memo with Unstable Object Props

**Learning:** `SessionList` was re-rendering all items because `sessionsWithHits` recreated all session objects even when hits were zero. Implementing a custom comparator in the child component (`SessionItem`) was rejected as brittle and unsafe (stale closures).
**Action:** The correct fix is to ensure the parent component (`History`) preserves the original object references when the data hasn't effectively changed (e.g. using the original `session` object when `searchHits` is 0/undefined), allowing standard `React.memo` to work correctly without custom comparators.

## 2024-05-23 - Unstable Hook Callbacks

**Learning:** `useThrottle` was returning a new function reference on every render because it depended on the input `callback`, which is often an unstable inline arrow function. This caused consuming effects (like scroll listeners) to constantly detach and re-attach, wasting resources.
**Action:** Use the `useRef` pattern (often called `useLatest`) inside custom hooks to store callbacks. This allows the returned function to depend only on stable values (like `delay`) while still executing the latest callback version.

## 2024-05-24 - Hooks Returning Auxiliary Data

**Learning:** `AgentChatMessages` was performing a redundant O(N) iteration to build `toolResultsMap` on every render (token update), even though `useMessageGrouping` was already iterating the same list (O(N)).
**Action:** List-processing hooks should return derived lookups (like maps) alongside the main result when consumers need them. This avoids redundant passes over the data and simplifies the consumer code.

## 2025-02-19 - Redundant Map Creation in Render Loop

**Learning:** `AgentChatMessages` was creating a new `Map` for `toolResults` inside the render loop for every tool group message, even though `useMessageGrouping` already provided a global map. This caused unnecessary object allocations and potentially triggered re-renders in children if they relied on prop identity.
**Action:** Utilize the global lookup map provided by the data processing hook (`useMessageGrouping`) instead of reconstructing partial maps in the render loop. Pass the global map to child components, allowing them to look up what they need.

## 2025-05-21 - Unstable Hook Return Values

**Learning:** `useRustBackend` was returning a new object literal on every render, causing consumers like `AgentMessageRenderer` (which renders frequently during streaming) to recreate callbacks (`handleUIAction`) and unnecessarily re-render heavy children (`UIResourceRenderer` with iframes).
**Action:** When a hook returns an object composed entirely of static or stable values (like imported module functions), define the object constant outside the hook to ensure referential stability and prevent cascading re-renders.

## 2025-06-02 - Stabilizing Derived Maps in Hooks

**Learning:** The `useMessageGrouping` hook was creating a new `toolResultsMap` instance on every execution, breaking `React.memo` optimization in `AgentMessageBubble` even when the map content was identical.
**Action:** Implement shallow equality checks for derived Maps/Sets in hooks and reuse the previous instance if content is unchanged, ensuring referential stability for consumers.

## 2025-06-25 - Premature Backend Refetch on Streaming

**Learning:** `AgentChatMessages` was triggering session file refetches on every streaming chunk of an assistant message if it contained tool calls, causing N+1 backend requests. However, files are only created after tool _execution_, not during the _call_.
**Action:** Only trigger refetches when a `tool` role message (the result) appears. Use `useThrottle` to debounce these refetches when multiple tool results arrive in rapid succession (e.g. parallel tool execution).

## 2025-06-25 - Layout Thrashing in Markdown Rendering

**Learning:** `AgentMessageRenderer` was calling `window.matchMedia` inside the render function of every `<code>` block to check for dark mode. For long messages with many code blocks, this caused excessive DOM queries and layout invalidation risks.
**Action:** Move global environment checks (like dark mode) to a custom hook (`useIsDarkMode`) at the parent level and inject the value into memoized components via props, preventing repeated DOM queries.

## 2026-02-05 - Lazy Map Cloning and Group Reuse

**Learning:** `useMessageGrouping` was performing O(N) map cloning and scanning on every streaming token update, even when the new content (text) didn't add any new tool results. It also unnecessarily re-evaluated stable "single" message groups ending at the divergence point.
**Action:** Implement "lazy cloning" for derived maps: track the index of the last contributing item (`lastToolResultIndex`) and reuse the previous map reference if the update is strictly after that index. Also, allow reusing "single" message groups ending at the divergence point since they can't consume subsequent messages.

## 2026-02-06 - ReactMarkdown Unstable Props

**Learning:** `ReactMarkdown` re-renders custom components like `code` on every text update (e.g., streaming) by passing a new `children` array reference, even if the content is identical. This breaks standard `React.memo` (shallow comparison) for heavy components like syntax-highlighted code blocks.
**Action:** Implement a custom comparison function for `React.memo` on components used in `ReactMarkdown` that compares `String(prevProps.children) === String(nextProps.children)` to ensure stability during updates.

## 2026-02-07 - Deep Comparison for Unstable Prop References

**Learning:** `AgentMessageRenderer` maps `MCPToolCallContent` to new `ToolCall` objects on every render, creating new references even when data is identical. This breaks `React.memo` on `AgentToolCallGroup` which relied on reference equality check for array items, causing unnecessary re-renders during high-frequency streaming updates.
**Action:** Update the custom `arePropsEqual` comparator in the child component (`AgentToolCallGroup`) to perform a deep equality check on the properties of the objects (`id`, `type`, `function`) instead of relying on reference equality for array items.

## 2026-02-12 - Missing Database Indexes on Frequently Queried Foreign Keys

**Learning:** Queries filtering by `session_id` in `messages`, `contents`, and `stores` tables were performing full table scans because SQLite (and many other DBs) does not automatically index foreign keys. This caused performance degradation in chat history loading and content management as the dataset grew.
**Action:** Always verify database indexes for columns used in `WHERE`, `JOIN`, and `ORDER BY` clauses, especially foreign keys. Add specific migrations to create these indexes if the ORM or database doesn't do it automatically.

## 2026-02-13 - Frequent Message Cloning in Hot Path

**Learning:** `handle_llm_response` clones the entire `Message` struct 3 times (cache, event, DB) before processing. This causes performance overhead when the message contains large payloads like tool arguments (file content).
**Action:** Use `Option::take()` to extract heavy fields (like `tool_calls`) from the struct instance after the required clones are made for cache/event/DB, but before spawning async work, to avoid one additional deep clone in the hot path. Note that this pattern does not prevent the earlier clones from carrying the heavy data; truly minimizing cloning would require extracting the heavy field before cloning and explicitly reconstructing lighter variants for the clones that do not need it. Move ownership into async blocks instead of cloning whenever possible.

## 2026-10-24 - O(N^2) Tree Traversal in Render Loop

**Learning:** `SessionHistoryPanel` was calculating descendant counts using a recursive function that filtered the entire `sessions` array at each step, resulting in O(N^2) complexity. This caused potential lag as the session history grew.
**Action:** When processing tree structures from flat lists, always build an O(N) adjacency map (parent -> children) first, then use that map for O(1) lookups during traversal, reducing overall complexity to O(N).

## 2025-06-25 - Expensive Date/Number Formatting in Render Loops

**Learning:** Calling `Number.toLocaleString()` (or similarly `Date.toLocaleDateString()`) repeatedly in React render loops or frequently called utility functions (like `formatUsageMetrics` called during streaming) causes significant performance bottlenecks because it instantiates a new `Intl.NumberFormat` instance internally on every call.
**Action:** Cache and reuse module-level `Intl.NumberFormat` (or `Intl.DateTimeFormat`) instances using a singleton or factory function to avoid repeated object allocations and layout/rendering stutter.

## 2026-04-02 - Redundant Sorting and In-Place Mutation of Derived Arrays

**Learning:** `AppSidebar` was calling `.sort()` on arrays returned from a map (`childrenByParent`) during every render. Because the map was built from an already-sorted array, the child arrays naturally maintained their sort order. Calling `.sort()` was not only a redundant O(K log K) computation but also mutated the map's arrays in-place during the React render phase, which violates React's pure render rules.
**Action:** When building adjacency lists or parent-child maps from initially sorted arrays, rely on the preserved sorting order. Never call `.sort()` on derived arrays within a render loop as it causes both unnecessary computation and in-place mutation.

## 2026-10-25 - Safe localized sorting of Adjacency Maps

**Learning:** `SessionHistoryPanel` needed to sort its `childrenByParent` derived map, but could not pre-sort `baseSessions` because the sorting logic itself (`sortByCurrentOrder`) depended on descendant counts calculated _after_ the map was built. Doing `.sort()` inside the `walk` function during render caused repeated O(K log K) sorting.
**Action:** When an adjacency map cannot be built from a pre-sorted array due to circular sorting dependencies, perform a one-time in-place sort of the map's values immediately after its construction inside the `useMemo` block (e.g., `for (const children of childrenByParent.values()) children.sort(...)`). This keeps the sort out of the recursive `walk` traversal and guarantees each child array is sorted only once per `useMemo` recomputation.

## 2024-05-26 - Incomplete Formatter Consolidation

**Learning:** Reusable formatter caches are often re-implemented in multiple component files instead of a centralized utility file, fragmenting the cache and increasing overall memory footprint.
**Action:** When centralizing `Intl` formats in a `utils` file, ensure you support `locale` variations by storing instances in a `Map` within the central utility function, so that various localized formats can be safely cached and reused without recreating localized formatters locally in components.

## 2026-10-26 - Redundant O(N log N) Sorting on Derived Arrays in Render Loop

**Learning:** `ScheduledTasksPage` was creating intermediate arrays using `.filter(...)` and then sorting them using `.sort(...)` repeatedly in the render loop just to calculate `nextGroupRun` (the earliest next run time among enabled tasks). Since the source array `group.tasks` was already correctly sorted upstream (enabled first, then by `nextRunAt`), this eager downstream sorting was entirely redundant and caused unnecessary array allocations and O(N log N) computation per group render.
**Action:** When extracting a single item (like a min/max or "next" item) from an already-sorted array, do not re-sort. Use an O(1) traversal like `.find(...)` to locate the first item matching your criteria. Similarly, avoid intermediate arrays from `.filter().length` and prefer `.reduce()` when just counting specific items in a hot path.

## 2026-04-18 - Memoizing Derived Array Metrics

**Learning:** Extracting inline `.reduce()` calls that count matching items (like enabled tasks or bookmarked sessions) into `useMemo` hooks or integrating them into existing `useMemo` blocks prevents O(N) recalculations on every component render.
**Action:** When computing scalar counts or metrics from arrays, calculate and store them in `useMemo` or integrate the logic directly into existing list mappings to ensure referential stability and eliminate redundant layout thrashing.

## 2026-10-27 - Bypassing React.memo with Sets

**Learning:** `ScheduledTaskRow` was rendering unnecessarily on every toggling or deletion. The parent component (`ScheduledTasksPage`) was passing `togglingIds` and `deletingIds` (which are `Set`s) as props to the row components. Since these sets were frequently recreated or mutated by the `useScheduledTasks` hook, `React.memo`'s shallow comparison failed, causing an O(N) re-render of all task rows whenever a single task was modified.
**Action:** When using `React.memo` to optimize child list components, avoid passing complex collection objects (like Sets or Maps) as props. Compute and pass primitive values (e.g., `isToggling={togglingIds.has(task.id)}`) directly within the parent's mapping loop. Also ensure all passed callbacks (`onEdit`, `onToggle`, `onDelete`) are referentially stable using `useCallback`.
