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
