# Refactoring Plan: Workspace Management Enhancement

**Date:** 2026-01-18 04:00  
**Sprint:** W3-Jan-26  
**Branch:** dev/0.4.0  
**Scope:** Workspace file system integration and user accessibility improvements

---

## 🔧 Architecture Note (CORRECTED)

**Workspace Override Location:**

- ❌ **NOT** in SettingsPage (global settings)
- ✅ **IN** AgentWorkspacePanel (session-specific UI)

**Rationale:** SettingsPage manages global/app-wide settings (API keys, UI language, window size), while workspace override is **per-session state**. AgentWorkspacePanel is already session-aware and is the natural location for session-specific workspace controls.

**Diff Context Lines Location:**

- ✅ **IN** SettingsPage (correct - this is a global setting)

---

## ✅ Clarification Decisions (Approved)

**Q2: Workspace Override Migration**

- **Decision:** No automatic migration; per-session override with cancel UI
- **Rationale:** Users want to edit existing files in place; cancellation returns to session-local path

**Q3: Terminal Launch Platform Support**

- **Decision:** Windows/macOS only (system commands exist); skip Linux unless standard command available
- **Rationale:** No `xdg-terminal` equivalent; avoid terminal emulator detection complexity

**Q4: Diff Context Lines**

- **Decision:** Configurable via Settings page (default: 3 lines)
- **Rationale:** Agents can read setting with readFile if needed; flexible for user preference

---

## 작업의 목적 (Purpose)

### Primary Goals

1. **Improve workspace accessibility** - Add native file explorer and terminal integration
2. **Streamline workspace management** - Allow per-session workspace directory overrides
3. **Enhance user control** - Make workspace directory configurable with proper UI/UX

### Expected Outcomes

- Users can directly access workspace files in native file explorer
- Users can open terminal in workspace directory
- Per-session workspace overrides with clear cancel mechanism
- Improved discoverability through UI buttons

---

## 현재의 상태 / 문제점 (Current State & Problems)

### Problem 1: Limited Workspace Navigation

**Current Behavior:**

- Users view workspace files only through AgentWorkspacePanel UI
- No direct access to file explorer
- No terminal access to workspace directory
- "Go to Home" button is buggy and confusing

**Impact:**

- Users must use separate file manager to access workspace
- No quick terminal access for manual operations
- Poor integration with native OS tools

---

### Problem 2: Workspace Directory Not User-Configurable

**Current Behavior:**

- Workspace directory is global: `{app_data_dir}/workspaces/{session_id}/`
- Users cannot change workspace location per session
- No option to edit files in custom directories

**Impact:**

- Forces all work into app-specific directories
- No support for existing project structures
- Users must copy files to/from workspace

---

### Problem 3: Missing File Explorer & Terminal Buttons

**Current UI:**

```rust
// AgentWorkspacePanel.tsx line 510-520 (buggy "Go to Home" button)
<button onClick={() => handleNavigateHome()}>
  Go to Home
</button>
```

**Problems:**

- "Go to Home" implementation is unclear/broken
- No "Open in Explorer" button
- No "Open in Terminal" button
- Poor UX for quick access to workspace

---

## 관련 코드의 구조 및 동작 방식 Summary (Code Architecture)

### Workspace Module Structure

```
src-tauri/src/mcp/builtin/workspace/
├── mod.rs                        ← WorkspaceServer implementation
│   ├── list_directory()          ← Lists files in workspace
│   └── call_tool()               ← Routes all tool calls
│
├── file_operations/
│   ├── read_write.rs
│   ├── search_query.rs
│   └── edit_replace.rs
│
└── tools/
    └── file_tools.rs
```

### Frontend Workspace Panel

```
src/features/workspace/
├── AgentWorkspacePanel.tsx       ← Main UI component
│   ├── Workspace file tree view
│   ├── File operations
│   └── Navigation controls
└── index.tsx
```

### Session Management

```rust
// src-tauri/src/agent/session.rs
pub struct SessionManager {
    sessions: Arc<DashMap<String, SessionState>>,
}

pub struct SessionState {
    id: String,
    workspace_path: PathBuf,  // ← Currently read-only
    mcp_servers: Vec<MCPServer>,
}
```

