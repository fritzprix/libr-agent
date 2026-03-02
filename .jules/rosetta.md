# ROSETTA'S JOURNAL - LOCALIZATION LOG

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
