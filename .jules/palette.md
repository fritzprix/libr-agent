## 2024-06-20 - Palette Journal Initialized
**Learning:** Initializing palette journal.
**Action:** Ready to log.

## 2024-06-20 - Icon-Only Buttons Require Tooltips
**Learning:** While `aria-label` is sufficient for screen readers on icon-only buttons, sighted users navigating with a mouse or keyboard require a visible tooltip to understand the button's action if the icon is not universally understood.
**Action:** Always wrap icon-only buttons with `<Tooltip>` components, reusing the same translation key used for the `aria-label`.
