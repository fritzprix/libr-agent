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
