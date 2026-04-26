# ROSETTA'S JOURNAL - LOCALIZATION LOG

## 2026-03-20 - [AgentPlanningPanel]

**Extracted:** 8
**Languages updated:** [EN, KO]
**Notes:** Localized AgentPlanningPanel strings like "AI Planning", "Current Goal", "Tasks", and priority badges. Added new `agent.planning` namespace to `common.json`.

## 2026-03-04 - [Sidebar Navigation]

**Extracted:** 3
**Languages updated:** [EN, KO]
**Notes:** Localized `AppSidebar` and `ui/sidebar`. Extracted "Scheduled Tasks" and hidden SR-only attributes like `SheetTitle` and `SheetDescription` for the mobile sidebar.

## 2024-05-22 - [MCP Server Management]

**Extracted:** ~50
**Languages updated:** [EN, KO]
**Notes:** Localized MCP Server Management (Extensions) and Dialog. Added `mcpServer` namespace to `common.json`. Used English strings for Korean as placeholders.

## 2024-05-25 - [App Shell & Error Boundary]

**Extracted:** 13
**Languages updated:** [EN, KO]
**Notes:** Localized `AppSidebar`, `ThemeToggle`, and `ErrorBoundary`. Added `sidebar`, `theme`, and `error` namespaces to `common.json`.

## 2026-02-23 - [Settings/GeneralTab]

**Extracted:** 10
**Languages updated:** [EN, KO]
**Notes:** Localized `GeneralTab` and `SkillsListModal`. Added `settings.general` keys and updated `settings.skills.modalTitle` with pluralization support.

## 2026-02-26 - [Playbook]

**Extracted:** ~35
**Languages updated:** [EN, KO]
**Notes:** Fully localized the Playbook feature (List, Card, Grouping, SortControls). Refactored `grouping-utils.ts` to return translation keys instead of hardcoded strings. Preserved existing Korean translations found in `List.tsx`.

## 2026-02-25 - [Assistant Feature]

**Extracted:** ~35
**Languages updated:** [EN, KO]
**Notes:** Localized `AssistantEditor`, `AssistantList`, `BuiltInToolsEditor`, and `SkillsEditor`. Added `assistant.tabs`, `assistant.list`, `assistant.builtin`, and expanded `skills` namespaces in `common.json`.

## 2026-02-28 - [Session History & Management]

**Extracted:** ~40
**Languages updated:** [EN, KO]
**Notes:** Localized `History` view, `SessionHistoryPanel`, and `SessionCard`. Added `sessionHistory` namespace in `common.json` with keys for panel UI, tab labels, toast messages, and session card actions. Used standard i18n pluralization rules for `subagentsCount`.

## 2026-03-01 - [MCP Server Page]

**Extracted:** 1
**Languages updated:** [EN, KO]
**Notes:** Replaced hardcoded "Manage your AI extensions and tools" string with `mcpServer.pageSubtitle` in `MCPServerPage.tsx`. Updated corresponding translations in English and Korean common.json.

## 2026-03-03 - [Agent Chat Components]

**Extracted:** 62
**Languages updated:** [EN, KO]
**Notes:** Localized hardcoded strings, toasts, placeholders, aria-labels, and tooltips across multiple Agent chat components including `AgentChatHeader`, `AgentWorkspacePanel`, `AgentChatStatusBar`, `AgentChatInput`, and the `useAgentFileAttachment` hook. Created new nested namespaces within `agent` (`header`, `workspace`, `statusBar`, `attachment`, `input`) in `common.json`.

## 2026-03-03 - [Scheduled Tasks]

**Extracted:** ~50
**Languages updated:** [EN, KO]
**Notes:** Localized `ScheduledTasksPage`, `ScheduledTaskModal`, `MentionTextarea`, and `ScheduleBuilder`. Created `scheduledTasks` namespace in `common.json` handling page structure, modal forms, and complex chron descriptions with standard pluralization.

## 2026-03-08 - [SessionFilesPopover]

**Extracted:** 1
**Languages updated:** [EN, KO]
**Notes:** Localized `SessionFilesPopover` strings for file listing, counts, and actions, using English as placeholders for Korean where needed.

## 2026-03-09 - [SettingsPage/ProviderCard]

**Extracted:** 10
**Languages updated:** [EN, KO]
**Notes:** Localized `SettingsPage` provider card labels, descriptions, and actions, and synced EN/KO entries in `common.json`.

## 2026-03-09 - [Agent Draft Chat View]

**Extracted:** 16
**Languages updated:** [EN, KO]
**Notes:** Localized `AgentDraftChatView`. Created new nested namespace `agent.draft` in `common.json`. Handled toasts, errors, and static descriptive text strings.

## 2026-03-13 - [AgentModelPicker]

**Extracted:** 4
**Languages updated:** [EN, KO]
**Notes:** Localized `AgentModelPicker` provider and model select placeholders, loading state, and refresh button tooltip.

## 2026-03-14 - [Settings]

**Extracted:** 9
**Languages updated:** [EN, KO]
**Notes:** Localized "Danger Zone" title and performance input placeholders across settings components. Synced missing reset-related strings in Korean.

## 2026-03-16 - [ErrorBubble/LoadingSpinner]

**Extracted:** 5
**Languages updated:** [EN, KO]
**Notes:** Localized `ErrorBubble` and `LoadingSpinner` components and added new `errorBubble` namespace to EN/KO.

## 2026-04-18 - [Agent Draft Chat View Updates]

**Extracted:** 0
**Languages updated:** [EN]
**Notes:** Updated default English fallback strings in `AgentDraftChatView.tsx` to exactly match their actual values in `en.json` to fix broken inline copy.

## 2026-04-23 - [KnowledgePage]

**Extracted:** 25
**Languages updated:** [EN, KO]
**Notes:** Fully localized `KnowledgePage.tsx` and its subcomponents (`KnowledgeGraphPreview`, `KnowledgeListItemCard`, `KnowledgeDetailDialog`). Added `knowledge` namespace to `common.json` for EN and KO. English used as placeholders in KO.

## 2026-04-18 - [ServerToolsModal]

**Extracted:** 8
**Languages updated:** [EN, KO]
**Notes:** Localized ServerToolsModal strings like "Tools List", "Connecting to server…", and input schema headers. Handled pluralization for tool counts and updated `common.json` namespace for `mcpServer.toolsModal`.

## 2024-05-28 - [SystemPerformanceSettings]

**Extracted:** 5
**Languages updated:** [EN, KO]
**Notes:** Added missing translation keys for `settings.system.automation`, `maxScheduledTaskGroups`, and the `scheduledTaskMinimumInterval` label/description keys plus related placeholders in `common.json`. The strings were already wrapped in `t()` in the component but missing from the locales files.
