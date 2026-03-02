# Skill Mention System — `@type:name` Reference Injection (SP10)

> **Spec:** `docs/specs/skill-mention-system.md`

## Overview

SP10 implements a **late-binding reference injection** system for the agent chat
input. Users type `@type:name` tokens directly in the textarea; the text is stored
in the database unchanged. The Rust LLM pipeline resolves each token and prepends
the referenced content to the user message immediately before sending to the LLM —
without modifying the stored message.

This is the first phase of the broader Chat Input Command & Reference Framework
described in the spec. The `/command` system is deferred to a future sprint.

## Architecture

### Late Binding vs. Early Binding

|                 | Early Binding (rejected) | Late Binding (implemented)            |
| --------------- | ------------------------ | ------------------------------------- |
| Resolution time | JS submit handler        | Rust, just before LLM call            |
| Storage         | Resolved content stored  | Raw `@skill:name` stored              |
| Re-runs         | Stale if skill updated   | Always uses latest content            |
| Implementation  | Frontend-only            | Rust `ReferenceRegistry` trait system |

### Data Flow

```
User types: "analyze this using @skill:docx"
         ↓  (stored as-is in DB)
DB stores: "analyze this using @skill:docx"
         ↓  (at LLM call time, in completion.rs)
Rust resolves @skill:docx → reads skill file content
         ↓
CompletionRequest user message becomes:
  "## Reference: @skill:docx\n\n<skill content>\n\n---\n\nanalyze this using @skill:docx"
         ↓  (frontend receives CompletionRequest event, calls LLM)
LLM sees skill content prepended to the message
```

The stored message and the UI display are never mutated.

## Backend Implementation

### `ReferenceResolver` Trait (`agent/references/mod.rs`)

```rust
#[async_trait]
pub trait ReferenceResolver: Send + Sync {
    /// The type name matched in `@type:arg` (e.g. "skill")
    fn type_name(&self) -> &str;
    /// Returns resolved content string, or None to silently skip
    async fn resolve(&self, arg: &str) -> Option<String>;
}
```

### `ReferenceRegistry`

Holds a list of `Box<dyn ReferenceResolver>`. `resolve_all(text)` parses
`@([\w]+):([\S]+)` from user message text, calls the matching resolver for each
match, and prepends the resolved content blocks before the original text.

```rust
pub struct ReferenceRegistry {
    resolvers: Vec<Box<dyn ReferenceResolver>>,
}
```

`build_default_registry()` wires up the built-in resolvers.

### `SkillReferenceResolver` (`agent/references/skill.rs`)

Handles `@skill:name`. Calls `get_configured_skills_directory()` and
`get_skill_content(path)` from the skills service to load the markdown file.
Returns `None` if the skill doesn't exist (silently skipped).

### Integration in LLM Pipeline (`agent/llm/completion.rs`)

`resolve_message_references()` is called on each user message before the
`CompletionRequest` is emitted to the frontend:

```rust
// In build_completion_request(), before emitting llm:completion-request
let messages = resolve_message_references(&messages, &registry).await;
```

Only user messages (`Role::User`) are processed. System/assistant messages are
passed through unchanged.

## Frontend Implementation

### `useInputToken` Hook (`features/agent/hooks/useInputToken.ts`)

A state-machine hook that drives the dropdown UX:

```
idle  ──→  typing-type  ──→  typing-arg  ──→  idle
     @              @skill:            select
```

| State         | Trigger                             | Dropdown shows                     |
| ------------- | ----------------------------------- | ---------------------------------- |
| `idle`        | —                                   | Nothing                            |
| `typing-type` | `/@([\w]*)$/` matches before cursor | Built-in token types               |
| `typing-arg`  | `/@([\w]+):([\S]*)$/` matches       | Filtered candidates (skills, etc.) |

**API:**

```ts
const {
  stage,
  typeResults,
  skillResults,
  onInputChange,
  onTypeSelect,
  onArgSelect,
  onDismiss,
} = useInputToken(skills);
```

- `onTypeSelect(typeName, value, cursor)` → inserts `@type:` into text, returns new value
- `onArgSelect(arg, value, cursor)` → inserts `@type:arg ` into text, returns new value
- Selection modifies textarea text directly — no chips, no side state

### Built-in Token Types

```ts
export const BUILTIN_TOKEN_TYPES: TokenType[] = [
  {
    name: 'skill',
    label: 'skill:',
    description: 'Inject skill documentation into context',
  },
  {
    name: 'file',
    label: 'file:',
    description: 'Inject workspace file content into context',
  },
  {
    name: 'tool',
    label: 'tool:',
    description: 'Add soft attention hint for a specific tool',
  },
];
```

