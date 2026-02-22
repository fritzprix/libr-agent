# Session Delete Options (SP7)

## Problem

Before SP7, deleting a session silently orphaned all child (subagent) sessions —
they remained visible in the list with no parent, and there was no way to clean
them up together. Additionally, the UI only removed the deleted session itself,
leaving orphaned children displayed incorrectly.

## UX Design

### Before

```
[ Trash icon ] → [ Confirm Delete ]
```

Single action, no choice. Children always orphaned silently.

### After

**Single session (no children):** behaviour unchanged — single "Confirm Delete" button.

**Session with children (`descendantCount > 0`):** two-column confirm layout:

```
┌─────────────────────┬──────────────────────┐
│   Delete all        │   Delete only this   │
│  +N subagents       │   Subagents kept     │
│  (red label)        │   (muted label)      │
└─────────────────────┴──────────────────────┘
                 [ Cancel ]
```

Each button has its own description label directly beneath it, eliminating
ambiguity about which action deletes subagents.

## Backend Implementation

### `orphan_and_delete_session` (Repository Layer)

```rust
// session_repository.rs
async fn orphan_and_delete_session(&self, session_id: &str) -> Result<(), DbError> {
    // Nullify parent_session_id for all direct children
    session::Entity::update_many()
        .col_expr(
            session::Column::ParentSessionId,
            Expr::value(Option::<String>::None),
        )
        .filter(session::Column::ParentSessionId.eq(session_id.to_string()))
        .exec(&self.db)
        .await?;
    // Delete only the parent
    Session::delete_by_id(session_id).exec(&self.db).await?;
    Ok(())
}
```

Implemented for both `SqliteSessionRepository` and `InMemorySessionRepository`.

### `delete_session_only` (Session Manager)

Terminates the session workflow, removes it from `active_sessions`, cleans
workspace directory and search index, then calls `orphan_and_delete_session`.

### Tauri Command

`agent_delete_session_only(session_id: String)` — registered in `lib.rs`.

## Frontend Implementation

### Two Delete Paths in `AgentSessionListContext`

**`deleteSession` (cascade):** BFS traversal collects all descendant IDs before
filtering — fixes a pre-existing bug where only the root session was removed
from UI state while descendant sessions remained visible.

```ts
setSessions((prev) => {
  const toRemove = new Set<string>([sessionId]);
  let frontier = [sessionId];
  while (frontier.length > 0) {
    const next: string[] = [];
    for (const s of prev) {
      if (
        s.parentSessionId !== undefined &&
        frontier.includes(s.parentSessionId)
      ) {
        toRemove.add(s.id);
        next.push(s.id);
      }
    }
    frontier = next;
  }
  return prev.filter((s) => !toRemove.has(s.id));
});
```

**`deleteSessionOnly` (orphan):** Removes the session, sets
`parentSessionId: undefined` on direct children (promoting them to top-level).

```ts
setSessions((prev) =>
  prev
    .filter((s) => s.id !== sessionId)
    .map((s) =>
      s.parentSessionId === sessionId
        ? { ...s, parentSessionId: undefined }
        : s,
    ),
);
```

### `SessionCard` Conditional UI

```tsx
{
  descendantCount === 0 ? (
    <Button variant="destructive" onClick={handleConfirmDelete}>
      Confirm Delete
    </Button>
  ) : (
    <div className="flex gap-2 w-full">
      <div className="flex flex-col items-center gap-1">
        <Button variant="destructive" onClick={handleConfirmDelete}>
          Delete all
        </Button>
        <p className="text-xs text-red-400">+{descendantCount} subagents</p>
      </div>
      <div className="flex flex-col items-center gap-1">
        <Button variant="outline" onClick={handleDeleteOnly}>
          Delete only this
        </Button>
        <p className="text-xs text-muted-foreground">Subagents kept</p>
      </div>
    </div>
  );
}
<Button variant="ghost" onClick={() => setShowConfirm(false)}>
  Cancel
</Button>;
```

## Regression Tests

Location: `src/context/__tests__/AgentSessionListContext.test.tsx`
Describe block: `AgentSessionListContext – SP7 session delete options`

| Test                                                                        | Assertion                                                                      |
| --------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| `deleteSession: BFS removes parent AND direct child from UI`                | 2-node tree → both removed after cascade delete                                |
| `deleteSession: BFS removes entire 3-level tree`                            | grandparent → parent → child: all 3 removed                                    |
| `deleteSessionOnly: removes parent, direct child becomes top-level`         | child survives with `parentSessionId: undefined`                               |
| `deleteSessionOnly: grandchild still linked to its own parent after orphan` | deleting middle node: child orphaned, grandchild's `parentSessionId` unchanged |

## Related Files

| File                                                         | Change                                          |
| ------------------------------------------------------------ | ----------------------------------------------- |
| `src-tauri/src/repositories/session_repository.rs`           | `orphan_and_delete_session` trait + SQLite impl |
| `src-tauri/src/repositories/in_memory_session_repository.rs` | in-memory impl                                  |
| `src-tauri/src/agent/session_manager.rs`                     | `delete_session_only`                           |
| `src-tauri/src/commands/agent_commands.rs`                   | `agent_delete_session_only` command             |
| `src-tauri/src/lib.rs`                                       | command registration                            |
| `src/lib/backend/session-crud.ts`                            | `deleteSessionOnly` wrapper                     |
| `src/context/AgentSessionListContext.tsx`                    | BFS cascade fix + `deleteSessionOnly`           |
| `src/features/agent/components/SessionCard.tsx`              | conditional confirm UI                          |
| `src/features/agent/components/SessionHistoryPanel.tsx`      | `onDeleteOnly` prop                             |
| `src/features/history/History.tsx`                           | `handleDeleteSessionOnly` wiring                |
| `src/context/__tests__/AgentSessionListContext.test.tsx`     | SP7 regression tests                            |
