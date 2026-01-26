# Workspace MCP Tools Specification

## Overview

This document specifies the behavior and standards for the Workspace MCP tools, ensuring compliance with the "Zero Trust / Dual Channel" architecture.

## Tool Definitions

### `writeFile` (Canonical Name)

**Old Name:** `createFile` (Deprecated/Removed)

#### Purpose

Atomic file creation and overwriting with strict safety controls.

#### Schema

```json
{
  "name": "writeFile",
  "description": "Create a new file or overwrite an existing one...",
  "inputSchema": {
    "type": "object",
    "properties": {
      "path": {
        "type": "string",
        "description": "Relative path from workspace root"
      },
      "content": {
        "type": "string",
        "description": "Content to write"
      },
      "overwrite": {
        "type": "boolean",
        "description": "Allow overwriting? (default: false)"
      }
    },
    "required": ["path", "content"]
  }
}
```

#### Behavior Matrix

| File State     | `overwrite` | Outcome             | Response Data                    |
| :------------- | :---------- | :------------------ | :------------------------------- |
| Does not exist | `false`     | **SUCCESS**         | Path, Size, Lines                |
| Exists         | `false`     | **FAILURE** (Error) | Guidance to set `overwrite=true` |
| Exists         | `true`      | **SUCCESS**         | Path, Size, Lines, **Diff**      |

#### Dual Channel Response

- **Text (Narrative):**
  - "✅ File Created: `src/main.rs` (12 lines)"
  - OR "✅ File Overwritten: `src/main.rs` (12 lines) \n `diff ... `"
  - AND "Next Steps: Use readFile to verify..."
- **Structure (Data):**
  - `{ "path": "...", "bytes": 1024, "overwritten": true }`

#### BP Compliance

- **Zero Trust:** Path validated against sandbox before any IO.
- **Canonical Naming:** Only `writeFile` exists.
- **Error Guidance:**
  - If file exists + `overwrite: false`: suggest `overwrite: true` or `editFile`.
  - If permission denied: suggest `listDirectory`.

## Legacy/Related Tools

- `editFile`: Use for targeted replacements (safer than overwriting).
- `deleteFile`: Destructive removal.
