# Debugging MCP Browser Agent Waiters Issue

## Overview

This document provides instructions for debugging the MCP browser agent waiters issue where `result_waiters` HashMap appears empty when `handle_script_result` is called, even though waiters should have been inserted during `execute_script`.

## Issue Summary

The problem manifests as:
1. `execute_script` creates a oneshot channel and inserts it into `result_waiters` with `session_id` as key
2. JavaScript executes and calls `window.__TAURI_INTERNALS__.invoke('browser_script_result', { payload })`
3. Rust's `handle_script_result` is called but finds the waiters HashMap empty
4. This results in failed script execution results and timeout errors

## Enhanced Logging

The codebase has been enhanced with detailed logging to help diagnose this issue:

### execute_script Logging
- Session ID being processed
- Waiter insertion confirmation with current waiters list
- Script execution status
- Result reception or timeout details
- Cleanup operations with remaining waiters

### handle_script_result Logging
- Session ID received from JavaScript
- Current waiters before attempting removal
- Success/failure of waiter lookup and removal
- Potential session ID mismatches
- Final waiters state

### Log Analysis Points
Look for these patterns in the logs:

```
[INFO] execute_script: Inserted waiter for session_id: 'xxx', current waiters: [...]
[INFO] execute_script: Script wrapper executed in session: 'xxx', now waiting for result
[INFO] handle_script_result: Called for session_id: 'xxx'
[INFO] handle_script_result: Available waiters before removal: []
[ERROR] handle_script_result: No waiting channel found for session 'xxx'
```

## Debugging Tools

### Built-in Diagnostic Command

A comprehensive diagnostic system has been added:

```rust
#[tauri::command]
pub async fn run_browser_diagnostic(
    session_id: String,
    server: tauri::State<'_, InteractiveBrowserServer>,
) -> Result<String, String>
```

#### Usage from Frontend
```javascript
import { invoke } from '@tauri-apps/api/tauri';

// Run diagnostic for a specific session
const result = await invoke('run_browser_diagnostic', { 
    sessionId: 'your-session-id-here' 
});
console.log('Diagnostic result:', result);
```

#### What the Diagnostic Tests
1. **Waiter Lifecycle Test**: Manually inserts and retrieves waiters
2. **Session ID Format Consistency**: Tests various ID formats
3. **Race Condition Simulation**: Tests concurrent operations

### Debug Methods Added

New public methods for debugging:

```rust
// Get current waiters list
pub fn get_waiters_debug(&self) -> Result<Vec<String>, String>

// Get waiters count
pub fn get_waiters_count_debug(&self) -> Result<usize, String>

// Insert test waiter
pub fn insert_test_waiter_debug(&self, session_id: &str) -> Result<oneshot::Receiver<String>, String>
```

## Diagnostic Steps

### Step 1: Enable Detailed Logging

Ensure your log level is set to `INFO` or `DEBUG` to see all diagnostic messages.

### Step 2: Run the Diagnostic Tool

```javascript
// Test with the problematic session ID from your logs
const sessionId = '6d958f16-ada5-4b85-abae-bb52e8188972';
const result = await invoke('run_browser_diagnostic', { sessionId });
```

### Step 3: Monitor Logs During Script Execution

When running `execute_script`, watch for:

1. **Waiter Insertion**: Confirm the waiter is actually inserted
   ```
   [INFO] execute_script: Inserted waiter for session_id: 'xxx', current waiters: ["xxx"]
   ```

2. **JavaScript Execution**: Confirm the script wrapper is executed
   ```
   [INFO] execute_script: Script wrapper executed in session: 'xxx'
   ```

3. **Result Handler Call**: Check if `handle_script_result` is called
   ```
   [INFO] handle_script_result: Called for session_id: 'xxx'
   ```

4. **Waiters State**: Check what waiters exist when the handler runs
   ```
   [INFO] handle_script_result: Available waiters before removal: []
   ```

### Step 4: Session ID Format Analysis

The diagnostic will test different session ID formats to identify potential mismatches:
- Standard UUID format
- Simple test IDs
- The exact format from your error logs

### Step 5: Race Condition Testing

The diagnostic simulates concurrent operations to identify timing issues.

## Common Causes and Solutions

### 1. Session ID Mismatch
**Symptoms**: Different session IDs in insert vs. retrieve operations
**Check**: Compare session IDs in logs, look for format differences
**Solution**: Ensure consistent ID generation and usage

### 2. Race Condition
**Symptoms**: Waiter disappears before JavaScript can invoke the result handler
**Check**: Timing of operations in logs
**Solution**: Review locking strategy and operation ordering

### 3. Lock Contention
**Symptoms**: Failed to acquire lock errors
**Check**: Lock acquisition failures in logs
**Solution**: Review mutex usage patterns

### 4. JavaScript Invocation Issues
**Symptoms**: `handle_script_result` never called
**Check**: Browser console for JavaScript errors
**Solution**: Verify Tauri IPC setup and JavaScript execution

## Additional Debugging Tips

### Browser Console Monitoring
Check the browser console for messages like:
```
[TAURI INJECTION] Sending to browser_script_result: {sessionId: "...", result: "..."}
```

### Session ID Validation
Ensure session IDs:
- Are properly generated (UUID format recommended)
- Are consistently formatted between Rust and JavaScript
- Don't contain special characters that could cause issues

### Timing Analysis
Look for the sequence:
1. Waiter inserted
2. Script executed
3. JavaScript calls result handler
4. Result handler processes

If this sequence is broken, identify where it fails.

## Reporting Issues

When reporting this issue, include:

1. **Full log sequence** from `execute_script` start to completion
2. **Session ID** that's causing problems
3. **JavaScript console output** showing the injection attempt
4. **Diagnostic tool results** for the problematic session
5. **Browser and OS information**

## Code Locations

Key files for understanding this issue:

- `src-tauri/src/services/interactive_browser_server.rs` - Main implementation
- `src-tauri/src/commands/browser_commands.rs` - Command handlers
- `src-tauri/src/services/browser_debug.rs` - Diagnostic tools

## Next Steps

If the diagnostic tools don't reveal the root cause:

1. Add more granular logging around mutex operations
2. Consider using `tracing` for better async operation tracking
3. Implement waiters timeout monitoring
4. Add JavaScript-side debugging hooks
5. Consider alternative IPC mechanisms for result delivery