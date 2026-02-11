## 2025-05-18 - [Accessible Form Fields]

**Learning:** React 18's useId generates IDs with colons (e.g., :r1:), which breaks standard CSS selectors (#:r1:) unless escaped.
**Action:** Use attribute selectors ([id=':r1:']) for testing/styling dynamically generated React IDs.

## 2025-05-20 - [Auto-resizing Textarea]

**Learning:** Chat inputs without auto-resize functionality create significant friction for multi-line messages. Using `useLayoutEffect` prevents visual flicker during resize operations compared to `useEffect`.
**Action:** Always implement auto-resize for chat inputs using `scrollHeight` and `useLayoutEffect`.

## 2025-06-25 - [Accessible Lists]

**Learning:** File lists rendered as `div`s with icon-only buttons create poor screen reader experience. Using `ul`/`li` provides structure, and dynamic `aria-label` (e.g., "Remove [filename]") on buttons is critical for context.
**Action:** Always use semantic list elements for collections and ensure action buttons in lists have context-specific labels.

## 2025-06-25 - [Radix Tooltip Composition]

**Learning:** Wrapping a functional component (like `Button`) inside `TooltipTrigger` without `asChild` creates invalid nesting and breaks accessibility/event propagation in Radix UI.
**Action:** Always use `<TooltipTrigger asChild>` when the trigger content is a custom component that forwards refs.
