## 2024-05-24 - Redundant ARIA Toggle Labels
**Learning:** Screen readers natively announce the expanded or collapsed state of elements with the `aria-expanded` attribute. Including action words like "Toggle" in the `aria-label` creates redundant and overly verbose announcements.
**Action:** For toggle buttons and accordions using `aria-expanded`, set the `aria-label` to simply describe the content (e.g., the `title`) without action verbs.
