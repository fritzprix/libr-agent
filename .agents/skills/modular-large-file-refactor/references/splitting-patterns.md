# Splitting Patterns

## Table of contents

1. Responsibility mapping
2. Extraction checklist
3. React and TypeScript patterns
4. Rust patterns
5. Naming rules
6. Smells that mean "stop splitting"

## 1. Responsibility mapping

Before moving code, map each section of the large file into one of these buckets:

- **Facade**: exported component, command, or public function
- **Pure logic**: parsing, normalization, validation, formatting, diffing
- **State**: reducers, hooks, local stores, memoized selectors
- **Effects**: network calls, filesystem calls, subscriptions, event wiring
- **Rendering**: JSX/TSX markup or display-specific transforms
- **Contracts**: types, schemas, enums, constants

If a bucket dominates the file, extract that bucket first.

## 2. Extraction checklist

Use this order unless the codebase gives you a stronger pattern:

1. Extract shared constants and types.
2. Extract pure helper functions.
3. Extract repeated or bulky render sections.
4. Extract stateful logic.
5. Leave orchestration in the original file.

Quick checks after each extraction:

- Did the public export stay stable?
- Did imports become clearer instead of noisier?
- Did the new file get a precise name?
- Did the extraction remove branching from the entrypoint?

## 3. React and TypeScript patterns

### Large component split

Use this when a component mixes rendering, state, backend calls, and formatting.

Suggested shape:

- `FeaturePanel.tsx` - facade and top-level composition
- `FeaturePanelSection.tsx` - meaningful UI subsection
- `useFeaturePanel.ts` - stateful behavior and event handlers
- `feature-panel-utils.ts` - pure transforms only
- `feature-panel-types.ts` - shared types if they are bulky or reused

Keep the facade responsible for:

- deciding what renders
- wiring hooks to components
- passing already-shaped props downward

Move out of the facade:

- data shaping with no side effects
- repeated event logic
- bulky conditional rendering blocks

### Large hook split

Use this when one hook owns fetching, derivation, side effects, and output formatting.

Suggested shape:

- `useFeatureState.ts` - state machine and handlers
- `feature-selectors.ts` - pure derivations
- `feature-api.ts` - backend or HTTP calls

Keep the main hook as the stable surface that combines those pieces.

### Service split

Use this when a service file mixes transport, validation, and response shaping.

Suggested shape:

- `service.ts` - public functions
- `service-client.ts` - transport only
- `service-validators.ts` - schema/type guards
- `service-mappers.ts` - conversion between transport and app shapes

Do not let mapper files call transport directly.

## 4. Rust patterns

### Command handler split

Use this when a Tauri command or MCP tool handler does too much in one file.

Suggested shape:

- `mod.rs` or existing command file - public command/tool entrypoint
- `validation.rs` - input validation and guards
- `service.rs` - orchestration and core operation
- `mapping.rs` or `response.rs` - output shaping
- `repository.rs` only if persistence logic is substantial and reusable

Keep the public command thin:

- deserialize input
- call service layer
- return typed result

### Large module split

Use sibling modules under the same directory when the file currently mixes unrelated concerns.

Prefer:

- one facade module
- a few internal modules with obvious names

Avoid:

- deep nesting that forces readers through multiple `mod.rs` layers
- scattering tiny single-function files everywhere

## 5. Naming rules

- Name files after the responsibility, not the source file they came from.
- Prefer `validation.ts` over `big-file-part-2.ts`.
- Prefer `browser_context.rs` over `helpers.rs`.
- If the module is internal-only, keep the name specific enough that a grep result is self-explanatory.

## 6. Smells that mean "stop splitting"

Stop and keep code together when:

- every new file needs half the original imports
- the reader must jump across several files to follow one happy path
- the split creates circular dependencies
- the extracted file has no stable reason to exist beyond shrinking the original
- the top-level file becomes a thin trampoline with no readable story
