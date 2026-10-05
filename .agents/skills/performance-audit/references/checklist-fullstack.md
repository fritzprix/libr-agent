# Full-Stack Performance Audit Checklist

LibrAgent is a local desktop application (Tauri + Rust + React/TypeScript + SQLite). Performance issues generally stem from blocking the event loop, excessive IPC serialization, redundant React rendering cycles, and unindexed or unbatched SQLite I/O.

---

## 1. Rust Backend & Concurrency

### Tokio & Async Blocking
- [ ] **Blocking operations inside async tasks**: Heavy CPU loops, synchronous file I/O (`std::fs`), or synchronous locks (`std::sync::Mutex`) held across `.await` points.
  - *Fix*: Use `tokio::task::spawn_blocking` for CPU-intensive/sync-blocking work, or `tokio::fs` / `tokio::sync::Mutex` when locking across `.await`.
- [ ] **Lock scope & contention**: Holding `Mutex` or `RwLock` guards longer than necessary, especially over long calculations or I/O.
  - *Fix*: Limit lock guard scope to the minimal block `{ let mut guard = lock.lock().unwrap(); ... }` or clone needed data and drop the guard immediately.
- [ ] **Unbounded channel / buffer growth**: `tokio::sync::mpsc::unbounded_channel` without backpressure causing memory spikes under load.
  - *Fix*: Use bounded channels with appropriate capacity or explicit rate-limiting.

### Memory & Allocation Hotspots
- [ ] **Excessive allocations and cloning**: Deep `.clone()` of large structs/strings inside tight loops or hot paths.
  - *Fix*: Borrow (`&str`, `&[T]`), use `Arc<T>` for shared read-only states, or reuse pre-allocated buffers (`Vec::with_capacity`).
- [ ] **Stream processing vs Large buffer buffering**: Reading huge files or LLM transcripts into memory entirely at once (`read_to_string`).
  - *Fix*: Stream line-by-line (`BufRead::lines`) or chunked processing where memory bound is critical.

---

## 2. Tauri IPC & Boundary

### Serialization & Payload Size
- [ ] **Giant IPC payloads**: Transferring multi-megabyte JSON payloads (e.g. huge full transcripts, raw diffs) over Tauri IPC invoke in a single synchronous call.
  - *Fix*: Truncate/paginate on Rust side, stream chunks, or load detailed content lazily on demand.
- [ ] **Chatty IPC calls**: Invoking multiple small Tauri commands sequentially in a loop instead of batching.
  - *Fix*: Consolidate into a single batched IPC command or event.
- [ ] **Leaked event listeners**: Tauri `listen()` unlisteners not being unregistered upon React component unmount.
  - *Fix*: Return unlisten callback inside `useEffect` cleanup.

---

## 3. Frontend (React / TypeScript)

### Rendering & State Propagation
- [ ] **Re-render cascades**: Large parent component state updates triggering re-renders of the entire tree.
  - *Fix*: Colocate state near its consumers, split contexts into discrete slices, or use selector-based subscriptions (e.g. Zustand shallow selectors).
- [ ] **Expensive inline computations in render path**: Filtering, sorting, or deep data transformation performed on every render without `useMemo`.
  - *Fix*: Wrap heavy computations in `useMemo` with minimal dependency arrays.
- [ ] **Unstable callbacks and objects passed to memoized children**: Passing inline objects/functions to `React.memo` components, invalidating memoization.
  - *Fix*: Wrap with `useCallback` / `useMemo`.
- [ ] **Unvirtualized long lists**: Rendering hundreds or thousands of DOM nodes (chat messages, logs, files) without virtualization.
  - *Fix*: Use virtual scrolling / windowing (e.g. `@tanstack/react-virtual` or pagination).

---

## 4. SQLite & Data Access

### Query & Transaction Efficiency
- [ ] **N+1 queries**: Fetching a parent record and then issuing individual `SELECT` queries for each child record in a loop.
  - *Fix*: Use `JOIN`, `IN (...)`, or single aggregated batch query.
- [ ] **Missing indexes on filtered/sorted columns**: `WHERE` and `ORDER BY` columns lacking covering indexes leading to full table scans.
  - *Fix*: Check with `EXPLAIN QUERY PLAN` and add indexes for frequently queried foreign keys or timestamps.
- [ ] **Unbatched inserts/updates**: Executing multiple `INSERT`/`UPDATE` statements individually outside an explicit transaction (`BEGIN ... COMMIT`).
  - *Fix*: Wrap bulk operations in a single transaction (up to 100x speedup in SQLite).
- [ ] **SQLite connection contention**: Blocking writes holding the database connection lock for extended periods.
  - *Fix*: Ensure WAL (Write-Ahead Logging) mode is enabled and read/write pools are properly configured.

---

## 5. Architectural Guardrails (LibrAgent Specific)

- **KISS / YAGNI**: Avoid over-abstracted layers or dual-ID / dual-state caching introduced solely under the guise of "optimization" that creates reverse lookup overhead and state drift.
- **Resource Constraints**: Never run raw `cargo` commands or heavy test suites during performance analysis. Rely on static code inspection, lightweight benchmarks, or targeted traces.
