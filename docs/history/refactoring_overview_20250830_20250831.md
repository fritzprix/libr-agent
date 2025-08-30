### Comprehensive Overview of Refactoring Plans

This overview analyzes the provided refactoring plans in chronological order based on their filenames (YYYYMMDD_HHMM format). The analysis covers key elements from each plan: purpose, current issues, proposed changes, and resolution criteria. Where overlaps or evolutions in plans are detected (e.g., multiple plans targeting similar components like Content Store or ResourceAttachmentContext), I identify redundancies and prioritize the most recent plan as the "final" evolution, assuming later plans refine or supersede earlier ones based on iterative development. This ensures the overview reflects a cohesive progression without duplicating effort.

Overarching themes across all plans include:

- **Improving modularity and maintainability**: Splitting large files, separating concerns, and reducing code duplication.
- **Enhancing type safety and consistency**: Especially in MCP tools and response handling.
- **Fixing UX and functional inconsistencies**: In file attachments, error messaging, and state management.
- **Performance and stability optimizations**: For search engines, databases, and multi-session handling.
- **No major conflicts**: Plans build on each other; e.g., Content Store improvements evolve from bug fixes to modularity, and file attachment plans progress from UI fixes to full system refactoring.

#### 1. refactoring_20250830_1400.md: MCP Tool Response Delivery Path Type Safety Enhancement

- **Purpose**: Strengthen type safety and consistency in the MCP tool response delivery path (module → worker → proxy → provider → UI) by enforcing `MCPResponse` as the return type for all tool functions, reducing runtime errors and eliminating type casting.
- **Current Issues**: Inconsistent return types (e.g., `Promise<unknown>` or specific types) across modules; lack of generics in proxy; inconsistent handling in provider (e.g., converting to text or JSON.stringify).
- **Proposed Changes**:
  - Update tool functions in modules like `planning-server.ts` and `content-store.ts` to return `Promise<MCPResponse>` using a `normalizeToolResult` wrapper.
  - Simplify worker and proxy to directly return `MCPResponse`.
  - Remove generics from proxy and standardize provider to pass `MCPResponse` untouched.
  - Log changes in `./docs/history/refactoring_{yyyyMMdd_hhmm}.md`.
- **Resolution Criteria**: All layers handle `MCPResponse` without casting; runtime errors reduced; consistent structure per MCP standards.
- **Overlap Analysis**: This plan focuses on MCP response typing. It overlaps slightly with later MCP modularity (e.g., 1410), but no direct redundancy—treat as foundational for MCP-related changes.

#### 2. refactoring_20250830_1510.md: Content Store Duplicate Content Handling Improvement

- **Purpose**: Enhance duplicate content handling in Content Store by separating indexes per `storeId`, allowing identical content across different sessions but preventing duplicates within the same `storeId` via file hashes.
- **Current Issues**: Global index management in `BM25SearchEngine` allows cross-`storeId` duplicates; no file-level duplicate checks in `addContent`; inefficient for multi-session environments.
- **Proposed Changes**:
  - Add `computeContentHash` function (browser-compatible hash calculation).
  - Modify `addContent` to check duplicates via `dbService.fileContents.findByHashAndStore` and return existing if found.
  - Strengthen `BM25SearchEngine` for per-`storeId` indexing.
  - Extend DB interface with `findByHashAndStore` and add `contentHash` field.
- **Resolution Criteria**: Duplicates prevented within `storeId`; allowed across; improved search/storage efficiency; tested via same-file scenarios.
- **Overlap Analysis**: This targets Content Store duplicates specifically. It evolves into broader Content Store improvements in later plans (2110, 2200), but focuses on hashes—integrate as a precursor, with later plans handling broader modularity.

#### 3. refactoring_20250830_1910.md: File Attachment UI Multi-File Display and State Management Improvement

- **Purpose**: Fix multi-file attachment UI issues and post-submit state initialization to ensure accurate display and consistency in file lists/chat.
- **Current Issues**: Only one file displays in UI for multiples; states not cleared post-submit; race conditions in `addFile`/`refreshSessionFiles`; incomplete `clearFiles`.
- **Proposed Changes**:
  - Separate states: `pendingFiles` (UI previews) and `sessionFiles` (server-synced).
  - Batch multi-attachments: Use `addFilesBatch` with single `refreshSessionFiles` post-all.
  - Update `Chat.tsx`: Reference `pendingFiles` for attachments; clear only `pendingFiles` post-submit.
  - Improve flow: Add to pending → server save → batch refresh → clear pending.
- **Resolution Criteria**: Multi-files display correctly; post-submit clears attachments; no race conditions; UI consistency between lists.
- **Overlap Analysis**: This UI-focused plan on file attachments overlaps heavily with later ResourceAttachmentContext refactors (1300, 1400). It introduces state separation, which is refined in 1300 (simplifying functions) and expanded in 1400 (environment handling). Prioritize 1400 as the most comprehensive/final for attachments.

#### 4. refactoring_20250830_2100.md: User Duplicate File Error Message Correction

- **Purpose**: Align error messages in file attachment system with actual behavior (session/`storeId`-specific duplicates, not global) to reduce user confusion.
- **Current Issues**: Message implies global duplicate ban, but system allows cross-session duplicates; based on SHA-256 hashes per `storeId`.
- **Proposed Changes**:
  - In `ResourceAttachmentContext.tsx` (`addFile` function), update error text from "another session... across all sessions" to "current session... within the same session".
  - Verify duplicate logic in `content-store.ts` (`findByHashAndStore`).
