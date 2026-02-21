# Hermes's Journal - IPC Optimization Log

## 2025-05-18 - [download_global_skills, SkillMetadata] **IPC Fix:** [Inefficiency] **Optimized:** [Solution applied]
**Optimized:**
- **Type Sync:** Unified `SkillMetadata` in `src/types/skills.ts` to ensure 1:1 match with Rust struct, resolving type duplication and potential mismatches.
- **Async Execution:** Refactored `download_global_skills` in `src-tauri/src/commands/skill_management.rs` to stream the zip download to a temporary file instead of buffering it in memory, reducing memory footprint during skill installation.
