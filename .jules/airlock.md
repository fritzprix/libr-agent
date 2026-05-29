## 2025-05-29 - Batched IPC Operations
**Learning:** Bulk operations driven from React (like looping through all items to delete them) create severe N+1 IPC call patterns that choke the Tauri bridge.
**Action:** Always use or create batched Tauri commands for bulk operations (e.g., clearing all state) instead of orchestrating the loop in the React frontend.