- **Resolution Criteria**: Messages reflect session-specific policy; tests confirm correct display; no user confusion.
- **Overlap Analysis**: Directly related to file attachments/duplicates. Builds on 1510 (Content Store duplicates) and feeds into later attachment plans (1910, 1300, 1400). No redundancy, but integrate with 1400's broader attachment refactor as the final state.

#### 5. refactoring_20250830_2110.md: Content Store Module Improvements

- **Purpose**: Boost stability/performance of Content Store via BM25 bug fixes, DB efficiency, and code structure optimizations.
- **Current Issues**: BM25 consolidate errors/missing calls; parallel race conditions; inefficient `listContent` (full fetch then filter); over-logging; tight coupling in chunking.
- **Proposed Changes**:
  - Add retry to BM25 `consolidate`; implement index locking.
  - Optimize `listContent` with DB-level `storeId` filtering.
  - Modularize chunking via strategy pattern; adjust logging levels; add input validation (Zod/Joi).
- **Resolution Criteria**: 100% BM25 success; 50%+ query time improvement; 80%+ code coverage; easier maintenance.
- **Overlap Analysis**: Expands on 1510's Content Store duplicates. Overlaps with 2200 (BM25 separation)—treat 2200 as the final modularity step, incorporating this plan's fixes.

#### 6. refactoring_20250830_2200.md: BM25 Search Engine Code Separation Refactoring Plan

- **Purpose**: Extract BM25 code from `content-store.ts` into a separate module for better modularity, maintainability, and testability; fix non-functional `initialize()` method.
- **Current Issues**: BM25 class/types mixed in large file (200+ lines); tight coupling; no real init in `initialize()`; hard to test/reuse.
- **Proposed Changes**:
  - Create `bm25/` subdir with `bm25-search-engine.ts` (class), `types.ts`, `index.ts` (exports).
  - Move BM25 implementation; add real init logic to `initialize()`.
  - Update `content-store.ts` to import from new module.
- **Resolution Criteria**: Full separation; compile/functionality preserved; independent testing/reuse possible.
- **Overlap Analysis**: Direct evolution of 2110's BM25 fixes. This is the most recent BM25-specific plan—use as final for BM25 modularity, superseding earlier Content Store aspects.

#### 7. refactoring_20250831_1300.md: ResourceAttachmentContext & Chat.tsx Refactoring Plan

- **Purpose**: Simplify file attachment state management in ResourceAttachmentContext and Chat.tsx by separating flows, removing redundancies, and using useSWR for optimization.
- **Current Issues**: Overlapping functions (`addFile`/`addFilesBatch`); complex states (`files`/`sessionFiles`/`pendingFiles`); unnecessary functions (`clearFiles`); inconsistent UX flows.
- **Proposed Changes**:
  - Split to `addPendingFiles` (UI add) and `commitPendingFiles` (server upload).
  - Use useSWR for `sessionFiles` (replace manual states/mutate for refresh).
  - Remove redundancies (e.g., `clearFiles`, `getFileById`); update Chat.tsx submit to commit pending.
  - Simplify context interface.
- **Resolution Criteria**: Simplified API/UX; code complexity reduced 50%; consistent flows.
- **Overlap Analysis**: Builds on 1910's state separation and 2100's messages. Overlaps with 1400 (same context, but 1400 adds environment handling)—treat 1400 as final for attachments, incorporating this plan's simplifications.

#### 8. refactoring_20250831_1400.md: ResourceAttachmentContext File Attachment System Refactoring Plan

- **Purpose**: Resolve HTML content injection in attachments under Vite/Tauri; clarify path/URL handling; improve Blob URL management and previews.
- **Current Issues**: Attachments get HTML instead of file content; mixed paths/URLs; memory leaks from unrevoked Blobs; environment inconsistencies.
- **Proposed Changes**:
  - Detect environment (`isTauriEnvironment`); extend interface for original paths/Files.
  - Update `addPendingFiles`/`addFileInternal` for environment-specific handling.
  - Improve previews by type (text/image); add cleanup to `convertToBlobUrl`.
  - Standardize error/recovery; optimize performance.
- **Resolution Criteria**: Real file content attached/previewed; environment compatibility; no leaks; improved UX.
- **Overlap Analysis**: Most comprehensive attachment refactor, superseding 1910, 2100, and 1300. Use as final for ResourceAttachmentContext, integrating prior state/UI fixes.

#### 9. refactoring_20250831_1410.md: Large File Modularization and Code Duplication Removal

- **Purpose**: Modularize large files (e.g., `filesystem.rs`, `Chat.tsx`, `content-store.ts`) and eliminate duplicates; ensure Worker dynamic loading compatibility.
- **Current Issues**: Oversized files with mixed responsibilities; repeated JSONSchema in Rust; inconsistent logging; bundler issues in dynamic imports.
- **Proposed Changes**:
  - Create Rust schema builder for reuse; split Content Store into submodules (parser/chunker/search).
  - Decompose frontend components (e.g., Chat.tsx hooks); use central logger in Worker.
  - Maintain default exports for dynamics.
- **Resolution Criteria**: Build/tests pass; 70%+ duplication reduced; Worker compatibility; single responsibilities.
- **Overlap Analysis**: Broadest plan, encompassing prior Content Store (2110/2200) and MCP (1400) aspects. Use as final overarching modularity, superseding specifics where overlapped (e.g., BM25 from 2200).
