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

## 2025-08-20 - [Interactive Status Indicators]

**Learning:** Static status indicators (like file counts in chat inputs) frustrate users when they hide details. Making these indicators interactive (e.g., via DropdownMenu) allows users to review and manage context without cluttering the UI.
**Action:** Wrap summary indicators in interactive elements (Dropdown/Popover) to provide detailed views and actions for the summarized content.
