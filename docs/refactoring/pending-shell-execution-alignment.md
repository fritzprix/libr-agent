# Refactoring Plan: PendingShellExecution Parameter Alignment

## Issue Summary

**Problem:** Parameter naming inconsistency between tool schema and implementation

- Tool schemas (`code_tools.rs`): camelCase (`executionId`, `userInput`)
- UI postMessage (`ui.rs`): snake_case (`execution_id`, `user_input`)
- Backend handlers (`handlers.rs`): snake_case extraction

**Impact:**

- Potential confusion for external tool integrations
- Inconsistent with MCP/JSON conventions (camelCase preferred)
- Technical debt from mixed naming conventions

**Additional Issue:**

- No background cleanup for expired/abandoned `PendingShellExecution` entries
- 5-minute timeout only checked on use, not proactively cleaned

---

## Design Decision

**Chosen Approach:** Single canonical format (camelCase)

**Rationale:**

1. MCP tool schemas follow JSON conventions (camelCase)
2. Tool definitions in `code_tools.rs` already use camelCase
3. External integrations expect schema-defined parameters
4. Clean codebase without dual-key complexity

**Rejected Alternative:** Dual-key support (snake_case + camelCase)

- Adds unnecessary complexity
- Migration tool, not design goal
- Deferred burden to maintain compatibility layer

---

## Phase 1: Backend Parameter Extraction (High Priority)

### Affected Files

- `src-tauri/src/mcp/builtin/workspace/code_execution/interactive/handlers.rs`

### Changes Required

#### 1.1 Update `handle_execute_pending_shell` parameter extraction

**Current (lines ~110-125):**

```rust
let execution_id = input
    .get("execution_id")
    .and_then(|v| v.as_str())
    .ok_or_else(|| "Missing execution_id parameter".to_string())?;

let encrypted_input = input
    .get("user_input")
    .and_then(|v| v.as_str())
    .ok_or_else(|| "Missing user_input parameter".to_string())?;
```

**Proposed:**

```rust
// Primary: camelCase (matches tool schema)
let execution_id = input
    .get("executionId")
    .or_else(|| input.get("execution_id")) // Backward compatibility for 1 release
    .and_then(|v| v.as_str())
    .ok_or_else(|| "Missing executionId parameter".to_string())?;

let encrypted_input = input
    .get("userInput")
    .or_else(|| input.get("user_input")) // Backward compatibility for 1 release
    .and_then(|v| v.as_str())
    .ok_or_else(|| "Missing userInput parameter".to_string())?;
```

**Notes:**

- Keeps fallback to snake_case for one release cycle
- Error messages updated to reflect camelCase as primary
- Remove fallback in next major release (v0.5.0)

#### 1.2 Update `handle_cancel_pending_execution` parameter extraction

**Current (lines ~590-595):**

```rust
let execution_id = input
    .get("execution_id")
    .and_then(|v| v.as_str())
    .ok_or_else(|| "Missing execution_id parameter".to_string())?;
```

**Proposed:**

```rust
let execution_id = input
    .get("executionId")
    .or_else(|| input.get("execution_id")) // Backward compatibility for 1 release
    .and_then(|v| v.as_str())
    .ok_or_else(|| "Missing executionId parameter".to_string())?;
```

---

## Phase 2: UI postMessage Update (High Priority)

### Affected Files

- `src-tauri/src/mcp/builtin/workspace/code_execution/interactive/ui.rs`

### Changes Required

#### 2.1 Update JavaScript postMessage parameters

**Current (lines ~126-128):**

```javascript
window.parent.postMessage(
  {
    type: 'tool',
    payload: {
      toolName: toolName,
      params: {
        execution_id: executionId,
        user_input: encryptedInput,
      },
    },
  },
  '*',
);
```

**Proposed:**

```javascript
window.parent.postMessage(
  {
    type: 'tool',
    payload: {
      toolName: toolName,
      params: {
        executionId: executionId,
        userInput: encryptedInput,
      },
    },
  },
  '*',
);
```

**Notes:**

- Aligns with tool schema definitions
- JavaScript variable names can stay camelCase (no change needed)
- Only payload keys need updating

---

