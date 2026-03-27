# Airlock Journal

This journal tracks critical learnings related to boundary enforcement and Separation of Concerns (SoC) between the React frontend and Rust backend in this Tauri application.

## 2024-05-24 - Extracting Cascade Delete Logic to Rust
**Learning:** The React frontend was duplicating the backend's database-level CASCADE delete logic by using a BFS tree traversal over the session list to find all descendant IDs whenever a session was deleted. This leaked business logic (the knowledge of how deletions cascade) into the view layer.
**Action:** Always have the Rust backend return authoritative lists of affected/deleted entities (DTOs) after mutating operations, so the React frontend only has to filter the state by ID instead of recomputing the mutation side-effects.