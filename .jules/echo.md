## 2024-05-24 - [Duplicate Throttle Implementation]

**Pattern:** Found a `useThrottleHook` implementation embedded inside `useDebounce.ts` that was completely unused and duplicated the intent of `useThrottle.ts`.
**Action:** Removed the unused `useThrottleHook` from `useDebounce.ts` to enforce Single Source of Truth (`src/hooks/useThrottle.ts`).

## 2024-05-24 - [Message Repository Serialization Duplication]

**Pattern:** Repeated complex JSON serialization and `ActiveModel` construction logic in `insert` and `insert_many` methods of `SqliteMessageRepository`.
**Action:** Extracted `message_to_active_model` and `serialize_optional_json` helper methods to centralize the logic and reduce duplication.

## 2026-01-16 - [Message Document Conversion Duplication]

**Pattern:** Identical manual field mapping from `entity::message::Model` to `MessageDocument` in both search indexing and global search command handlers.
**Action:** Implemented `From<Model>` trait for `MessageDocument` to centralize the conversion logic and replaced manual mapping with `MessageDocument::from` or `.into()`.

## 2026-01-18 - [Duplicated Labeled Input Logic]

**Pattern:** Identical JSX structure (container, label, error message) duplicated in `InputWithLabel.tsx` and `TextareaWithLabel.tsx`.
**Action:** Extracted `FieldWrapper` component to centralize the layout and styling logic for labeled form fields.

## 2026-05-24 - [Duplicate Session Filtering Logic]

**Pattern:** Redundant session filtering logic (matching against name, ID, assistant name/description) in `SessionList.tsx` and `AgentChatStartView.tsx`.
**Action:** Extracted `filterSessions` utility in `src/lib/session-utils.ts` to centralize filtering logic and ensure consistent search behavior across views.

## 2026-01-20 - [JSON Repository Serialization]

**Pattern:** Repeated `serde_json::from_str` and `unwrap_or_default` calls in `MessageRepository::model_to_message` and private helper method duplication for serialization.
**Action:** Extracted `to_json_option`, `from_json_option`, and `from_json_or_default` into `src-tauri/src/utils/json.rs` to centralize serialization logic and reduce boilerplate.

## 2026-05-24 - [Repository Trait Cleanup and Conversion Duplication]

**Pattern:** Vestigial `create_table` no-op method duplicated in repository traits, and repeated manual `SessionMetadata` mapping in `SessionRepository`.
**Action:** Removed `create_table` from `MessageRepository` and `SessionRepository` traits, and implemented `TryFrom<session::Model>` for `SessionMetadata` to centralize conversion logic.

## 2026-05-25 - [Message Search Indexing Duplication]

**Pattern:** Identical logic for fetching message models, converting them to `MessageDocument` via `from()`, and populating `MessageSearchEngine` in three different places (`messages_commands.rs` (x2) and `background_worker.rs`).
**Action:** Extracted `MessageSearchEngine::build_from_models` factory method to centralize this creation logic.

## 2026-05-26 - [Message Repository Query Duplication]

**Pattern:** Identical query construction for fetching `Message` (domain object) and `message::Model` (database entity) in `MessageRepository`, and repeated `OnConflict` logic in insert methods.
**Action:** Refactored getter methods to chain calls (reusing model retrieval) and extracted `get_upsert_on_conflict` helper for consistent upsert logic.

## 2026-01-27 - [Chat Message Creation Duplication]

**Pattern:** Identical object literal structure and initialization logic (e.g., `createId`, `threadId` fallback) repeated across `createSystemMessage`, `createUserMessage`, and `createToolMessage`.
**Action:** Extracted `createBaseMessage` helper to centralize message instantiation and reduce structural repetition.

## 2026-06-05 - [Search Pagination and Indexing Duplication]

**Pattern:** Repeated manual pagination (vector slicing) and index building logic in `messages_commands.rs` and `search/service.rs`.
**Action:** Extracted `paginate_in_memory` into `utils/pagination.rs` and `build_global_temporary_index` into `search/service.rs` to centralize this logic.

## 2026-06-06 - [Workspace Path Security Duplication]

**Pattern:** Repeated path resolution, canonicalization, and security verification checks (e.g., `canonicalize`, `starts_with`) in `workspace_commands.rs` and `download_commands.rs`, including a security vulnerability where one check bypassed canonicalization.
**Action:** Created `src-tauri/src/utils/security.rs` with `resolve_secure_path` to centralize secure path resolution logic, fix the vulnerability, and ensure consistent usage of `tokio::fs` for non-blocking I/O.
