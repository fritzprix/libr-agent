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

## 2026-03-05 - [Scheduled Tasks Components] **Eradicated:** [Derived State and Action-Effect Chains] **Woven:** [Component Composition and Declarative State]

- Collapsed multiple scheduled-task state mirrors into a single source of truth owned by a parent coordinator component.
- Replaced imperative “update-and-sync” effects with declarative props and callbacks wired through small, focused child components.
- **Benefits:** Eliminated redundant derived state and action-effect loops, making scheduled task flows easier to reason about and less prone to subtle desync bugs.

## 2025-02-17 - [Modal] **Eradicated:** [Prop Hoarder / Monolithic component wrapper] **Woven:** [Compound Components Pattern]

## 2024-05-19 - useMCPServerForm hook **Eradicated:** Derived State (URL parsing in useEffect on mount) **Woven:** Direct State Initialization

## 2024-05-19 - useSettingsForm hook **Eradicated:** Derived State (Syncing globalSettings to formState in useEffect) **Woven:** Adjusting State During Render

## 2024-05-19 - InputTokenDropdown component **Eradicated:** Derived State (Resetting activeIndex via useEffect on mode change) **Woven:** Adjusting State During Render

## 2024-05-19 - ScheduledTaskModal component **Eradicated:** Derived State (Setting default assistantId via useEffect on load) **Woven:** Adjusting State During Render

## 2026-03-05 - [SessionFilesPopover / usePlaybookSearch / useWorkspaceFiles] **Eradicated:** [Derived State & Effect Syncing] **Woven:** [Adjusting State During Render & React Key Pattern]

- **SessionFilesPopover:** Removed the `useEffect` that manually reset local state (`setIsOpen(false)`, `setSelectedFile(null)`, etc.) when `sessionId` changed. Replaced this effect-based reset with a declarative **React Key Pattern** (`<SessionFilesPopover key={session.id} />`) in `AgentChatHeader`, forcing natural unmount/remount on session change.
- **usePlaybookSearch:** Eradicated the anti-pattern of copying props to state and using an effect to clear the `playbooks` array on `query === null`. Refactored to compute directly by returning an empty array dynamically during the render cycle if `query` is null, preventing flashes of stale data.
- **useWorkspaceFiles:** Aligned comments and inline documentation with the current behavior; no structural refactor was required for this hook.
- **Renders Saved:** Eliminated the "State Duplicator" anti-pattern and the accompanying double-renders on prop changes, restoring declarative rendering integrity.

## 2026-03-05 - [ServerToolsModal / BuiltInToolsEditor] **Eradicated:** [God Component / Logic in Render] **Woven:** [Custom Hook Pattern]

- **ServerToolsModal:** Extracted the data fetching logic for server tools (`probe_mcp_server`) into a `useServerTools` hook. The modal component now strictly focuses on presentation and state consumption.
- **BuiltInToolsEditor:** Extracted the fetching of builtin server definitions into a `useBuiltinTools` hook, eliminating the component's internal `useEffect` orchestration.
- **Benefits:** Decoupled data fetching from UI presentation, significantly improving testability and adhering to the modernized Container/Presentational pattern.

## 2026-03-05 - [HttpForm / EnvVarsForm] **Eradicated:** [Action-Effect Chains] **Woven:** [Callback Ref Pattern / Event-Driven State]

- **HttpForm & EnvVarsForm:** Removed the `useEffect` block that monitored `.length` changes on `customHeaders` and `envVars` to imperatively focus the newly added input via a `prevLengthRef`.
- **Woven:** Applied the React **Callback Ref** pattern conditionally governed by an `isAddingRef` flag set within the `onClick` handler. When a new input mounts after clicking "Add", its `ref` callback directly calls `.focus()`.
- **Renders Saved:** Eliminated a redundant reactive cycle (the action-effect chain), bypassing the extra reconciliation required by `useEffect`-driven focus synchronization.