## Phase 3: Background Cleanup Implementation (Medium Priority)

### Affected Files

- `src-tauri/src/mcp/builtin/workspace/mod.rs`

### Changes Required

#### 3.1 Add cleanup method to `PendingExecutions`

**Location:** After `PendingExecutions` impl block (~line 97)

**Add New Method:**

```rust
impl PendingExecutions {
    // ... existing methods ...

    /// Remove expired pending executions older than the given TTL
    pub fn cleanup_expired(&self, ttl_seconds: u64) {
        let mut map = self.pending.lock().unwrap();
        let now = std::time::SystemTime::now();

        map.retain(|_id, pending| {
            match pending.created_at.elapsed() {
                Ok(elapsed) => elapsed.as_secs() < ttl_seconds,
                Err(_) => false, // Remove entries with invalid timestamps
            }
        });
    }

    /// Get count of pending executions (for monitoring)
    pub fn count(&self) -> usize {
        self.pending.lock().unwrap().len()
    }
}
```

#### 3.2 Update `PendingShellExecution` struct

**Current (lines ~62-71):**

```rust
pub struct PendingShellExecution {
    pub command: String,
    pub work_dir: String,
    pub encryption_nonce: String,
    pub run_mode: RunMode,
    pub timeout: Option<u64>,
    pub environment: HashMap<String, String>,
    pub args: Vec<String>,
    pub requires_user_input: bool,
}
```

**Proposed:**

```rust
pub struct PendingShellExecution {
    pub command: String,
    pub work_dir: String,
    pub encryption_nonce: String,
    pub run_mode: RunMode,
    pub timeout: Option<u64>,
    pub environment: HashMap<String, String>,
    pub args: Vec<String>,
    pub requires_user_input: bool,
    pub created_at: std::time::SystemTime, // NEW: For expiration tracking
}
```

#### 3.3 Update creation site in `handlers.rs`

**Location:** `handle_interactive_shell` function (~line 61-72)

**Add timestamp:**

```rust
let pending_execution = PendingShellExecution {
    command,
    work_dir: work_dir_clone,
    encryption_nonce: encryption_nonce.clone(),
    run_mode: run_mode.clone(),
    timeout,
    environment,
    args,
    requires_user_input: true,
    created_at: std::time::SystemTime::now(), // NEW
};
```

#### 3.4 Add periodic cleanup task in `WorkspaceServer::new`

**Location:** After pending_executions initialization (~line 170)

**Add Cleanup Spawner:**

```rust
impl WorkspaceServer {
    pub fn new(session_id: String, workspace_path: String) -> Self {
        // ... existing initialization ...

        let pending_executions = Arc::new(PendingExecutions::default());

        // Spawn background cleanup task
        let pending_executions_cleanup = Arc::clone(&pending_executions);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                // Cleanup entries older than 10 minutes (2x timeout)
                pending_executions_cleanup.cleanup_expired(600);
            }
        });

        Self {
            // ... rest of initialization ...
        }
    }
}
```

**Notes:**

- Cleanup runs every 60 seconds
- Removes entries older than 10 minutes (2x USER_INPUT_TIMEOUT_SECS)
- Non-blocking async task
- Prevents memory leak from abandoned executions

---

## Phase 4: Documentation Updates (Low Priority)

### Affected Files

- `docs/guides/builtin_tool_bp.md` (if exists)
- `src-tauri/src/mcp/builtin/workspace/tools/code_tools.rs` (inline comments)

### Changes Required

#### 4.1 Add parameter documentation to tool definitions

**Location:** `code_tools.rs` ~line 540-575

**Add Comment Block:**

```rust
// executePendingShell tool - CRITICAL: Parameters must use camelCase
//
// Frontend Integration Note:
// - UI postMessage must send: { executionId: string, userInput: string }
// - Tool schema defines: executionId, userInput (camelCase)
// - Backend handlers now enforce camelCase as primary format
// - Backward compatibility (execution_id, user_input) removed in v0.5.0
//
Tool {
    name: "executePendingShell".to_string(),
    // ... rest of definition ...
}
```

---

## Phase 5: Testing Requirements (High Priority)

### Test Cases to Add

#### 5.1 Unit Tests for Parameter Extraction

