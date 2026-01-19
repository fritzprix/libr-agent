# Command Helper Utility Refactoring

## Problem Analysis

### Original Implementation Issues

1. **Code Duplication**: Same Windows cmd.exe wrapping logic duplicated in:
   - `stdio_manager.rs` (session-isolated MCP servers)
   - `lifecycle.rs` (global MCP servers)

2. **Hardcoded Command List**:

   ```rust
   let might_be_cmd = command == "npx"
       || command == "npm"
       || command == "uvx"
       || command == "uv"
       || command.contains("node");
   ```

   - Fragile and requires updates for new tools
   - Incomplete (missing `.bat`, `.ps1`, other tools)

3. **No File Extension Detection**:
   - Code guessed if command was `.cmd` based on name
   - Didn't check actual file extensions

4. **Platform-Specific Logic Scattered**:
   - Windows-specific code mixed with business logic
   - Hard to test and maintain

## Improved Implementation

### New Utility Module: `mcp/utils/command_helper.rs`

**Key Improvements:**

1. **Centralized Logic**:

   ```rust
   pub fn prepare_command(command: &str, args: &[String]) -> (String, Vec<String>)
   ```

   - Single source of truth for command preparation
   - Used by both `stdio_manager.rs` and `lifecycle.rs`

2. **Pattern-Based Detection**:

   ```rust
   matches!(
       basename,
       "npx" | "npm" | "node" | "pnpm" | "yarn" | "bun" |  // Node.js
       "uvx" | "uv" | "pip" | "pipx" |                       // Python
       "python" | "python3"
   )
   ```

   - Uses Rust's `matches!` macro for cleaner syntax
   - Easy to extend with new tools
   - Self-documenting with ecosystem grouping

3. **Extension-Based Detection**:

   ```rust
   if command.ends_with(".cmd") || command.ends_with(".bat") || command.ends_with(".ps1")
   ```

   - Handles explicit file extensions
   - Supports `.cmd`, `.bat`, `.ps1` scripts

4. **Cross-Platform Design**:

   ```rust
   #[cfg(windows)]
   fn needs_shell_wrapper(command: &str) -> bool { ... }

   #[cfg(not(windows))]
   fn needs_shell_wrapper(command: &str) -> bool { false }
   ```

   - Platform-specific code isolated with `#[cfg]`
   - Unix/Linux: No wrapping (commands work directly)
   - Windows: Automatic wrapping for scripts

5. **PowerShell Support**:

   ```rust
   if command.ends_with(".ps1") {
       ("powershell.exe", ["-ExecutionPolicy", "Bypass", "-File", ...])
   }
   ```

   - `.ps1` files use PowerShell with bypass policy
   - `.cmd`/`.bat` and Node.js tools use `cmd.exe /C`

## Test Results

### Before Refactoring

```
❌ Direct npx: program not found
✅ cmd.exe /c npx: SUCCESS
```

### After Refactoring

```
✅ All commands properly wrapped:
  - npx    → cmd.exe /C npx
  - npm    → cmd.exe /C npm
  - node   → cmd.exe /C node
  - uvx    → cmd.exe /C uvx
  - python → cmd.exe /C python

✅ .exe files pass through unchanged:
  - custom.exe → custom.exe (no wrapping)

✅ Actual spawn test:
  - npx --version → 11.1.0 (SUCCESS)
```

## Code Changes

### 1. New Utility Module

**File**: `src-tauri/src/mcp/utils/command_helper.rs`

- `needs_shell_wrapper(command: &str) -> bool`: Determines if wrapping needed
- `prepare_command(command: &str, args: &[String]) -> (String, Vec<String>)`: Main API
- Comprehensive unit tests (11 test cases)

### 2. stdio_manager.rs Refactoring

**Before** (26 lines of Windows-specific logic):

```rust
let (final_command, final_args) = if cfg!(windows) && !command.ends_with(".exe") {
    let might_be_cmd = command == "npx" || command == "npm" || ...;
    if might_be_cmd {
        let mut new_args = vec!["/c".to_string(), command.to_string()];
        new_args.extend(args.iter().cloned());
        ("cmd.exe".to_string(), new_args)
    } else { ... }
} else { ... };
```

**After** (3 lines):

```rust
let (final_command, final_args) =
    crate::mcp::utils::command_helper::prepare_command(command, args);
```

### 3. lifecycle.rs Refactoring

**Before** (39 lines with debug logging):

```rust
log::info!("=== MCP Server Spawn Debug Info ===");
// ... 15 lines of PATH logging ...
let (final_command, final_args) = if cfg!(windows) && !command.ends_with(".exe") {
    // ... 20+ lines of Windows logic ...
};
log::info!("Final spawn command: {} {:?}", final_command, final_args);
```

**After** (5 lines):

```rust
let (final_command, final_args) =
    crate::mcp::utils::command_helper::prepare_command(command, args);

log::info!("Starting MCP server '{}': {} {:?} (env vars: {})",
    name, final_command, final_args, env.len());
```

## Benefits

1. **Maintainability**:
   - Single location to update Windows command wrapping logic
   - New tools added by updating one match pattern
   - Clear separation of concerns

2. **Testability**:
   - Utility function easily testable in isolation
   - Example test: `test_command_helper.rs`
   - No dependency on full MCP stack

3. **Readability**:
   - Business logic in `stdio_manager`/`lifecycle` now cleaner
   - Platform-specific details abstracted away
   - Self-documenting with ecosystem grouping

4. **Extensibility**:
   - Easy to add new tool ecosystems (e.g., Ruby, Go)
   - Easy to add new script types (e.g., `.sh` on Windows via WSL)
   - Pattern-based approach scales well

5. **Correctness**:
   - Handles all script file types (`.cmd`, `.bat`, `.ps1`)
   - Proper PowerShell execution policy for `.ps1`
   - Uses `/C` (standard) instead of `/c` (lowercase)

## Migration Guide

For any new code that spawns processes:

```rust
// ❌ OLD: Manual Windows handling
#[cfg(windows)]
let cmd = if is_script { "cmd.exe" } else { command };

// ✅ NEW: Use command_helper
use crate::mcp::utils::command_helper;
let (cmd, args) = command_helper::prepare_command(command, args);
```

## References

- Test: `src-tauri/examples/test_command_helper.rs`
- Test: `src-tauri/examples/test_cmd_exe_wrapper.rs`
- Implementation: `src-tauri/src/mcp/utils/command_helper.rs`
- Usage:
  - `src-tauri/src/mcp/session_isolation/stdio_manager.rs`
  - `src-tauri/src/mcp/server/lifecycle.rs`
