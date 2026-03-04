# Weaver's Journal - The Pattern Log

## 2026-02-27 - [AgentChatStatusBar / SessionHistoryPanel / ToolCallCompactItem] **Eradicated:** [Action-Effect Chains & Syncing State] **Woven:** [Adjusting State During Render Pattern]

- **AgentChatStatusBar:** Removed two `useEffect` hooks used to synchronize `lastMetrics` when the session changed or new metrics arrived. Added a `prevSessionId` state and updated variables directly during render.
- **SessionHistoryPanel:** Eliminated the `useEffect` used to check if `selectedLineageId` still existed when `sessions` changed. Stored the previous `sessions` in a `useState` tracker and computed the reset logic directly during the render phase, gated on `selectedLineageId` being set to avoid unnecessary re-renders.
- **ToolCallCompactItem:** Removed the `useEffect` that triggered auto-expand on error/resource load. Moved the `setIsExpanded` call to the render phase (Adjusting State During Render) while retaining `useRef` for the transition-sentinel values `previousHasError`/`previousHasResource`, which are updated via a lean `useEffect` to avoid extra re-renders from state changes that don't affect output.
- **Renders Saved:** Removed cascading action-effect loops across key components; while the new `useState` trackers introduce an extra, intentional render when values change, they avoid the previous double-render cascades and keep updates local.

## 2026-02-27 - [AgentResourceAttachmentContext] **Eradicated:** [Defensive Coding/Redundant State Reset] **Woven:** [React Lifecycle Integrity]

- Removed `prevSessionIdRef`, `uploadedFilenamesRef`, and the associated `useEffect` that manually reset state on session ID change.
- The `AgentContainer` already uses `key={sessionId}` to force a remount of the provider, guaranteeing fresh state initialization naturally.
- **Renders Saved:** Eliminated potential double-renders from effect-based state resets.

## 2026-02-27 - [AgentMessageBubble] **Eradicated:** [Prop Drilling (Callback) & Logic in Render] **Woven:** [Configuration over Composition & Separation of Concerns]

- Extracted complex `displayContent` calculation to `computeDisplayContent` utility.
- Replaced `getAssistantName` callback prop with a simple `assistantName` string prop.
- **Benefits:** Component is now pure presentation; logic is testable in isolation.

## 2026-02-27 - [AgentChatInput] **Eradicated:** [God Component/Mixed Concerns] **Woven:** [Custom Hook Pattern]

- Extracted `handleSubmit`, input state, and file submission logic into `useChatSubmit` hook.
- **Benefits:** `AgentChatInput` now focuses solely on UI rendering; business logic is reusable and testable.

## 2026-03-02 - [AgentChatInput / Hooks] **Eradicated:** [Redundant Logic / Duplication] **Woven:** [DRY Pattern / Centralized Data Fetching Hook]

- Removed the redundant `useSessionTools` hook, which duplicated the functionality of `useAgentTools`.
- Updated `AgentChatInput` to use the more robust, centralized `useAgentTools` hook, providing validation, loading state, and error handling out of the box.

## 2026-03-03 - [PlaybookList] **Eradicated:** [God Component / Prop Hoarder] **Woven:** [Custom Hook Pattern / Unified State]

- Extracted complex data fetching, filtering, grouping, and deletion logic into a new custom hook `usePlaybooks`.
- Replaced 8 individual props passed to `<SortControls />` with a unified `PlaybookSortState` object.
- **Benefits:** `PlaybookList` is now a lean presentation component, and `SortControls` has a significantly reduced prop surface area.
