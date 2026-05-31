## 2025-05-29 - Batched IPC Operations
**Learning:** Bulk operations driven from React (like looping through all items to delete them) create severe N+1 IPC call patterns that choke the Tauri bridge.
**Action:** Always use or create batched Tauri commands for bulk operations (e.g., clearing all state) instead of orchestrating the loop in the React frontend.
## 2026-05-26 - Clear All Sessions Optimization
**Learning:** Found an N+1 IPC call issue in `dbUtils.clearAllSessions()` where React iterates over all sessions and invokes `deleteSession` for each one over IPC, even though there's an existing `agent_clear_all_sessions` Tauri command.
**Action:** Replace the chatty loop in `dbUtils.clearAllSessions` with a single IPC call to the backend. Same for `dbUtils.clearAllAssistants` and others if possible, but definitely `clearAllSessions`.
