# UI/UX Audit: Chat & AI Agent Interface — Gaps & Refactoring Plan

## Summary

A comprehensive audit of the Chat and AI Agent interface reveals several areas where the current implementation falls short of recommended UX patterns. While many improvements are already in place, three gaps need action and several low-hanging fruit exist.

---

## Audit Findings

### Already Fixed ✅ (No action needed)

| Area                  | Finding                                                                     |
| --------------------- | --------------------------------------------------------------------------- |
| **Input Composition** | Height bounded to 96px via `useTextareaAutosize`                            |
| **Input Composition** | Typing unblocked during file uploads — only submit is gated                 |
| **Input Composition** | Non-blocking `toast.error()` used everywhere — zero `window.alert()`        |
| **Input Composition** | All icon buttons wrapped in `<Tooltip>`                                     |
| **Message Rendering** | Full auto-scroll bottom-pinning engine with FAB button                      |
| **Thinking Traces**   | Visual separation via `text-xs opacity-50 italic` + distinct container      |
| **Tool Calls**        | Consecutive tool grouping with collapsible container                        |
| **Tool Calls**        | Status indicators (spinner / green / red / amber)                           |
| **Safety Approvals**  | Arguments in code-styled container with scroll area                         |
| **Modals**            | `DialogDescription` present in both `AgentToolsModal` and `SkillsListModal` |

### Gaps Requiring Action ⚠️

| #     | Area              | Gap                                                                                       | Impact |
| ----- | ----------------- | ----------------------------------------------------------------------------------------- | ------ |
| **1** | Message Rendering | No markdown stream preprocessing — unclosed fences show broken rendering during streaming | Medium |
| **2** | Thinking Traces   | Always expanded, no collapse/expand toggle                                                | Low    |
| **3** | Safety Approvals  | No keyboard shortcuts (`Enter` to approve, `Esc` to deny)                                 | Low    |

### Styling Fixes 🎨

| #     | Area                  | Issue                                                            | Fix         |
| ----- | --------------------- | ---------------------------------------------------------------- | ----------- |
| **4** | PendingApprovalWidget | `text-[10px]` on two `<Badge>`s                                  | → `text-xs` |
| **5** | AgentToolsModal       | `text-[11px]` on filter pills, `text-[10px]` on tool type badges | → `text-xs` |

---

## Refactoring Plan

### Priority 1 — Quick Wins (low risk, ~20 min total)

#### 4. Standardize badge typography

Replace all `text-[10px]` and `text-[11px]` with `text-xs` in:

- `src/features/agent/components/PendingApprovalWidget.tsx` (2 badges)
- `src/features/agent/components/AgentToolsModal.tsx` (filter pills + tool type badges)

#### 3. Keyboard shortcuts for approval widget

In `PendingApprovalWidget`, add:

- `Enter` → approve current/highest-priority pending approval
- `Esc` → deny current/highest-priority pending approval

Focus management: first approval card gets focus ring on mount.

### Priority 2 — UX Quality (medium effort, ~1.5–2 hrs total)

#### 2. Collapsible ThinkingBubble

In `ThinkingBubble.tsx`:

- Add `isExpanded` state (default `false`)
- Add expand/collapse toggle button next to the "Thinking Process" label
- When collapsed, show only a truncated preview (first ~80 chars) + "Expand" button
- When expanded, show full content with `max-h-32 overflow-y-auto`
- Respect `followChatScroll` — auto-pin to bottom only when expanded

#### 1. Stream markdown preprocessing

New hook: `useStreamMarkdownPreprocess(content, isStreaming)`:

- During streaming: auto-close unclosed markdown fences (` ``` ` → ` ```\n`), unclosed bold/italic markers, incomplete HTML tags
- When streaming completes: restore original content (let the real generation take over)
- Prevents layout reflows caused by broken markdown rendering mid-stream

---

## Verification

1. **Badge typography**: Visual scan — all badges use consistent `text-xs` sizing
2. **Keyboard shortcuts**: Press `Enter` to approve, `Esc` to deny — no mouse needed
3. **Thinking collapse**: Default collapsed, expand reveals full trace
4. **Stream preprocessing**: Streaming an unclosed ` ```ts ` block should not cause layout shifts — block renders complete until generation finishes

Run `pnpm refactor:validate` after all changes.
