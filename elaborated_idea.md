# Sprint W3-Jan-26: Elaborated Implementation Plan

**Date:** January 18, 2026  
**Sprint:** W3-Jan-26  
**Branch:** dev/0.4.0  
**Status:** Planning & Clarification Phase

---

## 📋 Table of Contents

1. [Overview](#overview)
2. [Feature Analysis: File Operations Enhancement](#feature-analysis-file-operations-enhancement)
3. [Feature Analysis: Workspace Management](#feature-analysis-workspace-management)
4. [Critical Clarification Questions](#critical-clarification-questions)
5. [Implementation Priorities](#implementation-priorities)
6. [Technical Implementation Notes](#technical-implementation-notes)
7. [Risk Assessment](#risk-assessment)

---

## Overview

This document provides a comprehensive feasibility analysis and design clarification for the Sprint W3-Jan-26 agenda, which focuses on two main areas:

1. **File Operations Enhancement** - Improving editFile and adding new search/edit capabilities
2. **Workspace Management** - Enhanced workspace mounting, opening, and configuration

---

## Feature Analysis: File Operations Enhancement

### 1. editFile: Identical String Detection

#### 📊 Status: ✅ HIGHLY FEASIBLE

#### Current Implementation

- **Location:** [`src-tauri/src/mcp/builtin/workspace/file_operations/edit_replace.rs`](src-tauri/src/mcp/builtin/workspace/file_operations/edit_replace.rs)
- **Lines:** 162-370
- **Pattern:** Extensive validation using `ErrorGuidance` pattern

#### Proposed Enhancement

Add validation to detect when `oldString` and `newString` are identical:

```rust
// After line 202 (after newString parameter extraction)
if old_string == new_string {
    return Ok(ErrorGuidance::with_guidance(
        ErrorCategory::InvalidInput,
        "oldString and newString are identical - no changes to make",
        vec![
            "Verify you intended to modify the content".to_string(),
            "If deleting text, use empty string for newString".to_string(),
            "If no changes needed, this operation is unnecessary".to_string(),
        ],
        ToolGroup::Workspace,
    ).to_mcp_result());
}
```

#### Implementation Details

- **Effort Estimate:** ⏱️ 30 minutes
- **Files to Modify:** 1 (edit_replace.rs)
- **Testing Required:** Unit test + integration test
- **Breaking Changes:** None
- **Dependencies:** None

#### Benefits

- Prevents unnecessary file writes
- Clearer error messages for agents
- Catches common copy-paste errors
- Maintains file modification timestamps accurately

---

### 2. searchLineInFile Tool

#### 📊 Status: ⚠️ PARTIALLY IMPLEMENTED - Needs Clarification

#### Existing Implementation: `grep` Tool

The [`grep`](src-tauri/src/mcp/builtin/workspace/file_operations/search_query.rs) tool (lines 308-507) already provides:

✅ **Current Capabilities:**

- Regex pattern matching
- Exact string matching
- Line number reporting (`lineNumbers: true`)
- Context display (±2 lines around matches)
- Both file and text input support

**Current Output Format:**

```json
{
  "matches": [
    { "line": 42, "text": "matched content" },
    { "line": 87, "text": "another match" }
  ]
}
```

**Text Output Example:**

````
**🔍 Grep Results: 2 match(es) found**

File: `src/main.rs`
Pattern: `error`
Options: case-insensitive

```rust
>   42 | fn handle_error() {
    43 |     log::error!("Error occurred");
    44 | }
````

#### Critical Clarification Questions

**Q1: Tool Naming & Discoverability**

- Should `searchLineInFile` be a **new separate tool** or an **alias** to `grep`?
- Problem: `grep` is not discoverable as "search line in file" functionality
- Agents may not know to use `grep` for line searching

> Answer: yes, it's kind of semantic issue of the tool name, agent never uses the `grep` tool even though there is task where the tool would be efficiently used for. in terms of functionality, the grep tool is almost identical to the search in line tool

**Q2: Functional Differences**
What specific features would `searchLineInFile` have that `grep` doesn't?

| Feature       | Current `grep` | Proposed `searchLineInFile` |
| ------------- | -------------- | --------------------------- |
| Regex support | ✅ Yes         | ❓ Yes/No?                  |
| Exact match   | ✅ Yes         | ❓ Yes only?                |
| Line numbers  | ✅ Optional    | ❓ Always?                  |
| Context lines | ✅ ±2 lines    | ❓ Configurable?            |
| Return format | JSON + Text    | ❓ Same/Different?          |

> Answer: no difference, just reuse the logic implemented

**Q3: Interface Design**

```rust
// Option A: Enhance grep tool description
"grep" - Search for patterns in files (also known as: searchLineInFile)

// Option B: Create separate simpler tool
"searchLineInFile" - Simple exact string search with line numbers
  → No regex complexity
  → Always returns line numbers
  → Simpler interface for basic searches

// Option C: Create alias tool
"searchLineInFile" - Alias for grep tool with preset options
  → Calls grep internally with lineNumbers=true
  → Simplified parameter set
```

> Answer just keep searchLineInFile as sole tool for the feature and remove grep, I think we can just rename grep tool and update their description in tool metadata

#### Recommendations

**Option 1: Enhance grep Tool (Recommended)**

- **Effort:** ⏱️ 1-2 hours
- **Approach:** Update tool description and examples
- **Benefits:** No code duplication, maintains single source of truth

```rust
pub fn create_grep_tool() -> MCPTool {
    MCPTool {
        name: "grep".to_string(),
        title: Some("Search Lines in File (grep)".to_string()),
        description: "Search for patterns in files and return matching lines with line numbers.

🎯 USE CASES:
- Find specific text in files (exact match or regex)
- Search with line numbers for editLineInFile
- Locate code sections before editing
- Text pattern matching

ALIASES: searchLineInFile, searchPattern, findInFile

PARAMETERS:
- pattern: Search pattern (exact string or regex)
- path: File path to search (or use 'input' for text)
- lineNumbers: true (returns line numbers) | false (text only)
- ignoreCase: Case-insensitive search

RETURNS:
- Line numbers and matched text
- Context lines (±2 lines around matches)
- Language-highlighted code blocks

EXAMPLES:
1. Find function definition:
   grep({pattern: \"def calculate\", path: \"main.py\", lineNumbers: true})

2. Search for errors:
   grep({pattern: \"error|exception\", path: \"logs.txt\", ignoreCase: true})

💡 NEXT: Use editLineInFile to modify matched lines
"
    }
}
```

**Option 2: Create Separate Tool**

- **Effort:** ⏱️ 4-6 hours
- **Approach:** New tool with simplified interface
- **Risks:** Code duplication, maintenance overhead

---

### 3. editLineInFile Tool

#### 📊 Status: ⚠️ NEEDS DESIGN CLARIFICATION - Complex Feature

#### Proposed Interface (from idea.md)

```typescript
editLineInFile(
  path: string,
  edits: [{line: number, value: string}]
) -> diff output (±2-3 lines around changes)
```

#### Critical Design Issues

##### Issue 1: Line Number Instability ⚠️

**Problem:** Line numbers change after each edit

**Example:**

```
Original file (5 lines):
1: import os
2: import sys
3:
4: def main():
5:     pass

Edit request: [
  {line: 2, value: "import json"},  // Replace line 2
  {line: 5, value: "    print('hello')"}  // Replace line 5
]

After first edit:
1: import os
2: import json  ← Changed
3:                ← Line 3 is now different!
4: def main():   ← Line 4 is now what was line 4
5:     pass      ← Line 5 is still line 5

But what if the edit changes number of lines?
```

**Solutions:**

1. **Apply in reverse order** (high to low) to maintain line numbers
2. **Atomic transaction** - calculate all offsets first
3. **Content-based matching** - like current `editFile`

##### Issue 2: Safety vs Convenience Trade-off

**Option A: Line-Number-Only (Dangerous)**

```rust
// No content validation
edits: [
  {line: 42, value: "new content"}
]
// Risk: Wrong line edited if file changed since last read
```

**Option B: Line-Number + Content Validation (Safe)**

```rust
// Requires old content for safety
edits: [
  {line: 42, old_value: "original", new_value: "new content"}
]
// Benefit: Fails if line doesn't match expectation
```

**Option C: Hybrid Approach**

```rust
// Optional old_value for validation
edits: [
  {line: 42, old_value?: "original", new_value: "new content"}
]
// If old_value provided → validate
// If omitted → blind replacement (agent's responsibility)
```

##### Issue 3: Multi-Edit Coordination

**Q1: Atomicity**

- Should all edits succeed/fail together?
  - only successful, when all the line given as input param are valid
- Or partial success with error reporting?
  - report which ones are incorrect as the agent can clearly understand what they are wrong about

**Q2: Conflict Detection**

```rust
edits: [
  {line: 5, value: "new 5"},
  {line: 5, value: "other 5"}  // Conflict!
]
```

- need definitely this feature, we have to check the conflict and if there is, the tool call should be rejected with proper error message

**Q3: Line Range Edits**

```rust
// What if edit spans multiple lines?
edits: [
  {line: 5, value: "line 5\nline 6\nline 7"}
]
// Does this replace line 5 only? Or 5-7?
```

- agent should use editFile for such a use case

#### Design Comparison: editFile vs editLineInFile

| Aspect             | Current `editFile`                 | Proposed `editLineInFile`         |
| ------------------ | ---------------------------------- | --------------------------------- |
| **Input Method**   | Content-based (exact string match) | Line-number-based                 |
| **Safety**         | High (requires exact match)        | Low (line numbers can shift)      |
| **Multi-edit**     | Sequential calls                   | Batch array                       |
| **Validation**     | Always validates content           | Optional validation?              |
| **Diff Output**    | ✅ Shows ±3 lines context          | ✅ Requested feature              |
| **Agent Workflow** | readFile → extract → edit          | searchLineInFile → editLineInFile |

#### Use Case Analysis

**Current Workflow (editFile):**

```javascript
// Agent workflow
1. readFile("test.py", 40, 50)  // Read lines 40-50
2. Agent extracts exact text:
   const oldText = `def calculate(a, b):
       return a + b`
3. editFile("test.py", oldText, newText)
4. Receives diff with context
```

**Proposed Workflow (editLineInFile):**

```javascript
// Proposed workflow
1. searchLineInFile("test.py", "def calculate")  // Find line
2. Returns: {line: 42, text: "def calculate(a, b):"}
3. editLineInFile("test.py", [
     {line: 42, value: "def calculate(a, b, c):"},
     {line: 43, value: "    return a + b + c"}
   ])
4. Receives diff with context
```

**Question:** What advantage does line-number-based editing provide?

**Concerns:**

1. **Race Condition:** File changes between search and edit
2. **Fragility:** Line numbers are not stable identifiers
3. **Agent V2 Context:** Agents see text content, not structured JSON arrays
4. **Duplication:** Current `editFile` already handles multi-line replacements

#### Critical Clarification Questions

**Q1: Primary Use Case**

- What specific scenario requires line-number-based editing?
- Why can't current `editFile` (content-based) solve this?
- Is the goal to simplify agent workflow or add new capability?

**Q2: Safety Requirements**

- Should line content be validated before replacement?
- How to handle file changes between search and edit?
- Is blind line replacement acceptable risk?

**Q3: Multi-Edit Semantics**

- Atomic (all-or-nothing) or partial success?
- How to handle overlapping line edits?
- Should edits be ordered by user or auto-sorted?

**Q4: Line Range Handling**

- Can one edit replace multiple lines?
- What if new_value contains multiple lines?
- How to delete lines (empty string or omit)?

**Q5: Backward Compatibility**

- Will this complement or replace `editFile`?
- Should agents be trained to use both tools?
- Migration path for existing agent prompts?

#### Proposed Design (Pending Clarification)

**Design A: Safe Line Editor (Recommended)**

```rust
pub async fn handle_edit_line_in_file(
    &self,
    args: Value,
    session_id: Option<String>,
) -> Result<MCPResult, String> {
    // Parameters
    struct LineEdit {
        line: usize,              // 1-based line number
        old_value: Option<String>, // Optional validation
        new_value: String,         // New content (can be multi-line)
    }

    // Algorithm
    // 1. Read entire file
    // 2. Validate all line numbers in range
    // 3. If old_value provided, validate content matches
    // 4. Sort edits by line number (descending) for stability
    // 5. Apply edits in reverse order (high to low)
    // 6. Generate unified diff with ±3 line context
    // 7. Write file atomically
    // 8. Return diff output
}
```

**Design B: Hybrid Editor**

```rust
// Support both validation modes
edits: [
  // Validated edit
  {
    line: 42,
    old_value: "original content",  // Fails if mismatch
    new_value: "new content"
  },
  // Unvalidated edit (agent's responsibility)
  {
    line: 50,
    new_value: "blind replacement"  // No validation
  }
]
```

#### Implementation Estimate

- **Effort:** ⏱️ 8-12 hours
  - Core logic: 4-6 hours
  - Validation & error handling: 2-3 hours
  - Diff generation: 1-2 hours
  - Tests: 2-3 hours
- **Complexity:** High
- **Risk:** Medium (design ambiguity)

---

## Feature Analysis: Workspace Management

### 1. Workspace Created in Local App Cache

#### 📊 Status: ✅ ALREADY IMPLEMENTED

#### Current Implementation

- **Location:** [`src-tauri/src/session.rs`](src-tauri/src/session.rs)
- **Function:** `create_session_workspace_async()` (lines 132-183)
- **Path Pattern:** `{app_data_dir}/workspaces/{session_id}/`

**Evidence:**

```rust
let session_dir = self.base_data_dir.join("workspaces").join(session_id);
// Example: ~/.local/share/com.fritzprix.libragent/workspaces/abc123/
```

**Features:**

- ✅ Template-based workspace creation
- ✅ Async directory creation
- ✅ Session isolation
- ✅ Error handling and fallback

**Conclusion:** ✅ No action required - feature already complete

---

### 2. User-Configurable Workspace Directory

#### 📊 Status: ⚠️ PARTIALLY IMPLEMENTED - UI Needed

#### Current Backend Support

**Implemented:**

- ✅ `SessionManager::get_workspace_dir()` - returns per-session workspace
- ✅ `switch_session` command - updates workspace context
- ✅ Session isolation mechanism

**Missing:**

- ❌ UI to override workspace directory
- ❌ Backend command to update session workspace path
- ❌ Settings storage for custom workspace roots
- ❌ Path validation and security checks

#### Critical Clarification Questions

**Q1: Scope of Configuration**

- **Per-Session Override:** Each session has custom workspace?
- **Global Override:** All sessions use same custom directory?
- **Template Override:** Change default for new sessions only?

**Q2: UI Placement**
Where should this configuration appear?

**Option A: Workspace Panel**

```tsx
<WorkspacePanel.Header>
  <Button onClick={openWorkspaceDirSelector}>
    <Settings /> Configure Directory
  </Button>
</WorkspacePanel.Header>
```

**Option B: Settings Modal**

```tsx
<Settings.Workspace>
  <DirectoryPicker
    label="Default Workspace Directory"
    value={workspaceRoot}
    onChange={setWorkspaceRoot}
  />
</Settings.Workspace>
```

**Option C: Session Settings**

```tsx
<SessionDetails>
  <DirectoryPicker
    label="Workspace Directory for this Session"
    value={sessionWorkspaceDir}
    onChange={updateSessionWorkspace}
  />
</SessionDetails>
```

**Q3: Persistence**

- **Temporary:** Override lasts until app restart?
- **Session-Persistent:** Saved to session metadata in DB?
- **Global-Persistent:** Saved to app settings?

**Q4: Security Constraints**
Which directories should be allowed?

- **Unrestricted:** Any directory user can access?
- **Restricted:** Only subdirectories of app data dir?
- **Whitelist:** User-approved directories only?
- **Sandboxed:** Respect OS sandboxing (macOS, Windows)?

**Q5: Migration Strategy**
What happens to existing sessions?

- Keep using default workspace?
- Migrate files to new directory?
- Provide migration tool?

#### Proposed Implementation

**Backend Changes:**

```rust
// src-tauri/src/commands/workspace_commands.rs

#[tauri::command]
pub async fn set_workspace_directory(
    session_id: String,
    directory_path: String,
    validate_only: bool,
) -> Result<SetWorkspaceResponse, String> {
    let session_manager = get_session_manager()?;

    // 1. Validate path exists and is accessible
    // 2. Check security constraints
    // 3. Verify write permissions
    // 4. If validate_only, return validation result
    // 5. Otherwise, update session workspace path
    // 6. Create directory if needed
    // 7. Invalidate workspace cache

    Ok(SetWorkspaceResponse {
        success: true,
        workspace_path: validated_path,
    })
}
```

**Frontend Changes:**

```tsx
// src/features/agent/components/WorkspaceDirectorySelector.tsx

export function WorkspaceDirectorySelector() {
  const [customDir, setCustomDir] = useState<string | null>(null);

  const handleSelectDirectory = async () => {
    const selected = await openDirectoryDialog();

    // Validate before setting
    const result = await setWorkspaceDirectory(
      sessionId,
      selected,
      true, // validate_only
    );

    if (result.success) {
      setCustomDir(selected);
      await setWorkspaceDirectory(sessionId, selected, false);
    }
  };

  return <DirectoryPicker value={customDir} onSelect={handleSelectDirectory} />;
}
```

#### Implementation Estimate

- **Effort:** ⏱️ 6-8 hours
  - Backend validation: 2-3 hours
  - UI component: 2-3 hours
  - Settings persistence: 1-2 hours
  - Testing: 1-2 hours

---

### 3. Open Workspace in File Explorer

#### 📊 Status: ✅ HIGHLY FEASIBLE

#### Current Implementation

- **Backend API:** `openWorkspaceFileWithDefaultApp()` - already exists!
- **Location:** [`src/lib/backend/workspace.ts`](src/lib/backend/workspace.ts)
- **Current Usage:** Opens individual files

#### Proposed Enhancement

Extend to open directories:

**Frontend Implementation:**

```tsx
// src/features/agent/components/AgentWorkspacePanel.tsx

const openWorkspaceInExplorer = async () => {
  try {
    // Get current workspace directory
    const workspaceDir = await getWorkspaceDir(session?.id);

    // Open directory with system default file manager
    await openWorkspaceFileWithDefaultApp(workspaceDir, session?.id);

    toast.success('Opened workspace in file explorer');
  } catch (error) {
    logger.error('Failed to open workspace', { error });
    toast.error('Failed to open workspace directory');
  }
};

// Add button to header
<CardHeader>
  <div className="flex items-center gap-1">
    <Button onClick={openWorkspaceInExplorer}>
      <FolderOpen className="w-3 h-3" />
    </Button>
  </div>
</CardHeader>;
```

**Backend Verification:**

```rust
// src-tauri/src/commands/workspace_commands.rs

#[tauri::command]
pub async fn open_workspace_in_explorer(
    session_id: Option<String>,
) -> Result<(), String> {
    let session_manager = get_session_manager()?;
    let workspace_dir = session_manager.get_workspace_dir(
        &session_id.unwrap_or("default".to_string())
    );

    // Open directory with system default app
    open::that(workspace_dir)
        .map_err(|e| format!("Failed to open directory: {}", e))?;

    Ok(())
}
```

#### Platform Support

- ✅ Windows: Opens in File Explorer
- ✅ macOS: Opens in Finder
- ✅ Linux: Opens in default file manager

#### Implementation Estimate

- **Effort:** ⏱️ 2-3 hours
- **Complexity:** Low
- **Risk:** Low

---

### 4. Open Workspace in Terminal

#### 📊 Status: ⚠️ BACKEND COMMAND NEEDED

#### Current State

- ❌ No existing command to open terminal
- ✅ Workspace path available via `get_workspace_dir()`
- ⚠️ Platform-specific terminal launching needed

#### Proposed Implementation

**Backend Command:**

```rust
// src-tauri/src/commands/workspace_commands.rs

#[tauri::command]
pub async fn open_terminal_at_workspace(
    session_id: Option<String>,
) -> Result<(), String> {
    let session_manager = get_session_manager()?;
    let workspace_dir = session_manager.get_workspace_dir(
        &session_id.unwrap_or("default".to_string())
    );

    open_terminal_at_path(&workspace_dir)?;
    Ok(())
}

fn open_terminal_at_path(path: &Path) -> Result<(), String> {
    use std::process::Command;

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(&["/c", "start", "cmd", "/k", "cd", "/d"])
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        // Use AppleScript for better control
        Command::new("osascript")
            .arg("-e")
            .arg(format!(
                "tell application \"Terminal\" to do script \"cd '{}'\"",
                path.display()
            ))
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        // Try multiple terminal emulators
        let terminals = [
            ("gnome-terminal", vec!["--working-directory"]),
            ("konsole", vec!["--workdir"]),
            ("xfce4-terminal", vec!["--working-directory"]),
            ("xterm", vec!["-e", "cd"]),
        ];

        let mut success = false;
        for (terminal, args) in terminals.iter() {
            if let Ok(_) = Command::new(terminal)
                .args(args)
                .arg(path)
                .spawn()
            {
                success = true;
                break;
            }
        }

        if !success {
            return Err("No supported terminal emulator found".to_string());
        }
    }

    Ok(())
}
```

**Frontend Integration:**

```tsx
const openWorkspaceInTerminal = async () => {
  try {
    await openTerminalAtWorkspace(session?.id);
    toast.success('Opened workspace in terminal');
  } catch (error) {
    logger.error('Failed to open terminal', { error });
    toast.error('Failed to open terminal');
  }
};

<Button onClick={openWorkspaceInTerminal}>
  <Terminal className="w-3 h-3" />
</Button>;
```

#### Platform Considerations

**Windows:**

- Opens cmd.exe by default
- Could support PowerShell as alternative
- WSL terminal support?

**macOS:**

- Opens Terminal.app
- Could support iTerm2 detection
- AppleScript provides good control

**Linux:**

- Multiple terminal emulators to support
- Fallback chain for compatibility
- Wayland vs X11 considerations

#### Implementation Estimate

- **Effort:** ⏱️ 4-5 hours
  - Backend command: 2-3 hours
  - Platform testing: 1-2 hours
  - UI integration: 1 hour

---

### 5. Remove "Go to Home" Button

#### 📊 Status: ✅ TRIVIAL - Simple Removal

#### Current Implementation

**Location:** [`src/features/agent/components/AgentWorkspacePanel.tsx`](src/features/agent/components/AgentWorkspacePanel.tsx) line 515

```tsx
<Button
  variant="ghost"
  size="sm"
  onClick={() => navigateToDirectory('/')}
  className="h-6 w-6 p-0"
  title="Go to root"
>
  <Home className="w-3 h-3" />
</Button>
```

#### Clarification Questions

**Q1: What is the Bug?**

- Does `navigateToDirectory('/')` not work?
- Does it navigate to wrong directory?
- Does it cause errors?

**Q2: Fix vs Remove**

- Should we **fix** the navigation logic?
- Or **remove** because feature is not useful?
- Or **replace** with "Open in Explorer" button?

#### Proposed Actions

**Option A: Simple Removal**

```tsx
// Delete lines 510-520
// Keep only Refresh button
<div className="flex items-center gap-1">
  <Button onClick={() => loadDirectory(rootPath)}>
    <RefreshCw />
  </Button>
</div>
```

**Option B: Fix Navigation**

```tsx
// Fix to navigate to workspace root
<Button
  onClick={() => {
    const workspaceRoot = './';
    setRootPath(workspaceRoot);
    loadDirectory(workspaceRoot);
  }}
>
  <Home />
</Button>
```

**Option C: Replace with Explorer Button**

```tsx
// Remove Home, add Explorer and Terminal
<div className="flex items-center gap-1">
  <Button onClick={openWorkspaceInExplorer}>
    <FolderOpen />
  </Button>
  <Button onClick={openWorkspaceInTerminal}>
    <Terminal />
  </Button>
  <Button onClick={() => loadDirectory(rootPath)}>
    <RefreshCw />
  </Button>
</div>
```

#### Implementation Estimate

- **Effort:** ⏱️ 5 minutes (removal) | 30 minutes (fix) | 3-4 hours (replace with new buttons)

---

## Critical Clarification Questions

### Priority 1: Blocking Implementation

#### Q1: editLineInFile - Core Purpose

**Question:** What is the primary use case that `editLineInFile` solves that current `editFile` cannot?

**Context:**

- Current `editFile` already handles multi-line replacements
- Content-based matching is safer than line-number-based
- Agents work with text content, not structured arrays

**Options:**

- A) Simplify agent workflow (fewer steps)
- B) Enable new capability (batch edits)
- C) Improve performance (single call vs multiple)
- D) Other reason (please specify)

#### Q2: editLineInFile - Safety Model

**Question:** Should line edits validate content before replacement?

**Trade-offs:**

- **Validated:** Safer but requires more parameters
- **Unvalidated:** Simpler but risk of wrong-line edits

**Recommendation:** Hybrid - optional validation for safety

#### Q3: searchLineInFile vs grep

**Question:** Should we create a new tool or enhance `grep`'s discoverability?

**Analysis:**

- `grep` already provides all requested functionality
- Issue is discoverability, not capability
- Creating duplicate tool increases maintenance burden

**Recommendation:** Enhance `grep` tool description and add aliases

---

### Priority 2: Design Decisions

#### Q4: Workspace Override Scope

**Question:** Should workspace directory override be per-session or global?

**Options:**

- **Per-Session:** More flexible, complex UI
- **Global:** Simpler, less flexible
- **Both:** Maximum flexibility, most complex

#### Q5: Workspace Override Security

**Question:** What security restrictions should apply to custom workspace directories?

**Options:**

- **Unrestricted:** Any accessible directory
- **Restricted:** Only within app data dir
- **Approved:** User must explicitly approve each directory

**Recommendation:** Approved list for security

#### Q6: Terminal Launch Preference

**Question:** Should users be able to choose which terminal emulator to use?

**Context:** Linux has many terminal emulators

**Options:**

- **Auto-detect:** Try common terminals in order
- **Configurable:** User selects preferred terminal
- **Both:** Auto-detect with override option

---

## Implementation Priorities

### Phase 1: Quick Wins (Week 1)

**Total Effort:** ~4-5 hours

1. ✅ **editFile Identical String Check** (30 min)
   - Simple validation addition
   - Clear error messages
   - No dependencies

2. ✅ **Remove/Fix Home Button** (5-30 min)
   - Clarify bug first
   - Simple code change

3. ✅ **Open in File Explorer** (2-3 hours)
   - Backend already supports it
   - Simple UI addition
   - Cross-platform compatible

---

### Phase 2: Medium Priority (Week 2-3)

**Total Effort:** ~10-12 hours

4. 🔶 **Open in Terminal** (4-5 hours)
   - Backend command needed
   - Platform-specific logic
   - Testing required

5. 🔶 **Enhance grep Tool Documentation** (1-2 hours)
   - Update descriptions
   - Add aliases
   - Improve examples

6. 🔶 **Workspace Directory Override UI** (6-8 hours)
   - Pending scope clarification
   - Settings integration
   - Validation logic

---

### Phase 3: Complex Features (Week 3-4)

**Total Effort:** ~8-12 hours

7. ⏸️ **editLineInFile Tool** (8-12 hours)
   - **BLOCKED:** Awaiting design clarification
   - High complexity
   - Significant testing required

---

## Technical Implementation Notes

### File Operations Architecture

#### Current Pattern: Content-Based Editing

```
Agent → readFile → Extract Exact Text → editFile(oldText,newText) →
Validate & Replace → Generate Diff → Write File
```

**Advantages:**

- Safe: Content must match exactly
- Stable: Immune to line number shifts
- Validated: Automatic verification

#### Proposed Pattern: Line-Based Editing

```
Agent → searchLineInFile → Find Line Numbers → editLineInFile(lines) →
Validate Line Numbers → Sort Edits → Apply in Reverse → Generate Diff → Write File
```

**Advantages:**

- Convenient: Fewer parameters
- Batch: Multiple edits in one call
- Fast: Single file write

**Disadvantages:**

- Risky: Line numbers can be stale
- Complex: Multi-edit coordination
- Validation: Optional vs required

---

### Workspace Management Architecture

#### Current Session Isolation

```
app_data_dir/
├── workspaces/
│   ├── session_abc123/     ← Isolated workspace
│   ├── session_def456/     ← Isolated workspace
│   └── templates/
│       └── base/           ← Template for new sessions
├── config/
└── logs/
```

#### Proposed Custom Workspaces

```
app_data_dir/
├── workspaces/
│   ├── session_abc123/     ← Default location
│   └── session_def456 -> /custom/path/  ← Symlink to custom dir
├── config/
│   └── workspace_overrides.json  ← Custom directory mappings
└── logs/
```

**Security Considerations:**

- Validate custom paths don't escape sandbox
- Check write permissions before switching
- Maintain approved directory list
- Handle symlink/junction for redirect

---

## Risk Assessment

### High Risk Items

#### 1. editLineInFile - Design Ambiguity

**Risk:** Unclear requirements lead to multiple reimplementations

**Mitigation:**

- ✅ Defer implementation until design clarified
- ✅ Create detailed design document first
- ✅ Get stakeholder approval before coding

#### 2. Workspace Override - Security

**Risk:** Unrestricted directory access creates security vulnerabilities

**Mitigation:**

- ✅ Implement path validation
- ✅ Maintain approved directory list
- ✅ Respect OS-level sandboxing
- ✅ Audit all path operations

---

### Medium Risk Items

#### 3. Terminal Launch - Platform Compatibility

**Risk:** Different terminal emulators on Linux

**Mitigation:**

- ✅ Implement fallback chain
- ✅ Test on multiple distros
- ✅ Provide configuration option
- ✅ Clear error messages if no terminal found

#### 4. grep Enhancement - Breaking Changes

**Risk:** Changing grep behavior affects existing agents

**Mitigation:**

- ✅ Only update descriptions, not behavior
- ✅ Add aliases without changing core
- ✅ Maintain backward compatibility

---

### Low Risk Items

#### 5. editFile Validation - Simple Addition

**Risk:** Minimal - simple validation check

**Mitigation:**

- ✅ Add comprehensive tests
- ✅ Maintain existing error patterns

#### 6. UI Button Changes - Cosmetic

**Risk:** Minimal - UI-only changes

**Mitigation:**

- ✅ Test on all platforms
- ✅ Verify accessibility

---

## Next Steps

### Immediate Actions Required

1. **Answer Clarification Questions**
   - Review Priority 1 questions
   - Decide on editLineInFile design
   - Confirm workspace override scope

2. **Approve Implementation Priorities**
   - Phase 1: Quick wins
   - Phase 2: Medium priority
   - Phase 3: Complex features

3. **Begin Phase 1 Implementation**
   - Start with low-risk items
   - Gather feedback iteratively

---

## Appendix: Code Location Reference

### Backend Files

- File Operations: `src-tauri/src/mcp/builtin/workspace/file_operations/`
  - `edit_replace.rs` - editFile implementation
  - `search_query.rs` - grep/searchFiles implementation
  - `read_write.rs` - readFile/createFile implementation
- Session Management: `src-tauri/src/session.rs`
- Workspace Commands: `src-tauri/src/commands/workspace_commands.rs`

### Frontend Files

- Workspace Panel: `src/features/agent/components/AgentWorkspacePanel.tsx`
- Backend Client: `src/lib/backend/workspace.ts`
- Rust Backend Hook: `src/hooks/use-rust-backend.ts`

### Tool Definitions

- File Tools: `src-tauri/src/mcp/builtin/workspace/tools/file_tools.rs`
- Tool Registry: `src-tauri/src/mcp/builtin/workspace/tools/mod.rs`

---

**Document Version:** 1.0  
**Last Updated:** January 18, 2026  
**Status:** Awaiting Clarification
