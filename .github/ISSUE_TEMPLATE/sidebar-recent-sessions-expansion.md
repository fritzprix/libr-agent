# refactor(frontend): expand Recent Sessions in sidebar with infinite scroll and expandable tree

## Problem

The **Recent Sessions** section in the app sidebar is cramped:

- Hard-capped at 5 items (`rows.length >= 5` in a `useMemo`)
- Flat list with simple indentation (nestingLevel 0-2)
- No expand/collapse for parent sessions
- No pagination or infinite scroll
- Squeezed between navigation groups and the footer, doesn't fill available space

Users report it's inconvenient to see and navigate their recent agent sessions.

## Proposed Solution

### Target State

```
SidebarContent (full height, scrollable)
├── Main Nav (History, Chat, etc.) -- fixed
├── Library Nav (Knowledge, Assistants, etc.) -- fixed
├── Recent Sessions (expands to fill remaining space)
│   ├── Session 1 (always visible)
│   │   ├── > Sub-session A (expandable)
│   │   └── > Sub-session B (expandable)
│   ├── Session 2 (always visible)
│   └── [loading indicator] (appears when scrolled to bottom)
└── Footer (Settings) -- fixed
```

### Two Features

1. **Expand to bottom + infinite scroll** -- The Recent Sessions group fills all remaining vertical space in SidebarContent. When the user scrolls to the bottom, the next page of sessions is loaded automatically via the existing cursor-based API.

2. **Expandable parent sessions** -- Root sessions are always visible. Clicking a parent expands to reveal its child sessions (sub-agents, org members, delegated tasks), and collapses them back.

## Feasibility

**Both features are highly feasible.** All required infrastructure already exists:

| Existing Infrastructure                                                                    | Used By                                        |
| ------------------------------------------------------------------------------------------ | ---------------------------------------------- |
| `AgentSessionListContext.loadMoreSessions()` + `hasMoreSessions` / `isLoadingMoreSessions` | Global session state                           |
| `computeSessionTree()` from `session-tree.ts`                                              | SessionHistoryPanel tree computation           |
| `SessionCard` with `hasExpandableChildren` / `isExpanded` / `onToggleExpand`               | SessionHistoryPanel expandable rows            |
| `useInfiniteScroll` hook                                                                   | SessionHistoryPanel scroll-driven pagination   |
| `useKnownDirectChildCounts` hook                                                           | SessionHistoryPanel unloaded children tracking |
| Cursor-based pagination API (`agent_list_sessions`)                                        | Backend already supports it                    |

This is a **refactor, not a rewrite**. The history panel already has every feature we need -- the sidebar just needs to borrow from it with a lighter visual style.

## Implementation Plan

### File 1: `src/components/layout/AppSidebar.tsx`

**Remove:**

- The `recentSessions` memo (~120 lines of flat list + depth-2 hardcap logic)
- `buildChildrenMap` import from `session-utils`

**Add:**

- Import `computeSessionTree` from `features/agent/components/session-tree`
- Import `SessionCard` from `features/agent/components/SessionCard`
- Import `useInfiniteScroll` from `features/agent/components/use-session-scroll`
- Import `useKnownDirectChildCounts` from `features/agent/components/use-known-direct-child-counts`
- Import `buildDescendantStatusCounts` from `@/lib/session-utils`
- State: `manuallyExpandedSessionIds`, `collapsedAutoExpandedSessionIds` (via `useState<Set<string>>`)
- Refs: `sidebarContentRef`, `loadMoreSentinelRef`

**New sidebar rendering:**

- Reuse `AgentSessionListContext`'s `loadMoreSessions`, `hasMoreSessions`, `isLoadingMoreSessions`
- Compute sidebar rows via `computeSessionTree()` with minimal config (no search/filter/sort)
- Precompute `descendantStatusCounts` for descendant badges
- Wire `useInfiniteScroll` to `SidebarContent`'s native scroll
- Replace flat `SidebarMenu` list with direct `SessionCard` rows

**Layout change:**

- Recent Sessions `SidebarGroup` gets `className="flex-1 min-h-0"` to fill remaining space
- `SidebarGroupContent` replaced with a plain `div` wrapper for `SessionCard` rows
- `loadMoreSentinelRef` div appended after rows for infinite scroll detection
- Loading indicator shown while `isLoadingMoreSessions`

### File 2: `src/features/agent/components/SessionCard.tsx`

**Add `variant` prop to `SessionCardProps`:**

```typescript
variant?: 'list' | 'sidebar';  // default: 'list'
```

- `list` mode = current full-featured card (used by SessionHistoryPanel) -- no change
- `sidebar` mode = compact single-line row:
  - Status dot (color-coded by status)
  - Expand/collapse chevron (only if `hasExpandableChildren`)
  - Session name (truncated)
  - Bookmark star (if bookmarked)
  - **No** status badge, **no** model info, **no** action buttons, **no** lineage hints
  - ~40 lines vs ~370 lines current

### File 3: (No new files)

All imports come from existing modules.

## Risk Assessment

| Risk                                                                                      | Severity             | Mitigation                                                                                                                     |
| ----------------------------------------------------------------------------------------- | -------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `SidebarContent` scroll container doesn't match `findScrollParent` in `useInfiniteScroll` | Low                  | Hook walks up DOM for `overflow: auto` -- `SidebarContent` has that class. Add `console.log(scrollParent)` on mount to verify. |
| `computeSessionTree` is heavy for sidebar                                                 | Low                  | Already memoized. Only runs on session list change, not per-render. With 50-100 sessions it's ~2ms.                            |
| Sidebar variant loses too much info                                                       | Low                  | Sidebar is a session launcher, not a viewer. Users can click into the session for full details.                                |
| Collapsed sidebar (icon mode) still shows nothing                                         | None (no regression) | `!isCollapsed` guard stays.                                                                                                    |

## Files Changed

| File                                            | Change Type                   | Lines (est.) |
| ----------------------------------------------- | ----------------------------- | ------------ |
| `src/components/layout/AppSidebar.tsx`          | Rewrite recent-sessions block | ~-80 / +120  |
| `src/features/agent/components/SessionCard.tsx` | Add `variant` prop            | ~+60         |

## Dependencies

- Depends on: None
- Blocks: None
- Related to: #1634 (refactor SessionCard by concern), #1649 (UI/UX Audit)

## Acceptance Criteria

- [ ] Recent Sessions section fills all remaining vertical space in the sidebar (below nav groups, above footer)
- [ ] Infinite scroll loads more sessions when user scrolls to the bottom
- [ ] Parent sessions show an expand/collapse chevron when they have child sessions
- [ ] Expanding a parent reveals its child sessions with appropriate indentation
- [ ] Collapsing a parent hides child sessions
- [ ] Loading indicator shows during `loadMoreSessions`
- [ ] No visual regression on collapsed (icon) sidebar mode
- [ ] No change to SessionHistoryPanel (list mode unchanged)
- [ ] `pnpm refactor:validate` passes
