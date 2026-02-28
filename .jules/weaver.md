# Weaver's Journal - The Pattern Log

## 2026-02-27 - [AgentChatStatusBar / SessionHistoryPanel / ToolCallCompactItem] **Eradicated:** [Action-Effect Chains & Syncing State] **Woven:** [Adjusting State During Render Pattern]

- **AgentChatStatusBar:** Removed two `useEffect` hooks used to synchronize `lastMetrics` when the session changed or new metrics arrived. Added a `prevSessionId` state and updated variables directly during render.
- **SessionHistoryPanel:** Eliminated the `useEffect` used to check if `selectedLineageId` still existed when `sessions` changed. Stored the previous `sessions` using a `useRef` and computed the reset logic directly during the render phase.
- **ToolCallCompactItem:** Removed `useEffect` for auto-expanding tools on error/resource load. Checked visibility changes during the render phase against previous refs and updated state naturally without relying on the effect lifecycle.
- **Renders Saved:** Eliminated cascading double-renders and action-effect cycles across multiple key components.

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
