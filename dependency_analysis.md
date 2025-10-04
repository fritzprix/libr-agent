# Dependency Analysis - lib.rs Refactoring

**Date**: 2025-10-04
**Branch**: dev/0.1.1

## Baseline Measurements

- **lib.rs line count**: 1154 lines
- **Commands in lib.rs**: 36 Tauri commands
- **Compile time**: 0.18s (cached)

## Global State Usage

### State Variables

1. **MCP_MANAGER** (`OnceLock<MCPServerManager>`)
   - Used by: All MCP-related commands (17 commands)
   - Access pattern: `get_mcp_manager()` function
   - Initialization: Lazy init via `get_or_init`

2. **SQLITE_DB_URL** (`OnceLock<String>`)
   - Used by: `delete_content_store` command
   - Access pattern: `get_sqlite_db_url()` function
   - Initialization: Explicit via `set_sqlite_db_url()`

### Commands Using Global State

**MCP_MANAGER users (17 commands)**:

- start_mcp_server (line 207)
- stop_mcp_server (line 216)
- call_mcp_tool (line 229)
- sample_from_mcp_server (line 255)
- list_mcp_tools (line 263)
- list_tools_from_config (line 332)
- get_connected_servers (line 383)
- check_server_status (line 389)
- check_all_servers_status (line 399)
- list_all_tools (line 461)
- get_validated_tools (line 470)
- list_builtin_servers (line 487)
- list_builtin_tools (line 498)
- call_builtin_tool (line 510)
- list_all_tools_unified (line 751)
- call_tool_unified (line 764)

**SQLITE_DB_URL users (1 command)**:

- delete_content_store (line 408)

## Cross-Command Dependencies

**Analysis Result**: ✅ **NO circular dependencies detected**

- No commands invoke other commands via Tauri's invoke mechanism
- Commands are independently executable
- Safe to modularize in any order

## Module Dependencies

### External Dependencies (to be imported in new modules)

- `session::get_session_manager()` - Used by workspace, session, file commands
- `services::SecureFileManager` - Used by file commands
- `services::InteractiveBrowserServer` - Already in browser_commands
- `sqlx::SqlitePool` - Used by content_store commands
- `tauri` traits and macros - Used by all commands

## Refactoring Safety Assessment

✅ **LOW RISK** - All conditions favorable:

1. No circular dependencies between commands
2. Clear global state access patterns
3. Independent command modules
4. Well-defined import boundaries

## Recommended Module Structure

```
commands/
├── mod.rs                    # Central re-exports
├── workspace_commands.rs     # 4 commands, uses get_session_manager
├── mcp_commands.rs          # 17 commands (existing + new), uses get_mcp_manager
├── content_store_commands.rs # 1 command, uses get_sqlite_db_url
├── log_commands.rs          # 3 commands, uses get_session_manager
├── file_commands.rs         # 4 commands, uses get_session_manager + SecureFileManager
├── download_commands.rs     # 2 commands, uses get_session_manager
├── url_commands.rs          # 1 command, standalone
├── session_commands.rs      # existing + 4 legacy commands
└── browser_commands.rs      # existing
```

## Next Steps

1. ✅ Create state.rs module
2. Move commands to respective modules
3. Update lib.rs imports and invoke_handler
4. Validation and testing
