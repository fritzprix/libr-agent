# 🧹 Refactor: Eliminate Code Over-Bloat Across `src/`

## Overview

The issue is real, but the repository has moved since the first draft.

Current `src/` audit shows:

- **84,051** total TypeScript / TSX lines
- **25** non-test application files above **500** lines
- **12** current priority runtime files totaling **8,740** lines

The problem is not just file length. Several of the largest files mix UI, orchestration, state management, parsing, retry logic, and transport concerns in a single module. That raises change risk, slows comprehension, and makes behavioral refactors harder than they should be.

---

## Current Priority Targets

| #   | File                                                            | Lines | Category           | Why it is a good refactor target                                                  |
| --- | --------------------------------------------------------------- | ----: | ------------------ | --------------------------------------------------------------------------------- |
| 1   | `src/features/knowledge/KnowledgePage.tsx`                      | 1,003 | Feature page       | Page shell, graph preview, list card, detail dialog, data loading, delete flow    |
| 2   | `src/context/AgentSessionListContext.tsx`                       |   832 | State provider     | Session loading, metadata parsing, CRUD, bookmark/view state, event handling      |
| 3   | `src/features/settings/SettingsPage.tsx`                        |   800 | Feature page       | Settings shell, dirty tracking, save/discard flow, restart/reset flow, tab wiring |
| 4   | `src/lib/ai-service/ollama-core.ts`                             |   732 | AI service core    | Conversion, stream processing, tool support logic, message normalization          |
| 5   | `src/context/AgentChatContext.tsx`                              |   729 | State provider     | Provider state plus message transform / supersession / queue helpers              |
| 6   | `src/context/llm/useExecuteCompletion.ts`                       |   713 | LLM hook           | Request lifecycle, streaming assembly, throttled updates, usage tracking          |
| 7   | `src/features/agent/context/AgentResourceAttachmentContext.tsx` |   676 | State provider     | File loading, conversion, workspace sync, inline media, commit flow               |
| 8   | `src/lib/logger.ts`                                             |   676 | Utility            | Queueing, IPC batching, config, log file management, logger facade                |
| 9   | `src/lib/ai-service/anthropic.ts`                               |   667 | AI service         | Provider implementation with cache, streaming, context injection                  |
| 10  | `src/context/llm/useLLMListener.ts`                             |   638 | LLM hook           | Event bridge, retry/backoff, fallback logic, Rust handoff, compaction             |
| 11  | `src/features/agent/hooks/useAgentDraftChat.ts`                 |   638 | Draft/session hook | Assistant bootstrap, drag-and-drop, attachment prep, session creation             |
| 12  | `src/lib/ai-service/openai.ts`                                  |   636 | AI service         | Provider implementation with cache, streaming, context injection                  |

### Important scope notes

- `src/components/ui/sidebar.tsx` is still large, but it is a shadcn/ui-derived file and is **not** a priority refactor target.
- `SettingsPage.tsx` is still oversized, but the old claim that it contains “11+ inline tabs” is no longer true. The tab bodies are already split; the remaining bloat is orchestration.
- `KnowledgeCard.tsx` and `KnowledgeDetailDialog.tsx` do **not** exist as standalone files right now. Those concerns are currently inlined inside `KnowledgePage.tsx`.
- LLM service duplication exists, but it is weaker than the original issue claimed because `openai.ts` and `anthropic.ts` already share `BaseAIService` and provider-specific helper modules.

---

## Phase 0 — Rebaseline the issue and docs

Before runtime refactors, update the issue text and architecture docs so they match the current repository:

- current file paths
- current line counts
- current refactor targets
- current feature boundaries

This phase is documentation-only, but it prevents the rest of the work from being aimed at stale files and stale assumptions.

---

## Phase 1 — Extract pure helpers from oversized providers / hooks

### 1. `AgentChatContext.tsx`

**Problem:** Provider state is mixed with message transformation, streaming supersession checks, and queue-related helper logic.

**Likely split:**

```text
src/context/AgentChatContext.tsx
src/lib/message-utils.ts
src/models/service-context.ts
src/hooks/usePendingMessageQueue.ts
```

### 2. `useExecuteCompletion.ts` + `useLLMListener.ts`

**Problem:** These two hooks both deal with LLM streaming lifecycle, retry/fallback behavior, message shaping, and UI synchronization.

