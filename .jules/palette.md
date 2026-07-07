## 2024-06-20 - Palette Journal Initialized
**Learning:** Initializing palette journal.
**Action:** Ready to log.

## 2024-06-20 - Icon-Only Buttons Require Tooltips
**Learning:** While `aria-label` is sufficient for screen readers on icon-only buttons, sighted users navigating with a mouse or keyboard require a visible tooltip to understand the button's action if the icon is not universally understood.
**Action:** Always wrap icon-only buttons with `<Tooltip>` components, reusing the same translation key used for the `aria-label`.
## 2025-02-09 - Add tooltip to unbookmark button
**Learning:** Sighted mouse users lack context for icon-only buttons like "Remove bookmark" without visual tooltips, even when `aria-label` is present for screen readers.
**Action:** Always wrap icon-only action buttons in `Tooltip` components to ensure parity between visual context and semantic accessibility labels.
## 2024-07-07 - Replace native title with Tooltip for icon buttons
**Learning:** Using native `title` attributes on icon-only buttons provides poor visibility and inconsistent feedback across browsers/OS. Screen reader support can also be inconsistent compared to ARIA patterns used by the design system's Tooltip component.
**Action:** Always replace native `title` attributes with the design system's `<Tooltip>` component on icon-only buttons for immediate, visually consistent, and keyboard-accessible feedback.
