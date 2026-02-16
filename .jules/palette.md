## 2025-05-18 - [Accessible Form Fields]

**Learning:** React 18's useId generates IDs with colons (e.g., :r1:), which breaks standard CSS selectors (#:r1:) unless escaped.
**Action:** Use attribute selectors ([id=':r1:']) for testing/styling dynamically generated React IDs.

## 2025-05-20 - [Auto-resizing Textarea]

**Learning:** Chat inputs without auto-resize functionality create significant friction for multi-line messages. Using `useLayoutEffect` prevents visual flicker during resize operations compared to `useEffect`.
**Action:** Always implement auto-resize for chat inputs using `scrollHeight` and `useLayoutEffect`.

## 2025-06-25 - [Accessible Lists]

**Learning:** File lists rendered as `div`s with icon-only buttons create poor screen reader experience. Using `ul`/`li` provides structure, and dynamic `aria-label` (e.g., "Remove [filename]") on buttons is critical for context.
**Action:** Always use semantic list elements for collections and ensure action buttons in lists have context-specific labels.

## 2025-07-15 - [Accessible Dialog Descriptions]

**Learning:** Radix UI `DialogContent` triggers accessibility warnings if `DialogDescription` is missing. Even if the content is self-explanatory (like a list), a description is required for screen readers or must be explicitly disabled via `aria-describedby={undefined}`.
**Action:** Always include a `DialogDescription` or explicit `aria-describedby` when implementing modals using `@/components/ui/dialog`.

## 2025-08-10 - [Accessible Loading States]

**Learning:** Loading spinners implemented as empty `div`s with CSS animations are invisible to screen readers, leaving users unaware of background processes.
**Action:** Always add `role="status"` and a visually hidden label (e.g., via `sr-only` span) to loading components to ensure status updates are announced.