### Current Workspace Setup

```rust
// Session creation in SessionManager::create_session_async()
let workspace_dir = app_data_dir.join("workspaces").join(&session_id);
tokio::fs::create_dir_all(&workspace_dir).await?;

let session = SessionState {
    id: session_id,
    workspace_path: workspace_dir,  // ← Fixed path
    mcp_servers: vec![],
};
```

---

## 변경 이후의 상태 / 해결 판정 기준 (Success Criteria)

### Task 1: Remove/Fix Home Button

**Acceptance Criteria:**

- ✅ Delete buggy "Go to Home" button from AgentWorkspacePanel
- ✅ Verify panel still renders correctly without button
- ✅ No broken event handlers remaining

---

### Task 2: Open in File Explorer Button

**Acceptance Criteria:**

- ✅ Button visible in AgentWorkspacePanel header
- ✅ Windows: Opens Explorer with workspace folder selected
- ✅ macOS: Opens Finder with workspace folder
- ✅ Linux: Shows helpful error message
- ✅ Clear button label/icon

---

### Task 3: Open in Terminal Button

**Acceptance Criteria:**

- ✅ Button visible in AgentWorkspacePanel header
- ✅ Windows: Launches cmd.exe in workspace directory
- ✅ macOS: Launches Terminal.app in workspace directory
- ✅ Linux: Shows informational message (no standard terminal)
- ✅ Clear error messages for failures

---

### Task 4: Workspace Directory Override

**Acceptance Criteria:**

- ✅ AgentWorkspacePanel header has "Override workspace directory" input
- ✅ Per-session override with cancel UI in the same panel
- ✅ Cancel button returns to session-local path
- ✅ No automatic migration of existing files
- ✅ Clear user guidance about override behavior
- ✅ Error handling for invalid/inaccessible paths
- ✅ Override UI is session-aware (uses current session context)

---

### Task 5: Diff Context Lines Configuration

**Acceptance Criteria:**

- ✅ Settings page has "Diff context lines" input (default: 3)
- ✅ Agents can read setting with readFile
- ✅ Applied to editFile and editLineInFile diff output
- ✅ Valid range: 1-10 lines

---

## 수정이 필요한 코드 및 수정부분의 코드 스니핏 (Code Modifications)

### Modification 1: Remove Home Button

**File:** `src/features/workspace/AgentWorkspacePanel.tsx`  
**Location:** Lines 510-520 (button definition)

```tsx
// CURRENT CODE (lines 505-520):
<div className="flex gap-2">
  <button
    onClick={() => handleNavigateHome()}
    className="px-3 py-1 text-sm bg-gray-600 hover:bg-gray-700 rounded"
  >
    Go to Home
  </button>
</div>;

// MODIFIED CODE (REMOVE BUTTON - DELETE THESE LINES):
{
  /* Home button removed - use Open in Explorer/Terminal instead */
}
```

**Rationale:**

- Buggy implementation with unclear purpose
- Replaced by "Open in Explorer" and "Open in Terminal" buttons
- Simplifies UI

---

### Modification 2: Add Explorer Button

**File:** `src/features/workspace/AgentWorkspacePanel.tsx`  
**Location:** Same area as removed Home button (line 510)

```tsx
// ADD THESE NEW BUTTONS:
<div className="flex gap-2">
  {/* Open in File Explorer */}
  <button
    onClick={handleOpenInExplorer}
    className="px-3 py-1 text-sm bg-blue-600 hover:bg-blue-700 rounded flex items-center gap-1"
    title="Open workspace folder in file explorer"
  >
    📁 Open in Explorer
  </button>

  {/* Open in Terminal */}
  <button
    onClick={handleOpenInTerminal}
    className="px-3 py-1 text-sm bg-green-600 hover:bg-green-700 rounded flex items-center gap-1"
    title="Open terminal in workspace directory"
  >
    ⌨️ Open in Terminal
  </button>
</div>
```

**Handler Functions:**

