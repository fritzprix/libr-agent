## 2024-06-20 - Palette Journal Initialized
**Learning:** Initializing palette journal.
**Action:** Ready to log.

## 2024-06-20 - Icon-Only Buttons Require Tooltips
**Learning:** While `aria-label` is sufficient for screen readers on icon-only buttons, sighted users navigating with a mouse or keyboard require a visible tooltip to understand the button's action if the icon is not universally understood.
**Action:** Always wrap icon-only buttons with `<Tooltip>` components, reusing the same translation key used for the `aria-label`.
## 2025-02-09 - Add tooltip to unbookmark button
**Learning:** Sighted mouse users lack context for icon-only buttons like "Remove bookmark" without visual tooltips, even when `aria-label` is present for screen readers.
**Action:** Always wrap icon-only action buttons in `Tooltip` components to ensure parity between visual context and semantic accessibility labels.
## 2024-07-08 - Replace native title attributes with Tooltip components for icon-only buttons
**Learning:** Native `title` attributes on icon-only buttons provide poor UX as they have delayed rendering, lack visual consistency with the app's design system, and are not reliably accessible via keyboard navigation alone.
**Action:** Always wrap icon-only buttons in the design system's `<Tooltip>` component to ensure immediate, visually consistent, and fully keyboard-accessible feedback instead of relying on the native HTML `title` attribute.