## 2026-03-11 - [EditorContext] **Eradicated:** [Effect State Sync (useEffect setting state based on props)] **Woven:** [Render-Phase Mutation Pattern]

- Removed the `useEffect` that mirrored incoming editor configuration/props into local state.
- Now derives editor state directly during the render phase, mutating refs and local variables instead of bouncing through an effect-based sync path.
- **Renders Saved:** Avoids the extra render caused by effect-driven state syncing and eliminates transient "double-apply" flashes.

## 2026-03-11 - [FileAttachment] **Eradicated:** [Array index as key in dynamic lists] **Woven:** [Unique Composite Key Pattern]

- Replaced index-based keys for attachment rows with a stable composite key built from intrinsic file identity (e.g., name/size/lastModified or a generated ID).
- Ensures list item identity remains stable across insertions, deletions, and reordering, preventing focus loss and flicker in the attachment UI.
- **Benefits:** More predictable reconciliation semantics and safer future refactors around drag-and-drop and progressive uploads.

## 2026-03-11 - [AgentChatStatusBar] **Superseded by 2026-03-14 entry**

- This intermediate note described an earlier render-phase-mutation experiment.
- The current implementation was later refined to the session-scoped snapshot-state pattern documented in the 2026-03-14 entry below.
- Keep the newer entry as the source of truth when reasoning about the live code.

## 2026-03-12 - [AssistantList / SkillsEditor] **Eradicated:** [God useEffect blocks, mixed business logic and presentation, derived state] **Woven:** [Custom Hook Pattern (useAssistantsList, useAssistantSkills)]

## 2026-03-13 - [useIsMobile hook] **Eradicated:** [Event-Driven State using useState & useEffect] **Woven:** [useSyncExternalStore Pattern]

- Replaced `useState` and `useEffect` with `React.useSyncExternalStore` in the `useIsMobile` hook.
- **Benefits:** Adheres to React 18+ conventions for subscribing to external stores/events natively, improving performance and reducing reliance on cascading effects for state management.

## 2026-03-14 - [AppSidebar / History] **Eradicated:** [Effect State Sync / Derived State] **Woven:** [Context Initialization Pattern]

- Removed the `useEffect` blocks that called `loadSessions()` on mount in both `AppSidebar` and `History` components.
- **Benefits:** The `AgentSessionListProvider` already initializes and handles `loadSessions`. Removing redundant loads from children components prevents unnecessary API calls and extra renders.

## 2026-03-14 - [ScheduledTasksPage / ScheduledTaskModal] **Eradicated:** [God Component / Action-Effect Chains / Derived State] **Woven:** [Custom Hook Pattern / Component Composition / Adjusting State During Render]

- **ScheduledTasksPage:** Extracted all data fetching, state management, and mutation logic into a new custom hook `useScheduledTasks`. Eradicated "God Component" behavior.
- **ScheduledTaskModal:** Removed the internal `useEffect` for data fetching (`listAssistants`) and passed `assistants` as a prop directly. Eradicated the action-effect chain `if (assistants !== prevAssistants)` by deriving the effective assistant selection directly during rendering.
- **Benefits:** Clean separation of concerns, complete eradication of prop copying and effect-based state syncing loops.

## 2026-03-14 - [AgentChatStatusBar] **Eradicated:** [Prop-Mirroring Reset Effect] **Woven:** [Session-Scoped Snapshot State]

- **AgentChatStatusBar:** Removed the prop-mirroring reset effect for `lastMetrics` and replaced it with session-scoped snapshot state that is explicitly reset when the session changes.
- **Benefits:** Keeps persisted metrics aligned with the active session without relying on render-phase state synchronization.

## 2026-03-14 - [GeneralTab / useSkillsDirectory] **Eradicated:** [God Component / Logic in Render] **Woven:** [Custom Hook Pattern]

- **GeneralTab:** Extracted the directory verification logic (`scan_skills_directory` and fallback to default dir) into a new `useSkillsDirectory` hook.
- **Benefits:** Decoupled business logic (verification of skills folder) from the presentation, removing the monolithic `useEffect` hook block inside `GeneralTabComponent`.