```tsx
// ADD HANDLERS IN AgentWorkspacePanel:

const handleOpenInExplorer = async () => {
  try {
    await invoke('open_workspace_in_explorer', {
      sessionId: sessionInfo?.id,
    });
  } catch (error) {
    logger.error('Failed to open explorer', error);
    // Show user-friendly error message
  }
};

const handleOpenInTerminal = async () => {
  try {
    await invoke('open_workspace_in_terminal', {
      sessionId: sessionInfo?.id,
    });
  } catch (error) {
    logger.error('Failed to open terminal', error);
    // Show user-friendly error message
  }
};
```

---

### Modification 3: New Tauri Commands for Explorer/Terminal

**File:** `src-tauri/src/commands/workspace.rs` (NEW or EXTEND)

```rust
use tauri::{AppHandle, command};
use std::process::Command;
use std::path::PathBuf;

#[command]
pub async fn open_workspace_in_explorer(
    session_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // Get workspace path from session
    let session_manager = state.session_manager.read().await;
    let session = session_manager
        .get_session(&session_id)
        .ok_or("Session not found")?;

    let workspace_path = &session.workspace_path;

    // Platform-specific command
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(format!("/select,{}", workspace_path.display()))
            .spawn()
            .map_err(|e| format!("Failed to open Explorer: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("-R")
            .arg(workspace_path)
            .spawn()
            .map_err(|e| format!("Failed to open Finder: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        // Linux: Try various file managers in order
        let file_managers = ["nautilus", "dolphin", "thunar", "pcmanfm"];
        let mut opened = false;

        for fm in &file_managers {
            if let Ok(_) = Command::new(fm)
                .arg(workspace_path)
                .spawn() {
                opened = true;
                break;
            }
        }

        if !opened {
            return Err(
                "No file manager found. Supported: nautilus, dolphin, thunar, pcmanfm".to_string()
            );
        }
    }

    Ok(())
}

#[command]
pub async fn open_workspace_in_terminal(
    session_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // Get workspace path from session
    let session_manager = state.session_manager.read().await;
    let session = session_manager
        .get_session(&session_id)
        .ok_or("Session not found")?;

    let workspace_path = &session.workspace_path;

    // Platform-specific command
    #[cfg(target_os = "windows")]
    {
        // Windows: Open cmd in workspace directory
        Command::new("cmd")
            .args(&["/c", "start", "cmd", "/k", &format!("cd /d {}", workspace_path.display())])
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        // macOS: Open Terminal.app in workspace directory
        let script = format!(
            "tell application \"Terminal\" to do script \"cd {}\"",
            workspace_path.display()
        );

        Command::new("osascript")
            .args(&["-e", &script])
            .spawn()
            .map_err(|e| format!("Failed to open Terminal: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        // Linux: No standard terminal command - return helpful message
        return Err(
            "Terminal launch not supported on Linux. No standard command available. \
             Open a terminal manually and navigate to the workspace directory.".to_string()
        );
    }

    Ok(())
}
```

**Register Commands in `src-tauri/src/main.rs`:**

```rust
// In the tauri::Builder setup:
.invoke_handler(tauri::generate_handler![
    // ... existing commands ...
    open_workspace_in_explorer,
    open_workspace_in_terminal,
    get_workspace_override,
    set_workspace_override,
    cancel_workspace_override,
])
```

---

### Modification 4: Workspace Override - AgentWorkspacePanel

**File:** `src/features/agent/components/AgentWorkspacePanel.tsx` (EXTEND)