**File:** `src-tauri/src/mcp/builtin/workspace/code_execution/interactive/handlers.rs`

**Tests Needed:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_execute_pending_shell_camelcase_params() {
        // Test primary camelCase format
        let input = json!({
            "executionId": "test-id-123",
            "userInput": "encrypted-data"
        });
        // Assert extraction succeeds
    }

    #[test]
    fn test_execute_pending_shell_snake_case_fallback() {
        // Test backward compatibility (v0.4.x only)
        let input = json!({
            "execution_id": "test-id-123",
            "user_input": "encrypted-data"
        });
        // Assert extraction succeeds with warning logged
    }

    #[test]
    fn test_execute_pending_shell_missing_params() {
        // Test error handling
        let input = json!({});
        // Assert returns Err with "Missing executionId parameter"
    }

    #[test]
    fn test_cancel_pending_execution_camelcase() {
        // Test cancelPendingExecution with camelCase
        let input = json!({
            "executionId": "test-id-123"
        });
        // Assert extraction succeeds
    }
}
```

#### 5.2 Integration Tests for Background Cleanup

**File:** `src-tauri/src/mcp/builtin/workspace/mod.rs`

**Tests Needed:**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pending_executions_cleanup_expired() {
        let pending_execs = PendingExecutions::default();

        // Add entry with old timestamp
        let mut old_pending = PendingShellExecution {
            command: "test".to_string(),
            work_dir: "/tmp".to_string(),
            encryption_nonce: "nonce".to_string(),
            run_mode: RunMode::Interactive,
            timeout: None,
            environment: HashMap::new(),
            args: vec![],
            requires_user_input: true,
            created_at: std::time::SystemTime::now() - std::time::Duration::from_secs(700),
        };
        pending_execs.add("old-id".to_string(), old_pending);

        // Add recent entry
        let new_pending = PendingShellExecution {
            created_at: std::time::SystemTime::now(),
            ..old_pending.clone()
        };
        pending_execs.add("new-id".to_string(), new_pending);

        assert_eq!(pending_execs.count(), 2);

        // Cleanup with 10-minute TTL
        pending_execs.cleanup_expired(600);

        assert_eq!(pending_execs.count(), 1); // Only new entry remains
    }

    #[tokio::test]
    async fn test_pending_executions_cleanup_invalid_timestamps() {
        // Test handling of invalid SystemTime (edge case)
        // Should remove entries with elapsed() errors
    }
}
```

#### 5.3 End-to-End Test for UI Integration

**File:** `scripts/verify_tools_modal.py` (extend existing Playwright test)

**Add Test Case:**

```python
async def test_interactive_shell_camelcase_params(page):
    """Verify UI sends camelCase parameters for pending shell execution"""

    # Monitor postMessage events
    params_sent = []
    await page.evaluate("""
        window.addEventListener('message', (event) => {
            if (event.data.type === 'tool') {
                window.__capturedParams = event.data.payload.params;
            }
        });
    """)

    # Trigger interactive shell execution
    # ... (setup code) ...

    # Verify postMessage sent camelCase
    captured = await page.evaluate("window.__capturedParams")
    assert "executionId" in captured, "Expected executionId (camelCase)"
    assert "userInput" in captured, "Expected userInput (camelCase)"
    assert "execution_id" not in captured, "Should not use snake_case"
```

---

## Migration Timeline

### Release v0.4.1 (Current Sprint)

- ✅ Phase 1: Backend with backward compatibility
- ✅ Phase 2: UI postMessage update
- ✅ Phase 3: Background cleanup implementation
- ✅ Phase 5: Test coverage

**Breaking Changes:** None (backward compatible)

### Release v0.5.0 (Next Major)

- ❌ Remove snake_case fallback from `handlers.rs`
- ❌ Update error messages to remove "(execution_id deprecated)" notes
- 📝 Changelog: Document snake_case deprecation removal

**Breaking Changes:** External integrations using snake_case parameters

---

## Risk Assessment

