# Session Bookmarks (SP12)

## Problem

With many agent sessions, there was no way to quickly surface important or
frequently-used sessions. Users had to scroll the full history list to find
sessions they cared about.

## UX Design

### Session Card — Bookmark Toggle

Each session card in the history panel has a **bookmark icon button** (top-right
of the card). It uses `Bookmark` (outline) when inactive and `BookmarkCheck`
(filled, yellow) when active. Clicking it toggles the bookmark state.

```
┌────────────────────────────────────────┐
│ Session name                    [🔖]  │
│ model · provider · updated_at          │
└────────────────────────────────────────┘
```

### History Panel — Bookmarked Filter

A **"Bookmarked"** toggle button appears in the session history panel header.
When active, the list filters to show only bookmarked sessions.

```
[ All ]  [ Bookmarked ▶ ]     ← filter toggle
────────────────────────
  [🔖 active] My Key Session
  [🔖 active] Research Agent
```

## Backend Implementation

### Database Migration

`m20260301_000011_add_bookmark_to_sessions` adds:

```sql
ALTER TABLE sessions ADD COLUMN is_bookmarked BOOLEAN NOT NULL DEFAULT 0;
```

SeaORM migration registered in `migration/src/lib.rs`.

### Entity

`src-tauri/src/entity/session.rs` — added `pub is_bookmarked: bool` field.

### Repository

`SessionRepository` trait gained a new method:

```rust
async fn toggle_bookmark(&self, session_id: &str, bookmarked: bool) -> Result<(), DbErr>;
```

Implemented in both `DbSessionRepository` (real DB) and `InMemorySessionRepository`
(tests). `SessionMetadata` and its `try_from` conversion include `is_bookmarked`.
Serialized as `isBookmarked` via `#[serde(rename_all = "camelCase")]`.

`upsert_session` also persists `is_bookmarked` when upserting.

### DB Schema Validator

`db_schema_validator.rs` validates the `is_bookmarked` column is present on startup.

### Tauri Command

```rust
#[tauri::command]
pub async fn agent_toggle_session_bookmark(
    session_id: String,
    bookmarked: bool,
    state: State<'_, AppState>,
) -> Result<(), String>
```

Registered in `lib.rs`.

## Frontend Implementation

### Types

`AgentSessionMetadata` and `AgentSession` both expose `isBookmarked?: boolean`.

### Backend Wrapper

`src/lib/backend/session-crud.ts`:

```ts
export async function toggleSessionBookmark(
  sessionId: string,
  bookmarked: boolean,
): Promise<void>;
```

### Context — Optimistic Update

`AgentSessionListContext` provides a `toggleBookmark(sessionId, bookmarked)` action.

The pattern is optimistic: the local list is updated immediately, the backend call
is made in the background, and on failure the list is reverted to the previous state.

### Components

| Component                 | Change                                                                                             |
| ------------------------- | -------------------------------------------------------------------------------------------------- |
| `SessionCard.tsx`         | Bookmark icon button; `onToggleBookmark?: (id) => void` prop                                       |
| `SessionHistoryPanel.tsx` | `showBookmarkedOnly` state + filter; "Bookmarked" toggle button; wires `onToggleBookmark` to cards |
| `History.tsx`             | Reads `toggleBookmark` from `AgentSessionListContext` and passes to panel                          |

## Files Changed

| File                                                         | Change                                        |
| ------------------------------------------------------------ | --------------------------------------------- |
| `migration/src/m20260301_000011_add_bookmark_to_sessions.rs` | New migration                                 |
| `migration/src/lib.rs`                                       | Registered migration #11                      |
| `src/entity/session.rs`                                      | `is_bookmarked` field                         |
| `src/repositories/session_repository.rs`                     | `toggle_bookmark`, `is_bookmarked` everywhere |
| `src/repositories/in_memory_session_repository.rs`           | `toggle_bookmark` stub                        |
| `src/db_schema_validator.rs`                                 | Column validation                             |
| `src/commands/agent_commands.rs`                             | `agent_toggle_session_bookmark` command       |
| `src/lib.rs`                                                 | Command registered                            |
| `src/models/agent-ipc.ts`                                    | `isBookmarked` on `AgentSessionMetadata`      |
| `src/models/agent.ts`                                        | `isBookmarked` on `AgentSession`              |
| `src/lib/backend/session-crud.ts`                            | `toggleSessionBookmark` wrapper               |
| `src/context/AgentSessionListContext.tsx`                    | `toggleBookmark` action                       |
| `src/features/agent/components/SessionCard.tsx`              | Bookmark button UI                            |
| `src/features/agent/components/SessionHistoryPanel.tsx`      | Bookmarked filter UI                          |
| `src/features/history/History.tsx`                           | Wired toggle action                           |