```tsx
// ADD WORKSPACE OVERRIDE UI IN AGENTWORKSPACEPANEL HEADER:

export function AgentWorkspacePanel() {
  // ... existing state ...\n  const { session } = useAgentSessionState();
  const [workspaceOverride, setWorkspaceOverride] = useState<string>('');
  const [isOverrideActive, setIsOverrideActive] = useState(false);

  // Load current override state on mount
  useEffect(() => {
    if (session?.id) {
      invoke('get_workspace_override', { sessionId: session.id })
        .then((path) => {
          if (path) {
            setWorkspaceOverride(path as string);
            setIsOverrideActive(true);
          }
        })
        .catch((err) => logger.error('Failed to load workspace override', err));
    }
  }, [session?.id]);

  const handleSetOverride = async () => {
    if (!workspaceOverride.trim() || !session?.id) return;

    try {
      await invoke('set_workspace_override', {
        sessionId: session.id,
        overridePath: workspaceOverride,
      });
      setIsOverrideActive(true);
      toast.success('Workspace override set successfully');
      // Reload directory to show new workspace
      loadDirectory('./');
    } catch (error) {
      logger.error('Failed to set workspace override', error);
      toast.error(`Failed to set override: ${error}`);
    }
  };

  const handleCancelOverride = async () => {
    if (!session?.id) return;

    try {
      await invoke('cancel_workspace_override', { sessionId: session.id });
      setWorkspaceOverride('');
      setIsOverrideActive(false);
      toast.success('Workspace override cancelled');
      // Reload directory to show session-local workspace
      loadDirectory('./');
    } catch (error) {
      logger.error('Failed to cancel workspace override', error);
      toast.error(`Failed to cancel override: ${error}`);
    }
  };

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between">
          <CardTitle>Workspace</CardTitle>
          <div className="flex gap-2">
            {/* Existing buttons: Open in Explorer, Open in Terminal */}
            <Button onClick={handleOpenInExplorer} size="sm">
              📁 Explorer
            </Button>
            <Button onClick={handleOpenInTerminal} size="sm">
              ⌨️ Terminal
            </Button>
          </div>
        </div>

        {/* Workspace Override UI */}
        <div className="mt-3 space-y-2">
          <div className="flex gap-2">
            <Input
              type="text"
              placeholder="/path/to/custom/workspace"
              value={workspaceOverride}
              onChange={(e) => setWorkspaceOverride(e.target.value)}
              className="flex-1"
              disabled={isOverrideActive}
            />
            {!isOverrideActive ? (
              <Button onClick={handleSetOverride} size="sm">
                Override
              </Button>
            ) : (
              <Button
                onClick={handleCancelOverride}
                size="sm"
                variant="destructive"
              >
                Cancel Override
              </Button>
            )}
          </div>
          {isOverrideActive && (
            <p className="text-xs text-yellow-400">
              ⚠️ Using custom workspace: {workspaceOverride}
            </p>
          )}
          {!isOverrideActive && (
            <p className="text-xs text-muted-foreground">
              Override workspace directory for this session (files stay in
              original location)
            </p>
          )}
        </div>
      </CardHeader>

      <CardContent>{/* Existing file tree UI */}</CardContent>
    </Card>
  );
}
```

---

### Modification 5: Backend Workspace Override Support

**File:** `src-tauri/src/agent/session.rs` (EXTEND SessionState)

```rust
pub struct SessionState {
    pub id: String,
    pub workspace_path: PathBuf,
    pub workspace_override: Option<PathBuf>,  // ← NEW: Per-session override
    pub mcp_servers: Vec<MCPServer>,
}

impl SessionState {
    pub fn get_effective_workspace(&self) -> &PathBuf {
        // Return override if set, otherwise session-local workspace
        self.workspace_override.as_ref().unwrap_or(&self.workspace_path)
    }
}
```

**File:** `src-tauri/src/commands/workspace.rs` (ADD NEW COMMANDS)

```rust
#[command]
pub async fn get_workspace_override(
    session_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let session_manager = state.session_manager.read().await;
    let session = session_manager
        .get_session(&session_id)
        .ok_or("Session not found")?;

    Ok(session.workspace_override.clone().map(|p| p.to_string_lossy().to_string()))
}

#[command]
pub async fn set_workspace_override(
    session_id: String,
    override_path: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // Validate path exists and is accessible
    let override_path = PathBuf::from(&override_path);

    if !override_path.exists() {
        return Err(format!("Path does not exist: {}", override_path.display()));
    }

    if !override_path.is_dir() {
        return Err(format!("Path is not a directory: {}", override_path.display()));
    }

    // Check if directory is readable/writable
    if !check_dir_access(&override_path).await? {
        return Err("Directory is not accessible (check permissions)".to_string());
    }

    // Set override in session
    let mut session_manager = state.session_manager.write().await;
    if let Some(mut session) = session_manager.get_session_mut(&session_id) {
        session.workspace_override = Some(override_path);
        Ok(())
    } else {
        Err("Session not found".to_string())
    }
}

#[command]
pub async fn cancel_workspace_override(
    session_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut session_manager = state.session_manager.write().await;
    if let Some(mut session) = session_manager.get_session_mut(&session_id) {
        session.workspace_override = None;  // Reset to session-local workspace
        Ok(())
    } else {
        Err("Session not found".to_string())
    }
}

async fn check_dir_access(path: &PathBuf) -> Result<bool, String> {
    // Try to read directory contents
    match tokio::fs::read_dir(path).await {
        Ok(_) => Ok(true),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                Ok(false)
            } else {
                Err(e.to_string())
            }
        }
    }
}
```