## 2026-03-24 - Data Fetching Hooks (useAgentTools, useServerTools, useBuiltinTools, usePlaybookSearch)

**Eradicated:** Imperative data fetching using `useState` and `useEffect` chains, including manual error and loading state management.
**Woven:** Declarative Data Fetching with `useSWR`, extracting side-effects into `onSuccess` and `onError` configuration options, and preventing unnecessary re-renders.

## 2026-03-24 - [ToolCallCompactItem] **Eradicated:** [Reading/writing refs during render phase] **Woven:** [Adjusting State During Render via useState]

- Removed `useRef` based tracking of previous values during the render phase.
- Used `useState` to track previous values for transitions, updating state during render and preserving the pure render rule.
- **Renders Saved:** Eliminated potential unpredictable render cycle side-effects while safely updating state during component evaluation.

## 2026-04-04 - [GeneralTab & AgentPlanningUpdates] **Eradicated:** [Cascading State Effect & Prop Hoarding] **Woven:** [Derived State & Hook Pattern]

- **useSkillsDirectory:** Eradicated the anti-pattern where a `useEffect` forcibly synced a default directory back to the parent component on mount without user interaction.
- **Woven (useSkillsDirectory):** Implemented derived state to compute and return an `effectiveDir` during render, which the parent (`GeneralTab`) uses for presentation and verification.
- **AgentPlanningUpdates:** Eradicated the "Prop Hoarder" anti-pattern in `PlanningToastSummary` by removing 7 localized string props passed down from the parent.
- **Woven (AgentPlanningUpdates):** Implemented the Custom Hook Pattern by calling `useTranslation()` directly inside `PlanningToastSummary`, allowing it to manage its own localization dependencies.

## 2026-04-09 - [AgentPlanningUpdates] **Eradicated:** [God useEffect block / Unrelated Event Subscription] **Woven:** [Custom Hook Pattern (useAgentMessageTrigger)]

- **AgentPlanningUpdates:** Extracted the complex Tauri event listener ('agent:event') and debouncing logic from a monolithic `useEffect` into the `useAgentMessageTrigger` custom hook.
- Added strict filtering via `messageFilter` to trigger context updates only for successful tool-result messages by checking `message.role === 'tool'`, `message.tool_call_id`, `!message.error`, and `message.metadata?.toolError !== true`.
- **Benefits:** Decoupled event subscription from component rendering logic, standardized event handling across the chat interface, and avoided refreshes from failed tool executions.

## 2026-04-10 - [SkillsEditor]

**Learning:** Extracting complex Drag-and-Drop subscription logic out of a monolithic component into a dedicated custom hook (`useSkillsDnD`) strictly separates presentation from side-effects, significantly improving component readability and reusability.
**Action:** Continually hunt for "God useEffect" blocks handling disparate UI concerns (like file drop events) and encapsulate them inside specialized hooks.

## 2026-04-21 - [InputTokenDropdown]

**Learning:** The ESLint configuration in this project does not define the `react-hooks/exhaustive-deps` rule, leading to errors when attempting to suppress it.
**Action:** Avoid using `// eslint-disable-next-line react-hooks/exhaustive-deps`, and generally favor fixing dependency arrays natively rather than suppressing the missing rule.

## 2025-04-25 - [ScheduledTaskModal] **Eradicated:** [God useEffect block / Mixed drag-and-drop side effects with presentation] **Woven:** [Custom Hook Pattern (useWorkspaceDropZone)]

- **ScheduledTaskModal:** Extracted the complex Tauri drag-and-drop subscription logic (`subscribe`, path processing, and state management) into a dedicated custom hook `useWorkspaceDropZone`.
- **Benefits:** Strictly separates drag-and-drop side-effects from the component's main render logic, drastically reducing the component's footprint and adhering to the modernized React custom hook pattern for isolated logic sharing.
