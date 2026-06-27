## 2024-06-20 - Palette Journal Initialized
**Learning:** Initializing palette journal.
**Action:** Ready to log.

## 2024-06-20 - Icon-Only Buttons Require Tooltips
**Learning:** While `aria-label` is sufficient for screen readers on icon-only buttons, sighted users navigating with a mouse or keyboard require a visible tooltip to understand the button's action if the icon is not universally understood.
**Action:** Always wrap icon-only buttons with `<Tooltip>` components, reusing the same translation key used for the `aria-label`.
## 2025-02-09 - Add tooltip to unbookmark button
**Learning:** Sighted mouse users lack context for icon-only buttons like "Remove bookmark" without visual tooltips, even when `aria-label` is present for screen readers.
**Action:** Always wrap icon-only action buttons in `Tooltip` components to ensure parity between visual context and semantic accessibility labels.
## 2024-06-27 - Add ARIA label to Switch
**Learning:** Switch components without explicitly linked labels are completely inaccessible to screen readers, leaving users guessing their function.
**Action:** Always provide an `aria-label` to standalone `Switch` components to clearly convey their toggling action and associated target.
