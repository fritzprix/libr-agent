## 2025-03-01 - Bulk Operations IPC Batching
**Learning:** React iterating over items and performing individual IPC calls per item (e.g. iterating over all sessions to call `agent_delete_session` for each) leads to an N+1 IPC call pattern, creating massive performance overhead on the Tauri bridge.
**Action:** Always replace frontend looping over IPC calls with a single backend batched IPC command (like `agent_clear_all_sessions`) that handles the loop on the Rust side, adhering to the Thin Client philosophy.
