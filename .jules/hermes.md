# Hermes's Journal - IPC Optimization Log

## 2025-05-18 - Skills IPC & Type Boundary

**Problem:** Duplicate `SkillMetadata` interfaces across 5 components and inefficient in-memory zip buffering in `download_global_skills`.

**Action:**
- **Type Sync:** Unified `SkillMetadata` in `src/types/skills.ts` to ensure 1:1 match with Rust struct, resolving type duplication and potential mismatches.
- **Memory Optimization:** Refactored `download_global_skills` in `src-tauri/src/commands/skill_management.rs` to stream the zip download directly to a temp file instead of buffering the entire archive in memory.
