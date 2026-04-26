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

## 2026-03-09 - Missing Keyboard Focus on Markdown Text Copy Button

**Learning:** Found that an absolute positioned button using `opacity-0 group-hover:opacity-100` for hover states becomes invisible to keyboard users when tabbing through the UI unless `focus-visible:opacity-100` and related focus-visible styles are explicitly added.
**Action:** Always add `focus-visible:opacity-100` along with standard focus rings (`focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none`) whenever using `opacity-0` with group hover mechanics so the control becomes visually apparent on keyboard focus for keyboard-only and low-vision users, and pair this with clear ARIA labeling to support screen reader users.

## 2026-03-12 - Missing Keyboard Focus on Native Button and Role-based Elements

**Learning:** Found that custom `<button>` and `<div role="button">` elements that bypass the design system's `Button` component lack the standard keyboard focus indicators, making them invisible to keyboard users navigating via Tab.
**Action:** Always apply `focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none` (along with `rounded` or `rounded-sm` as needed) to any interactive element that acts as a button but does not inherit standard global button styling.

## 2024-04-04 - Wrapping SidebarTrigger with Tooltip

**Learning:** For Radix UI tooltips wrapping a trigger with `asChild`, ensuring `asChild` propagates the event up the DOM needs proper usage of Radix primitive, particularly combining `DropdownMenuTrigger` with `TooltipTrigger` on `Button`. Also `forwardRef` warnings show up if the inner button is not using forwardRef correctly, but in `SessionNotificationsBell` combining them with `asChild` creates refs warning if not nested correctly. `TooltipTrigger asChild` around `DropdownMenuTrigger asChild` is needed.
**Action:** When nesting Tooltips around Radix `DropdownMenuTrigger`, ensure both have `asChild` property so the original `<Button>` element receives both tooltip and dropdown aria/ref properties.

## 2026-04-11 - Group Focus for Hidden Controls

**Learning:** When using `opacity-X group-hover:opacity-100` to show controls inside a container on mouse hover, the controls remain invisible to keyboard users when the parent container receives focus, unless explicitly handled.
**Action:** Always pair `group-hover:opacity-100` with `group-focus-visible:opacity-100` (or `group-focus-within:opacity-100` if the interactive element is inside the group) to ensure the UI elements become visible when accessed via keyboard navigation.