`file:` and `tool:` show in the type dropdown but have no backend resolver yet
(silently no-ops). Resolvers are added by implementing `ReferenceResolver` and
registering in `build_default_registry()`.

### `InputTokenDropdown` Component (`features/agent/components/InputTokenDropdown.tsx`)

Renders the suggestion popup above the textarea (`absolute bottom-full`).

- **Mode `types`** — shows `BUILTIN_TOKEN_TYPES` with label + description
- **Mode `skills`** — shows filtered `SkillMetadata[]` with `@skill:name` preview

Keyboard navigation (`↑↓ Enter Tab Escape`) via a `capture: true` window listener
so it intercepts before the textarea's `keydown` handler.

Uses `onMouseDown` (not `onClick`) to prevent textarea blur before selection is
registered.

### `AgentChatInput` Integration

```tsx
const {
  stage,
  typeResults,
  skillResults,
  onInputChange,
  onTypeSelect,
  onArgSelect,
  onDismiss,
} = useInputToken(skills);

// In textarea onChange:
onInputChange(value, selectionStart);

// Dropdown rendered above textarea when stage !== 'idle':
{
  stage.kind !== 'idle' &&
    (typeResults.length > 0 || skillResults.length > 0) && (
      <InputTokenDropdown
        mode={
          stage.kind === 'typing-type'
            ? { kind: 'types', items: typeResults }
            : { kind: 'skills', items: skillResults }
        }
        onSelectType={(typeName) => {
          const newValue = onTypeSelect(typeName, input, cursorPos);
          setInput(newValue);
          // position cursor after @type: via requestAnimationFrame + setSelectionRange
        }}
        onSelectArg={(arg) => {
          setInput(onArgSelect(arg, input, cursorPos));
        }}
        onDismiss={onDismiss}
      />
    );
}
```

No chip UI. `@skill:docx` stays in the textarea text as-is. No `getMessagePrefix`.

## Files Changed

| File                                                         | Change                                                                           |
| ------------------------------------------------------------ | -------------------------------------------------------------------------------- |
| `src-tauri/src/agent/references/mod.rs`                      | New — `ReferenceResolver` trait, `ReferenceRegistry`, `build_default_registry()` |
| `src-tauri/src/agent/references/skill.rs`                    | New — `SkillReferenceResolver` for `@skill:name`                                 |
| `src-tauri/src/agent/mod.rs`                                 | `pub mod references`                                                             |
| `src-tauri/src/agent/llm/completion.rs`                      | `resolve_message_references()` call before emitting CompletionRequest            |
| `src/features/agent/hooks/useInputToken.ts`                  | New — state machine hook                                                         |
| `src/features/agent/components/InputTokenDropdown.tsx`       | New — suggestion dropdown                                                        |
| `src/features/agent/components/AgentChatInput.tsx`           | Rewritten to use `useInputToken` + `InputTokenDropdown`                          |
| ~~`src/features/agent/hooks/useSkillMention.ts`~~            | Deleted (replaced by `useInputToken`)                                            |
| ~~`src/features/agent/components/SkillMentionDropdown.tsx`~~ | Deleted (replaced by `InputTokenDropdown`)                                       |

## Extension Guide

### Adding a New `@type:` Resolver (Rust)

1. Create `src-tauri/src/agent/references/your_type.rs`:

```rust
pub struct YourTypeResolver;

#[async_trait]
impl ReferenceResolver for YourTypeResolver {
    fn type_name(&self) -> &str { "yourtype" }
    async fn resolve(&self, arg: &str) -> Option<String> {
        // return Some(content) or None to skip silently
    }
}
```

2. Register in `build_default_registry()` in `references/mod.rs`:

```rust
registry.register(Box::new(YourTypeResolver));
```

3. Add the frontend entry to `BUILTIN_TOKEN_TYPES` in `useInputToken.ts`
   and wire up candidate fetching in `useInputToken` if needed.

### Adding a New Arg Source (Frontend)

Add a new branch in `useInputToken` alongside the `skill` case:

```ts
const yourResults =
  stage.kind === 'typing-arg' && stage.typeName === 'yourtype'
    ? yourItems.filter(...)
    : [];
```

Then handle it in `InputTokenDropdown` as a new `mode.kind`.

## Deferred

- `@file:path` resolver (workspace file injection with path security validation)
- `@tool:name` resolver (tool hint injection)
- `/command` system (`/clear`, `/compact`, `/model`)
- Progressive search depth for `@file:` queries (VS Code Ctrl+P style)
