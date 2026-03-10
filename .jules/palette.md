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

## 2025-08-01 - [Hidden Interactive Elements]

**Learning:** Placing interactive buttons inside non-semantic clickable containers (like a div with an onClick handler) makes the container inaccessible to keyboard users unless it has `role="button"` and `tabIndex`. The internal button becomes the primary keyboard interaction point, so it **must** have a clear, descriptive label.
**Action:** When using the "clickable card" pattern, ensure either the card itself is a proper button, OR the internal action buttons have explicit aria-labels describing the card's primary action (e.g., "Install [Name]" instead of just "Download").

## 2026-02-23 - [Nested Interactive Controls]

**Learning:** Nesting interactive elements (like a `button` inside a `div role="button"`) creates accessibility barriers. Screen readers may ignore the inner control.
**Action:** When a card acts as a single button but contains a visual "action button", make the container the interactive element (`role="button"`) and replace the inner button with a visual-only element (`div` with button styles + `aria-hidden="true"`).

## 2025-05-23 - [Input Type Toggling]

**Learning:** For password-like fields (like API keys), users frequently need to verify the pasted content. Providing a show/hide toggle significantly improves usability and reduces errors.
**Action:** When implementing password inputs for non-auth credentials (e.g., tokens, keys), always include a visibility toggle using state management for the input `type` attribute.

## 2026-02-25 - [Dynamic List Focus Management]

**Learning:** When adding items to a dynamic list (like environment variables), default focus behavior leaves the user stranded on the "Add" button, requiring multiple tab presses to reach the new input.
**Action:** Implement `useEffect` to track list length changes and automatically focus the first input of the newly added item using stable IDs or refs.

## 2025-02-17 - Tooltips on Action Buttons

**Learning:** Icon-only action buttons (like Edit/Delete) often lack immediate context for keyboard and mouse users, making destructive or important actions ambiguous.
**Action:** Always wrap icon-only buttons in `Tooltip` components using the standard Shadcn/Radix implementation, ensuring both visual feedback on hover and ARIA support.

## 2025-02-17 - Missing Focus Indicators on Native Buttons
**Learning:** Even when custom UI components (like the custom `Button` variant) automatically handle accessibility focus styling, plain native `<button>` elements throughout the app often lack them, impacting keyboard accessibility. Some buttons are completely invisible until hovered without proper `focus-visible` styling (like in `MarkdownText.tsx`).
**Action:** When adding or reviewing plain `<button>` components outside of the main design system button component, always ensure that `focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none` (along with `rounded` where applicable) is applied to maintain consistent keyboard navigation feedback.
