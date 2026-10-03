# Session Export Formats & Architecture

This guide explains the session export subsystem in LibrAgent (`src-tauri/src/session_export/`), which provides trajectory exports for analysis, evaluation benchmarks, and human sharing.

---

## 1. Overview

LibrAgent sessions represent complex agent conversations comprising user requests, LLM thoughts, multi-step tool calls, and tool execution results.
To support both automated evaluation harnesses (e.g., [Harbor](https://github.com/avast/harbor), Terminal-Bench) and human post-mortem review, LibrAgent supports two export formats:

1. **ATIF (Agent Tool Interaction Format v1.7)**: Machine-readable JSON trajectory schema for benchmark harnesses and trajectory analyzers.
2. **Markdown (`.md`)**: Formatted, human-readable transcript with conversation context, tool arguments, and results.

---

## 2. Supported Formats

### 1. ATIF (Agent Tool Interaction Format v1.7)

File naming convention: `{sanitized_session_name}_trajectory.json`

ATIF standardizes agent trajectory data across different LLM benchmarks. The LibrAgent ATIF exporter conforms to `ATIF-v1.7`:

```json
{
  "schema_version": "ATIF-v1.7",
  "session_id": "session-42f8c0...",
  "agent": {
    "name": "LibrAgent",
    "version": "0.9.x",
    "model_name": "anthropic/claude-3-7-sonnet"
  },
  "steps": [
    {
      "step_id": 1,
      "source": "user",
      "message": "Analyze the codebase and list all routes."
    },
    {
      "step_id": 2,
      "source": "agent",
      "message": "I will inspect the route definition files.",
      "reasoning_content": "Looking for route definitions in src/...",
      "tool_calls": [
        {
          "tool_call_id": "call_123",
          "function_name": "workspace__searchFiles",
          "arguments": { "pattern": "Route" }
        }
      ]
    },
    {
      "step_id": 3,
      "source": "environment",
      "message": "",
      "observation": {
        "tool_call_id": "call_123",
        "content": "[{\"path\": \"src/routes/index.tsx\"}]"
      }
    }
  ]
}
```

### 2. Markdown (`.md`)

File naming convention: `{sanitized_session_name}.md`

Markdown export produces a clean, readable log:
- User messages rendered as clear section headings (`## User`).
- Agent reasoning/thinking rendered in expandable or blockquoted sections.
- Tool invocations and responses formatted in syntax-highlighted code blocks.
- Media links and assistant citations preserved.

---

## 3. Scaffolding & Synthetic Message Filtering

During normal execution, LibrAgent generates internal scaffolding messages for context compaction, state recovery, and UI rendering.
These synthetic messages are essential for runtime stability but should **not** appear in benchmark trajectories or exported conversation transcripts.

The export pipeline filters messages through `is_excluded_from_session_analysis_export()`:

| Filtered Message Type | Reason for Exclusion |
|---|---|
| `is_streaming == Some(true)` | Incomplete, transient streaming chunks |
| `is_compact_summary()` | Internal LLM summary generated during context compaction |
| `is_compaction_instruction()` | System prompt instructions guiding compaction |
| `is_recovery_message()` | Host recovery scaffolds generated during connection retries |
| `is_internal_synthetic_user_message()` | Synthetic prompts injected by harness or subagent coordinator |
| `is_request_layout_scaffolding_message()` | UI layout hint messages |
| `is_compaction_overlay_message()` | Temporary UI overlays |

---

## 4. Streaming & Memory Efficiency

To support long-running agent workflows (sessions with thousands of messages), the exporter avoids loading the entire SQLite table into memory at once:

1. **Chunked SQLite Pagination**: Messages are paged in causal order (`rowid ASC`) in batches of 500 (`EXPORT_MESSAGE_PAGE_SIZE = 500`).
2. **Incremental Trajectory Building**:
   - `AtifTrajectoryBuilder::push()` processes and aggregates steps incrementally.
   - `markdown::append_message()` writes incrementally to an output buffer.
3. **No Redundant Deserialization**: Synthetic scaffolding is filtered during pagination before constructing the full DOM.

---

## 5. Filename & Stem Sanitization

Exported filenames are derived from the session title or session ID, passed through `sanitize_export_stem()`:
- Illegal path characters (`/`, `\`, `:`, `<daemon>`, `"`, `|`, `?`, `*`, control characters) are replaced with underscores (`_`).
- Leading/trailing whitespace and periods are stripped.
- Reserved Windows device names (`CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9`) are automatically prefixed (e.g., `_CON`).
- Maximum filename length is capped at 180 characters (`MAX_EXPORT_STEM_CHARS`).

---

## 6. Tauri Command & Invocation

The export subsystem is triggered from the frontend or test harnesses via the `export_session_file` Tauri command:

```typescript
import { invoke } from '@tauri-apps/api/core';

// Export as ATIF v1.7 trajectory
const atifPath = await invoke<string>('export_session_file', {
  sessionId: 'session-1234',
  format: 'atif',
});

// Export as Markdown transcript
const mdPath = await invoke<string>('export_session_file', {
  sessionId: 'session-1234',
  format: 'markdown',
});
```

When invoked from the desktop UI, the command opens the native file save dialog with default file names pre-populated.
