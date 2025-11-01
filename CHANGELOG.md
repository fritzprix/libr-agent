# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Breaking Changes

- **Platform-specific shell execution tool names**:
  - Windows: `execute_shell` renamed to `execute_windows_cmd` to clarify cmd.exe usage
  - Unix: `execute_shell` remains unchanged (bash/sh)
  - This change improves tool naming clarity and prevents cross-platform confusion
  - Tool descriptions and examples are now platform-specific
  - **Migration**: Update any hardcoded tool name references from `execute_shell` to `execute_windows_cmd` on Windows

## [0.1.1] - 2025-10-11

### Highlights

- Improved History UX and search plumbing (global message search aggregation, session-level hit counts).
- Introduced built-in WebMCP UI tools for interactive prompts and small visualizations (prompt_user, reply_prompt, visualize_data).
- Backend message persistence improvements: groundwork for SQLite-backed message storage and Tauri commands for message CRUD.
- BM25 search integration planning and initial index metadata handling; session index cleanup implemented to remove orphaned index files.

### Added

- WebMCP UI tools (built-in): `prompt_user`, `reply_prompt`, `visualize_data`.
  - HTML-based UI resources returned as multipart MCP responses to enable interactive in-chat UI flows.
  - Worker and module registry updated to include `ui` server module.
- History search improvements:
  - `searchMessages` Tauri wrapper added to `rust-backend-client` (client-side wrappers and SWR integration).
  - Frontend aggregates message search results into session-level summaries (count, latest timestamp, snippet) for the History view.
- Session index cleanup: backend now removes BM25 index files and index metadata when sessions are deleted (reduces disk bloat).
- Sidebar & UI polish:
  - Removed legacy "Message Search (BM25)" sidebar item and unified History link to `/history`.
  - Added keyboard shortcut to toggle the sidebar (Ctrl+B).

### Changed / Improved

- Message persistence plan and tooling:
  - Added design and initial server/client commands for message pagination and upsert (Tauri commands: `messages_get_page`, `messages_upsert`, etc.).
  - Frontend `SessionHistoryContext` prepared to switch from IndexedDB optimistic persistence to backend SQLite persistence.
- BM25 search architecture documented and partially implemented:
  - Index metadata schema (`message_index_meta`) introduced to track index_path, last_indexed_at and doc_count.
  - Background reindex worker design (dirty tracking, incremental reindex) included.
- Playbook / UIResource improvements: multipart responses (text + UIResource) are used for richer tool results (playbook lists, interactive UI).

### Fixed

- Removed the unused `MessageSearch.tsx` and cleaned up related imports.
- Linting/formatting and build fixes discovered during validation.

### Notes & Next Steps

- Run the full validation pipeline before publishing: `pnpm refactor:validate` (lint, format, rust checks, build, dead-code).
- Remaining work planned for 0.1.x:
  - Finalize SQLite message persistence and production-safe migration from IndexedDB.
  - Implement `messages_search` Tauri command and BM25 index manager (load/save/search).
  - Add tests for message CRUD, search, and index persistence.
  - Consider adding a compact release note/summary to the top of `README.md`.

### Binary Checksums (SHA256)

For verifying download integrity:

- `LibrAgent_0.1.1_amd64.deb`: `44f55aeff87b755fa364119a9249486520731b041a4c980793c9dceca8efa73e`
- `LibrAgent-0.1.1-1.x86_64.rpm`: `c007bb2931a074eeb865b9dd50d3b1e4173354ea0ed57a911f7ada30fb02c00a`
- `LibrAgent_0.1.1_amd64.AppImage`: `48c0b415297d5f8bf0a0339669758842afc65c9547f9fdf3f9f0cc1121c0d853`

### Reference

- See `docs/history/*` and `docs/sprints/*` for implementation notes, design decisions, and code pointers used to prepare this release.