**File:** `src-tauri/src/mcp/builtin/workspace/mod.rs` (UPDATE PATH RESOLUTION)

```rust
// In WorkspaceServer implementation:
impl WorkspaceServer {
    pub fn new(session_id: String, app_data_dir: PathBuf, session_manager: Arc<SessionManager>) -> Self {
        // When resolving workspace path, use get_effective_workspace()
        let workspace_path = {
            let session = session_manager.get_session(&session_id);
            session
                .map(|s| s.get_effective_workspace().clone())
                .unwrap_or_else(|| app_data_dir.join("workspaces").join(&session_id))
        };

        Self {
            session_id,
            workspace_path,
            session_manager,
        }
    }
}
```

---

### Modification 6: Diff Context Lines Setting (Global Setting)

**Note:** This is a **global setting** (unlike workspace override which is session-specific), so SettingsPage is the correct location.

**File:** `src-tauri/src/config/app_settings.rs` (EXTEND AppSettings)

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct AppSettings {
    // ... existing settings ...
    #[serde(default = "default_diff_context_lines")]
    pub diff_context_lines: usize,
}

fn default_diff_context_lines() -> usize {
    3
}

impl AppSettings {
    pub fn get_diff_context_lines(&self) -> usize {
        // Ensure value is within valid range
        std::cmp::min(10, std::cmp::max(1, self.diff_context_lines))
    }
}
```

**Frontend Settings Handler:**

```typescript
// In SettingsPage.tsx (Tab: Advanced)
async function handleSaveDiffContextLines(value: number) {
  if (value < 1 || value > 10) {
    logger.warn('Invalid diff context lines value', { value });
    return;
  }

  try {
    await invoke('save_app_settings', {
      diffContextLines: value,
    });
    logger.info('Diff context lines updated', { value });
  } catch (error) {
    logger.error('Failed to save diff context lines', error);
  }
}
```

---

## 재사용 가능한 연관 코드 (Reusable Related Code)

### Platform Detection Utility

**File:** `src-tauri/src/utils/platform.rs` (NEW or EXTEND)

```rust
#[cfg(target_os = "windows")]
pub const PLATFORM: &str = "windows";

#[cfg(target_os = "macos")]
pub const PLATFORM: &str = "macos";

#[cfg(target_os = "linux")]
pub const PLATFORM: &str = "linux";

pub fn is_windows() -> bool {
    cfg!(target_os = "windows")
}

pub fn is_macos() -> bool {
    cfg!(target_os = "macos")
}

pub fn is_linux() -> bool {
    cfg!(target_os = "linux")
}
```

---

### Session Access Pattern

**File:** `src-tauri/src/agent/session.rs`

```rust
pub struct SessionManager {
    sessions: Arc<DashMap<String, SessionState>>,
}

impl SessionManager {
    pub fn get_session(&self, session_id: &str) -> Option<SessionState> {
        self.sessions.get(session_id).map(|entry| entry.clone())
    }

    pub fn get_session_mut(&mut self, session_id: &str) -> Option<&mut SessionState> {
        // For mutable operations - requires write lock on SessionManager
        self.sessions.get_mut(session_id).map(|mut entry| &mut *entry)
    }
}
```

---

## Test Code 추가 및 수정 필요 부분에 대한 가이드 (Test Guidelines)

### Test File Structure

```
src-tauri/tests/
├── workspace_integration_tests.rs
└── platform_specific_tests.rs
```

### Test Cases for Explorer/Terminal Buttons

```rust
#[test]
#[cfg(target_os = "windows")]
fn test_open_explorer_windows() {
    // Verify explorer command format
    let workspace_path = PathBuf::from("C:\\workspace");
    let expected_cmd = format!("explorer /select,{}", workspace_path.display());
    // Assert command is valid
}

