# MCP Server Startup Timeout Configuration

## Overview

Added user-configurable MCP (Model Context Protocol) server startup timeout to system settings. This allows users to adjust timeout values for servers that require longer initialization times without needing to set environment variables.

## Problem Solved

Previously, MCP server initialization timeout was hardcoded to 10 seconds. This caused issues with:

- Slow-starting servers (e.g., chess MCP server requiring 30+ seconds)
- First-run package installations via npx
- Heavy computational initialization
- Windows process creation overhead

## Implementation

### Frontend Changes

#### 1. Updated SystemSettings Interface

**File:** `src/lib/services/settings-service.ts`

Added new field to SystemSettings:

```typescript
export interface SystemSettings {
  maxFileUploadSizeMB: number;
  workspaceCapacityMB: number;
  webActionTimeoutSeconds: number;
  mcpServerStartupTimeoutSeconds: number; // NEW
  searchIndexFrequencyMinutes: number;
  activeSessionRetentionHours: number;
  shellIsolationLevel: IsolationLevel;
}
```

Default value set to 30 seconds (increased from previous 10s default):

```typescript
system: {
  // ...
  mcpServerStartupTimeoutSeconds: 30,
  // ...
}
```

#### 2. Added UI Control

**File:** `src/features/settings/components/SystemPerformanceSettings.tsx`

Added new input field in System & Performance section:

- Label: "MCP Server Startup Timeout (Sec)"
- Range: 10-120 seconds
- Default: 30 seconds
- Description: "How long to wait for MCP tool servers to initialize. Increase if servers fail to start."

### Backend Changes

#### 1. Updated Configuration Logic

**File:** `src-tauri/src/config.rs`

Enhanced `mcp_startup_timeout_seconds()` function:

**Default Value**: 30 seconds

User settings are applied dynamically when creating each session's MCP proxy through `MCPServiceProxyManager`.

```rust
// Config default
pub fn mcp_startup_timeout_seconds() -> u64 {
    DEFAULT_MCP_STARTUP_TIMEOUT_SECONDS // 30
}

// Applied in MCPServiceProxyManager
if let Ok(settings) = get_user_settings("systemSettings").await {
    if let Some(timeout) = settings.mcp_server_startup_timeout_seconds {
        config = config.with_startup_timeout(timeout);
    }
}
```

#### 2. Updated Documentation

**File:** `src-tauri/src/mcp/session_isolation_config.rs`

Updated field documentation:

```rust
/// Timeout in seconds for MCP server process startup/initialization.
/// Default: 30 seconds (increased from 10s to accommodate slower servers)
/// Environment variable: `LIBRAGENT_MCP_STARTUP_TIMEOUT_SECONDS`
/// User setting: Settings → Advanced → System & Performance → MCP Server Startup Timeout
pub process_startup_timeout_seconds: u64,
```

## Usage

### Through UI (Recommended)

1. Navigate to **Settings** (⚙️ icon)
2. Go to **Advanced** tab
3. Scroll to **System & Performance** section
4. Adjust **MCP Server Startup Timeout (Sec)** field
5. Click **Apply Changes**

## Configuration Priority

```text
User Settings (Database)
    ↓ (if not found)
Default Value (30s)
```

## Benefits

1. **User-Friendly:** No need to set environment variables
2. **Persistent:** Settings saved in database
3. **Discoverable:** Visible in UI with helpful description
4. **Flexible:** Can be adjusted per installation

## Testing

Verified compilation and build:

- ✅ Frontend TypeScript compilation (`pnpm build`)
- ✅ Backend Rust compilation (`cargo clippy`)
- ✅ ESLint validation (`pnpm lint`)

## Related Files

### Modified Files

- `src/lib/services/settings-service.ts`
- `src/features/settings/components/SystemPerformanceSettings.tsx`
- `src-tauri/src/config.rs`
- `src-tauri/src/mcp/session_isolation_config.rs`

### Related Documentation

- [Session Isolation Config](../../src-tauri/src/mcp/session_isolation_config.rs)
- [System Settings](../architecture/system-settings.md)
- [Troubleshooting Guide](../guides/troubleshooting.md)

## Migration Notes

No migration required. Existing installations will use the new 30s default. Users can configure through UI after first launch.

## Future Enhancements

Potential improvements:

- Per-server timeout configuration
- Auto-detect slow servers and suggest timeout increase
- Timeout history and recommendations based on actual server performance
- Advanced users: Different timeouts for first run vs subsequent runs
