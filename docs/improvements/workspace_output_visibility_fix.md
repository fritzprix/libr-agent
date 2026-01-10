# Workspace Output Visibility Fix

## Problem

The `readProcessOutput` and `pollProcess` tools in the Workspace MCP server were returning process output only in the `structured_content` JSON field. The `content` (text) field, which is visible to the LLM, only contained a summary message (e.g., "Read 20 lines").

This caused the Agent to be unable to see the actual output of commands it executed, leading to failure in tasks requiring output analysis.

## Changes Implemented

### 1. `handle_read_process_output`

Modified in `src-tauri/src/mcp/builtin/workspace/mod.rs`.

- Before: Returned "Read N lines" text.
- After: Returns "Read N lines from stdout tail:\n\n[Actual Output Content]".
- This ensures the LLM sees the content it requested.

### 2. `handle_poll_process`

Modified in `src-tauri/src/mcp/builtin/workspace/mod.rs`.

- Before: Returned status only (e.g., "Process starting").
- After: If `tail` argument is provided and output exists, appends "\n\n--- Output (last N lines) ---\n[Output Content]" to the status message.
- This allows quick status checks to also provide context without a separate read call.

## Verification

Created a new test suite: `src-tauri/src/mcp/builtin/workspace/test_output_visibility.rs`.

- `test_read_process_output_visibility`: Executes `echo`, waits, reads output, and asserts that the text prompt contains the echoed string.
- `test_poll_process_tail_visibility`: Executes `echo` (async), waits, polls with `tail`, and asserts that the text prompt contains the echoed string.

Both tests passed successfully.
