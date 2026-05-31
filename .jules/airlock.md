## 2026-05-26 - Clear All Sessions Optimization
**Learning:** Found an N+1 IPC call issue in `dbUtils.clearAllSessions()` where React iterates over all sessions and invokes `deleteSession` for each one over IPC, even though there's an existing `agent_clear_all_sessions` Tauri command.
**Action:** Replace the chatty loop in `dbUtils.clearAllSessions` with a single IPC call to the backend. Same for `dbUtils.clearAllAssistants` and others if possible, but definitely `clearAllSessions`.