#[test]
#[cfg(target_os = "macos")]
fn test_open_terminal_macos() {
    // Verify osascript command format
    let workspace_path = PathBuf::from("/home/user/workspace");
    let script = format!("tell application \"Terminal\" to do script \"cd {}\"", workspace_path.display());
    // Assert script is valid AppleScript
}

#[test]
fn test_workspace_override_path_validation() {
    // Test that invalid paths are rejected
    // Test that inaccessible paths are rejected
    // Test that valid paths are accepted
}

#[test]
fn test_workspace_override_cancellation() {
    // Verify override is cleared when cancelled
    // Verify session returns to original workspace path
}

#[test]
fn test_diff_context_lines_bounds() {
    // Test min value (1)
    // Test max value (10)
    // Test values outside bounds are clamped
}
```

---

## Clarification Q-list

### Q2: Workspace Override Migration

**Question:** How should existing sessions handle workspace directory changes?

**Context:** If user changes workspace directory, existing sessions still reference old path.

**Options:**

- A) Keep old sessions in old directory (no migration)
- B) Automatically migrate files to new directory (risky)
- C) Prompt user for migration decision (best UX)

**Answer:** No Migration is needed. General usecase is that user wants to make files they have be edited by AI agent, and don't want to mess up their existing location. We need UI/UX to cancel the override, then the workspace will point to the session local path.

---

### Q3: Terminal Launch Platform Support

**Question:** Should we support terminal launching on Linux?

**Context:** Linux has many terminal emulators (gnome-terminal, konsole, xfce4-terminal, alacritty, kitty, etc.). No standard command like `open` (macOS) or `start` (Windows).

**Options:**

- A) Auto-detect and launch available terminal (complex)
- B) User-configurable terminal preference (medium complexity)
- C) Skip Linux support for now (simplest)

**Answer:** Only add this feature when there is system-wide standard command like `open`. Otherwise, drop this feature for Linux.

---

### Q4: Diff Context Lines

**Question:** Should editLineInFile support configurable diff context lines?

**Context:** Current implementation: ±3 lines of context (hardcoded). For large edits, more context may be helpful.

**Options:**

- A) Keep hardcoded ±3 lines (simpler)
- B) Add optional `context` parameter to tool (more flexible)

**Answer:** Make Settings page have setting for the context, and confirm that AI agent can read that context if they want using readFile with line specified.

---

## 추가 분석 과제 (Additional Analysis Tasks)

### Task 1: Platform-Specific Terminal Detection

**Purpose:** Verify terminal launch works reliably on all platforms

**Testing Required:**

- Windows: Test cmd.exe, PowerShell, Windows Terminal
- macOS: Test Terminal.app, iTerm2 detection
- Linux: Document unsupported state with helpful error message

---

### Task 2: File Manager Availability

**Purpose:** Verify file explorer integration across platforms

**Testing Required:**

- Windows: Verify Explorer integration works
- macOS: Verify Finder integration works
- Linux: Test multiple file managers (nautilus, dolphin, thunar, pcmanfm)

---

### Task 3: Workspace Override Edge Cases

**Purpose:** Ensure override system is robust

**Edge Cases to Test:**

- Override path contains spaces
- Override path is network/SMB path
- Override path becomes inaccessible after setting
- Override path is deleted after setting
- Session with override path is serialized and restored

---

## 참고 자료 (References)

### External Documentation

- [Tauri Command Invocation](https://tauri.app/v1/guides/features/command/)
- [Platform-Specific Code (Tauri)](https://tauri.app/v1/guides/platform-specific/)

### Internal Documentation

- [File Operations Architecture](../../docs/architecture/file-operations.md)
- [Session Management](../../docs/architecture/session-management.md)
- [Settings System](../../docs/architecture/settings.md)

---

**Document Status:** Ready for Workspace Management Implementation  
**Phase 2 Priority:** High (System Integration)  
**Estimated Completion:** 10-13 hours (terminal: 2-3hrs, override: 6-8hrs, diff settings: 2hrs)