| Risk                                    | Likelihood | Impact | Mitigation                                         |
| --------------------------------------- | ---------- | ------ | -------------------------------------------------- |
| External MCP clients using snake_case   | Medium     | High   | Keep fallback for 1 release, document in CHANGELOG |
| Memory leak from abandoned executions   | High       | Medium | Background cleanup (Phase 3)                       |
| Timezone issues in timestamp comparison | Low        | Low    | Use `SystemTime::elapsed()` (UTC-agnostic)         |
| Cleanup interval too aggressive         | Low        | Low    | 10-minute TTL (2x timeout) with 60s check interval |

---

## Rollback Plan

If issues arise post-deployment:

1. **Parameter extraction failures:**
   - Revert to snake_case-only parsing
   - Add camelCase as fallback instead
   - Document as "fixed in v0.4.2"

2. **Background cleanup performance:**
   - Increase cleanup interval from 60s to 300s
   - Add config parameter for `cleanup_interval_secs`
   - Add metrics logging for cleanup execution time

3. **UI integration breaks:**
   - Revert `ui.rs` postMessage keys to snake_case
   - Keep backend dual-key support indefinitely
   - Document as "deferred to v0.5.0"

---

## Success Metrics

- ✅ Zero runtime errors from parameter extraction failures
- ✅ Memory usage stable (no PendingShellExecution leaks)
- ✅ All unit tests passing (target: 100% coverage for parameter extraction)
- ✅ Integration tests passing (Playwright test verifies camelCase postMessage)
- ✅ Documentation updated with parameter conventions

---

## Implementation Checklist

### Phase 1: Backend (1-2 hours)

- [ ] Update `handle_execute_pending_shell` parameter extraction
- [ ] Update `handle_cancel_pending_execution` parameter extraction
- [ ] Add unit tests for both camelCase and snake_case inputs
- [ ] Verify compilation and existing tests pass

### Phase 2: UI (30 minutes)

- [ ] Update `ui.rs` postMessage payload keys
- [ ] Test HTML generation with manual inspection
- [ ] Verify JavaScript variable names consistent

### Phase 3: Cleanup (2-3 hours)

- [ ] Add `created_at` field to `PendingShellExecution`
- [ ] Implement `cleanup_expired()` and `count()` methods
- [ ] Update creation site in `handlers.rs`
- [ ] Add periodic cleanup task to `WorkspaceServer::new`
- [ ] Write unit tests for cleanup logic
- [ ] Test cleanup doesn't remove active executions

### Phase 4: Documentation (1 hour)

- [ ] Add inline comments to tool definitions
- [ ] Update any existing guides referencing parameters
- [ ] Document deprecated snake_case in CHANGELOG

### Phase 5: Testing (2-3 hours)

- [ ] Write unit tests for parameter extraction
- [ ] Write integration tests for cleanup
- [ ] Extend Playwright tests for UI postMessage
- [ ] Manual end-to-end testing
- [ ] Verify backward compatibility with snake_case

### Review & Deploy

- [ ] Code review with focus on breaking changes
- [ ] Update PR description with migration notes
- [ ] Tag release as v0.4.1
- [ ] Monitor production logs for parameter errors

**Total Estimated Time:** 6-10 hours

---

## References

- Original Issue: Parameter naming inconsistency identified during PR #494 review
- Related Files:
  - `src-tauri/src/mcp/builtin/workspace/code_execution/interactive/handlers.rs`
  - `src-tauri/src/mcp/builtin/workspace/code_execution/interactive/ui.rs`
  - `src-tauri/src/mcp/builtin/workspace/tools/code_tools.rs`
  - `src-tauri/src/mcp/builtin/workspace/mod.rs`
- Design Principle: Single Responsibility Principle (SRP) - canonical naming convention

---

## Appendix: Alternative Approaches Considered

### A. Dual-Key Support (Rejected)

**Pros:**

- No breaking changes ever
- Maximum compatibility

**Cons:**

- Perpetual technical debt
- Confusing for new developers
- Maintenance burden

### B. Immediate Breaking Change (Rejected)

**Pros:**

- Clean cut, no migration path
- Forces immediate alignment

**Cons:**

- Breaks external integrations without notice
- Poor user experience

### C. Frontend-Only Change (Rejected)

**Pros:**

- Simpler implementation

**Cons:**

- Doesn't fix schema mismatch
- Backend still uses wrong convention
