# Browser Error Handling: Structured Type System Architecture

## Overview

This document describes the structured error handling system designed to replace string-matching based error handling in browser tools.
**Status**:

- ✅ **Rust Backend**: Fully Implemented (`src-tauri/src/services/browser_error.rs`)
- 🚧 **TypeScript Frontend**: Pending Implementation (Design Phase)

## Architecture Changes

### 1. Rust Backend (`src-tauri/`)

#### Implemented File: `src-tauri/src/services/browser_error.rs`

Structured error types are defined to provide type safety:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "code", content = "context")]
pub enum BrowserError {
    SessionNotFound { session_id: String },
    SessionClosed { session_id: String },
    WindowNotFound { session_id: String },
    ElementNotFound { selector: String, session_id: String },
    ElementNotInteractable { selector: String, session_id: String },
    NavigationFailed { url: String, reason: String, session_id: String },
    ScriptExecutionFailed { reason: String, session_id: String },
    Timeout { operation: String, duration_ms: u64, session_id: String },
    LockFailed { reason: String },
    InvalidParameter { parameter: String, reason: String },
    Unknown { message: String },
}
```

**Key Features:**

- `#[serde(tag = "code", content = "context")]`: Ensures structured JSON output.
- `From<BrowserError> for String`: Automatically converts to JSON string when returning `Result<String, String>`, maintaining backward compatibility with signatures expecting strings.

#### Verified Implementation

The `BrowserError` enum is actively used in `interactive_browser_server.rs` (verified via codebase analysis).

### 2. TypeScript Frontend (Proposed)

> [!NOTE]
> The following frontend implementation is **DESIGN ONLY**. It has not yet been implemented in the codebase.
> Current frontend validation relies on legacy string matching.

#### Proposed Design: `browser-error.ts`

TypeScript definitions should mirror the Rust backend:

```typescript
export enum BrowserErrorCode {
  SESSION_NOT_FOUND = 'SESSION_NOT_FOUND',
  SESSION_CLOSED = 'SESSION_CLOSED',
  WINDOW_NOT_FOUND = 'WINDOW_NOT_FOUND',
  ELEMENT_NOT_FOUND = 'ELEMENT_NOT_FOUND',
  // ... matching all Rust variants
}

export type BrowserError =
  | SessionNotFoundError
  | SessionClosedError
  | WindowNotFoundError;
// ... union type
```

**Planned Utilities:**

- `isBrowserError(error)`: Type guard.
- `parseBrowserError(error)`: Logic to parse the JSON string returned by backend.
- `getBrowserErrorMessage(error)`: User-friendly message generator.

## Error Serialization Format

### Rust → TypeScript JSON Payload

```json
{
  "code": "ELEMENT_NOT_FOUND",
  "context": {
    "selector": ".button",
    "session_id": "abc-123"
  }
}
```

## Benefits

1.  **Type Safety**: Eliminates fragile string parsing.
2.  **Structured Context**: Provides actionable data (selectors, session IDs) separately from messages.
3.  **Extensibility**: Adding new errors in Rust automatically propagates structure (requires TS update).

## Migration Plan (Frontend)

To complete the implementation, the following steps are needed:

1.  Create `src/features/tools/browser-tools/browser-error.ts` (or similar path).
2.  Implement `parseBrowserError` to detect if an error string is JSON.
3.  Update tool execution logic to use structured error handling.

## Conclusion

The backend foundation is solid (`browser_error.rs`), but the frontend integration is currently missing. The system currently relies on the `From<BrowserError> for String` fallback which effectively passes JSON strings to the frontend, but the frontend treats them as opaque error messages.
