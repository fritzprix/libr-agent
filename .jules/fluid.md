# Fluid - Performance & UX Log 🌊

This log tracks significant performance improvements, Jank removals, and UX polish applied to the application.
Only add entries when a major blocking issue is resolved or a critical flow is enhanced with optimistic UI/loading states.

## Format
## YYYY-MM-DD - [Component/Flow]
**Bottleneck:** [Blocking task / Missing feedback]
**Flow Restored:** [UX Improvement Applied]

---

## 2024-05-23 - [MCPServerManagement/Delete & Toggle]
**Bottleneck:**
- Delete dialog closed immediately without feedback ("Naked Await"), confusing users if the operation failed or took time.
- Active toggle switch was unresponsive/laggy, waiting for full server roundtrip before updating state.
**Flow Restored:**
- **Delete:** Added `isDeleting` state to `AlertDialogAction` with a spinner. Prevented dialog closure during async operation.
- **Toggle:** Implemented Optimistic UI updates using SWR `mutate` + `optimisticData` for instant feedback (0ms latency).
