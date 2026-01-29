# Prism's Journal - Visual Debt Log

This log tracks visual inconsistencies, layout breaks, and design system violations identified by Prism.

## Format

## YYYY-MM-DD - [Component/Page] **Visual Bug:** [Issue] **Fix:** [Applied Pattern]

## Entries

## 2026-01-23 - [BadgeLegacy] **Visual Bug:** Hardcoded colors (yellow-400, green-400) **Fix:** Use semantic tokens (warning, success)

## 2026-01-23 - [SessionItem] **Visual Bug:** Inline styles and raw button usage **Fix:** Remove style, use Button component

## 2026-01-23 - [SessionList] **Visual Bug:** Template literal className **Fix:** Use cn() utility

## 2026-01-24 - [LoadingSpinner] **Visual Bug:** Hardcoded color (border-t-green-400) **Fix:** Use semantic token (border-t-primary)

## 2026-01-24 - [StatusIndicator] **Visual Bug:** Hardcoded colors (green-400, red-400, gray-400) **Fix:** Use semantic tokens (success, destructive, muted-foreground)

## 2026-01-24 - [InputWithLabel] **Visual Bug:** Hardcoded colors (text-green-400, border-red-400) **Fix:** Use semantic tokens (text-success, border-destructive)

## 2026-01-24 - [AgentChatStatusBar] **Visual Bug:** Hardcoded colors and inconsistent mapping **Fix:** Use semantic tokens and unified status colors

## 2026-01-24 - [SessionCard] **Visual Bug:** Hardcoded colors and inconsistent mapping **Fix:** Use semantic tokens and unified status colors

## 2026-01-25 - [SessionFilesPopover] **Visual Bug:** Hardcoded colors (text-gray-400, text-green-400) **Fix:** Use semantic tokens (text-muted-foreground, text-success)

## 2026-01-25 - [ErrorBubble] **Visual Bug:** Hardcoded colors (amber, blue, red) and opacity hacks **Fix:** Use semantic tokens (warning, primary, destructive) with opacity modifiers

## 2026-01-25 - [AssistantCard] **Visual Bug:** Hardcoded badge colors (blue-100, green-100) **Fix:** Use semantic tokens (primary/10, success/10)

## 2026-01-25 - [TokenMetricsBadge] **Visual Bug:** Hardcoded colors (blue, green, yellow) **Fix:** Use semantic tokens (primary, success, warning)

## 2026-01-25 - [FileAttachment] **Visual Bug:** Hardcoded colors (green-400, red-400) **Fix:** Use semantic tokens (success, destructive)

## 2026-01-26 - [AgentPlanningPanel] **Visual Bug:** Hardcoded priority colors (red-500, yellow-500, green-500) **Fix:** Use semantic tokens (destructive, warning, success)

## 2026-01-26 - [AgentChatHeader] **Visual Bug:** Hardcoded active state color (text-blue-400) **Fix:** Use semantic token (text-primary)

## 2026-01-26 - [AgentWorkspacePanel] **Visual Bug:** Hardcoded alert colors (yellow-500, green-500) **Fix:** Use semantic tokens (warning, success)

## 2026-01-26 - [AgentToolsModal] **Visual Bug:** Hardcoded error text color (text-red-500) **Fix:** Use semantic token (text-destructive)

## 2026-01-26 - [PlaybookCard] **Visual Bug:** Hardcoded bookmark color (yellow-500) and magic number width **Fix:** Use semantic token (warning) and grid value (max-w-36)

## 2026-01-26 - [FieldWrapper] **Visual Bug:** Hardcoded error text color (text-red-400) **Fix:** Use semantic token (text-destructive)

## 2026-01-26 - [AgentToolCallDetails] **Visual Bug:** Hardcoded error colors (red-50/200/600/900) and dark mode overrides **Fix:** Use semantic tokens (destructive) with opacity modifiers

## 2026-01-26 - [AgentChatAttachedFiles] **Visual Bug:** Magic number width (max-w-[150px]) **Fix:** Use grid value (max-w-36)

## 2026-01-27 - [AgentWorkspacePanel] **Visual Bug:** Magic number width (text-[10px]) **Fix:** Use standard token (text-xs)

## 2026-01-27 - [TokenMetricsBadge] **Visual Bug:** Magic number width (text-[10px]) **Fix:** Use standard token (text-xs)

## 2026-01-27 - [AgentModelPicker] **Visual Bug:** Arbitrary width (min-w-[120px]) **Fix:** Use grid value (min-w-32)

## 2026-01-27 - [AgentToolCallDetails] **Visual Bug:** Arbitrary height (max-h-[400px]) **Fix:** Use grid value (max-h-96)

## 2026-01-27 - [AgentTerminalHeader] **Visual Bug:** Arbitrary width (max-w-[300px]) **Fix:** Use standard token (max-w-xs)

## 2026-01-27 - [AgentDraftChatView] **Visual Bug:** Magic numbers (text-[10px], min-h-[44px]) **Fix:** Use standard tokens (text-xs, min-h-11)

## 2026-01-27 - [Playbook/Card] **Visual Bug:** Arbitrary width (max-w-[120px]) **Fix:** Use grid value (max-w-32)

## 2026-01-27 - [Playbook/List] **Visual Bug:** Arbitrary width (sm:w-[250px]) **Fix:** Use grid value (sm:w-64)

## 2026-01-27 - [SortControls] **Visual Bug:** Arbitrary width (w-[200px]) **Fix:** Use grid value (w-52)

## 2026-01-27 - [AgentChatView] **Visual Bug:** Arbitrary height (max-h-[100vh]) **Fix:** Use standard token (max-h-screen)

## 2026-01-27 - [AgentDraftChatView] **Visual Bug:** Arbitrary height (max-h-[100vh]) **Fix:** Use standard token (max-h-screen)

## 2026-01-27 - [AgentWorkspacePanel] **Visual Bug:** Magic number inline styles (8 + depth * 16) **Fix:** Use named constants (BASE_PADDING, DEPTH_STEP)