**Likely split:**

```text
src/context/llm/useExecuteCompletion.ts
src/context/llm/useLLMListener.ts
src/context/llm/stream-state.ts
src/context/llm/retry-policy.ts
src/context/llm/error-normalization.ts
```

### 3. `AgentSessionListContext.tsx`

**Problem:** Session list state, metadata parsing, CRUD logic, and local session transforms are bundled together.

**Likely split:**

```text
src/context/AgentSessionListContext.tsx
src/lib/session-metadata.ts
src/lib/session-list-transforms.ts
```

---

## Phase 2 — Split oversized UI pages

### 4. `KnowledgePage.tsx`

**Problem:** The page still contains:

- SVG graph layout logic
- graph preview rendering
- list item card rendering
- detail dialog rendering
- list/detail fetch orchestration
- delete flow

**Likely split:**

```text
src/features/knowledge/KnowledgePage.tsx
src/features/knowledge/components/KnowledgeGraphPreview.tsx
src/features/knowledge/components/KnowledgeListItemCard.tsx
src/features/knowledge/components/KnowledgeDetailDialog.tsx
src/features/knowledge/utils/layoutGraphNodes.ts
```

### 5. `SettingsPage.tsx`

**Problem:** The page shell still owns too much orchestration:

- dirty-state computation
- save/discard/leave flow
- restart/reset/session deletion flow
- tab navigation wiring

**Important:** This is **not** a “split inline tabs” task anymore.

**Likely split:**

```text
src/features/settings/SettingsPage.tsx
src/features/settings/hooks/useSettingsPageLifecycle.ts
src/features/settings/hooks/useSettingsDirtyState.ts
src/features/settings/hooks/useSettingsActions.ts
```

---

## Phase 3 — Split attachment and draft session flows

### 6. `AgentResourceAttachmentContext.tsx`

**Problem:** Fetching, blob/url conversion, workspace sync, inline media handling, and commit logic all live in one provider.

**Likely split:**

```text
src/features/agent/context/AgentResourceAttachmentContext.tsx
src/features/agent/lib/attachment-loading.ts
src/features/agent/lib/attachment-conversion.ts
src/features/agent/lib/attachment-commit.ts
```

### 7. `useAgentDraftChat.ts`

**Problem:** Draft UI state, drag-and-drop, assistant loading, session bootstrap, and attachment preprocessing are bundled into one giant hook.

**Likely split:**

```text
src/features/agent/hooks/useAgentDraftChat.ts
src/features/agent/hooks/useDraftAssistant.ts
src/features/agent/hooks/useDraftAttachments.ts
src/features/agent/hooks/useDraftSessionCreation.ts
```

---

## Phase 4 — Split `logger.ts`

### 8. `src/lib/logger.ts`

**Problem:** Queueing, batching, config, file-management helpers, formatting, and context logger creation are all in one module.

**Constraint:** Preserve the existing public API shape for:

- `getLogger()`
- `log`
- `logUtils`

**Likely split:**

```text
src/lib/logger/index.ts
src/lib/logger/queue.ts
src/lib/logger/config.ts
src/lib/logger/files.ts
src/lib/logger/core.ts
```

---

## Phase 5 — Reassess LLM service cleanup

### 9. `openai.ts`, `anthropic.ts`, `ollama-core.ts`

**Problem:** There is still cleanup value here, but the original issue overstated the duplication.

**Rule:** Do **not** start here. Reassess only after the higher-value context, hook, page, and logger refactors land.

Potential follow-up work:

- extract remaining provider cache helpers
- reduce repeated model-list caching patterns
- isolate streaming assembly helpers where provider logic is still mixed with transport handling

---

## Suggested execution order

1. Phase 0 — Rebaseline docs and issue text
2. Phase 1 — Extract helpers from contexts / hooks
3. Phase 2 — Split `KnowledgePage.tsx`
4. Phase 3 — Split attachment and draft flows
5. Phase 4 — Split `logger.ts`
6. Phase 2 follow-up — Reassess `SettingsPage.tsx`
7. Phase 5 — Reassess provider cleanup

---

## Validation

After each completed phase, run:

```bash
pnpm refactor:validate
```

This work is structural refactoring only. Public behavior, component contracts, and existing backend interactions must stay intact.
