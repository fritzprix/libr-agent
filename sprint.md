# Sprint W3-Jan-26: Agent Workspace Enhancements

**Branch:** dev/0.4.0  
**Sprint Duration:** 3-4 weeks  
**Total Estimated Effort:** ~22-30 hours (updated after clarifications)

---

## 🎯 Key Clarifications Resolved

1. **Line Count Limit:** editLineInFile enforces 10,000 lines maximum (practical LLM context)
2. **Workspace Override:** Per-session with cancel UI; no automatic migration
3. **Terminal Launch:** Windows/macOS only (Linux skipped - no standard command)
4. **Diff Context:** Configurable via Settings (default: 3 lines)
5. **Multi-File Search:** Enhance searchFiles to return line numbers

---

## Sprint Goals

1. Improve agent file editing workflow and tool discoverability
2. Enhance workspace usability with native system integration
3. Strengthen validation and error prevention in file operations

---

## Feature Overview

### 1. File Operations Enhancement

#### **1.1 editFile - Identical String Validation**

**Problem:** Agents sometimes call editFile with identical old/new values, causing unnecessary file writes.

**Approach:**

- Add pre-validation check in edit_replace.rs (line ~202)
- Return clear error: "old_value and new_value are identical"
- No file operations performed when strings match

**Effort:** 30 minutes  
**Priority:** Quick Win

---

#### **1.2 searchLineInFile - Tool Rename**

**Problem:** Agents don't discover the "grep" tool despite perfect functionality for line searching.

**Approach:**

- Rename `grep` → `searchLineInFile` in tool metadata
- Keep all existing search_query.rs logic (regex, exact match, line numbers)
- Update tool description to emphasize line-based searching

**Rationale:** This is a semantic naming fix, not a functional change. The tool already does everything needed.

**Effort:** 1-2 hours  
**Priority:** Quick Win

---

#### **1.4 searchFiles Enhancement - Add Line Numbers**

**Problem:** Multi-file search doesn't return line numbers, forcing agents to use multiple searchLineInFile calls.

**Approach:**

- Add `line_number` field to searchFiles response format
- Maintain backward compatibility with existing response structure
- Update tool description to mention line number support

**Rationale:** Reuse existing multi-file search infrastructure; keep searchLineInFile focused on single-file use case.

**Effort:** 2 hours  
**Priority:** Phase 2 (NEW - from Q5 clarification)

---

#### **1.3 editLineInFile - Batch Line Editor**

**Problem:** Agents need multi-step workflow (searchLineInFile → multiple editFile calls) for batch single-line edits.

**Approach:**

- New tool accepting array of `{line: number, old_value?: string, new_value: string}`
- **Line count limit: 10,000 lines maximum** (files >10K lines exceed practical LLM context)
- Atomic operation: ALL edits succeed or ALL fail
- Validate all line numbers in range before applying any edits
- Detect and reject duplicate line numbers (conflict detection)
- Apply edits in reverse order (high to low) to preserve line stability
- Generate unified diff with **configurable context lines** (default: 3, set in Settings)

**Design Constraints:**

- Single-line replacements only (agents use editFile for multi-line)
- Optional old_value validation for safety
- Clear error reporting for which specific lines failed

**Effort:** 8-12 hours  
**Priority:** Phase 3

---

### 2. Workspace Management

#### **2.1 Workspace in Local Cache**

**Status:** ✅ Already Implemented

Location: `{app_data_dir}/workspaces/{session_id}/`  
No action required.

---

#### **2.2 Open Workspace in File Explorer**

**Problem:** Users can't easily access workspace files outside the app.

**Approach:**

- Add "Open in Explorer" button to AgentWorkspacePanel header
- Call existing `openWorkspaceFileWithDefaultApp()` with workspace directory
- Cross-platform support (Windows Explorer, macOS Finder, Linux file managers)

**Effort:** 2-3 hours  
**Priority:** Quick Win

---

#### **2.3 Open Workspace in Terminal**

**Problem:** Users need terminal access for advanced file operations.

**Approach:** Platform-Specific Implementation

- **Windows:** Use `cmd /c start cmd /k cd /d <path>` (system command)
- **macOS:** Use `open -a Terminal <path>` (system command)
- **Linux:** Return error "not supported" (no xdg-terminal equivalent)

**Rationale:** Avoid terminal emulator detection complexity on Linux; use only system-wide commands where they exist.

**Effort:** 2-3 hours (reduced from 4-5)  
**Priority:** Phase 2

---

#### **2.4 Remove "Go to Home" Button**

**Problem:** Home button is buggy (specific bug unclear).

**Approach:**

- Remove button from AgentWorkspacePanel.tsx (line ~515)
- Replace with new "Open in Explorer" and "Open in Terminal" buttons
- Keep only Refresh button in header

**Effort:** 5 minutes (removal) | 3-4 hours (full replacement with new buttons)  
**Priority:** Quick Win

---

#### **2.5 User-Configurable Workspace Directory**

**Status:** Partially implemented backend, needs UI.

**Approach:** Per-Session Override with Cancel UI

- **Use Case:** Users want AI to edit existing files without moving them
- **Scope:** Per-session override (not global)
- **No Migration:** Sessions keep their workspace paths
- **Cancellation:** UI provides "Cancel Override" button to revert to session-local path

**Implementation:**

- Backend: `set_workspace_directory(session_id, path, validate_only)` command
- Frontend: Directory picker in Workspace Panel with "Cancel Override" button
- Validation: Path exists, write permissions, security constraints
- Persistence: Save override path in session metadata

**Effort:** 6-8 hours  
**Priority:** Phase 2

---

## Implementation Plan

### **Phase 1: Quick Wins (Week 1) - 4-5 hours**

```
✅ Priority 1: editFile identical string check (30 min)
✅ Priority 2: Rename grep → searchLineInFile (1-2 hrs)
✅ Priority 3: Open workspace in file explorer (2-3 hrs)
✅ Priority 4: Remove/fix Home button (5 min)
```

**Deliverables:**

- Improved tool discoverability for agents
- Better file operation validation
- Enhanced workspace access for users

---

### **Phase 2: System Integration (Week 2-3) - 10-13 hours**

```
🔶 Priority 1: Open workspace in terminal (2-3 hrs) - Windows/macOS only
🔶 Priority 2: Workspace directory override (6-8 hrs) - Per-session with cancel UI
🔶 Priority 3: searchFiles enhancement (2 hrs) - Add line numbers to response
```

**Deliverables:**

- Native system integration (Explorer on all platforms + Terminal on Windows/macOS)
- Per-session workspace override with cancellation UI
- No migration strategy (sessions keep their paths)
- searchFiles returns line numbers for multi-file search workflows

---

### **Phase 3: Advanced Features (Week 3-4) - 8-12 hours**

```
⏸️ Priority 1: editLineInFile tool (8-12 hrs)
```

**Deliverables:**

- Streamlined batch editing workflow for agents
- Reduced multi-step editing complexity

---

## Key Technical Decisions

### ✅ Confirmed Decisions

1. **searchLineInFile:** Rename only, no functional changes
2. **editLineInFile Atomicity:** All edits succeed or all fail (atomic operation)
3. **editLineInFile Conflicts:** Detect duplicate line numbers, reject with clear error
4. **editLineInFile Scope:** Single-line replacements only (multi-line → use editFile)
5. **Terminal Launch:** Platform-specific with fallback chain on Linux
6. **Explorer Button:** Reuse existing backend API

### ⚠️ Pending Decisions

1. **Workspace Override Scope:** Global vs per-session vs both?
   - **Recommendation:** Start with global, iterate to per-session

---

## Risk Assessment

| Feature             | Risk Level | Mitigation                                |
| ------------------- | ---------- | ----------------------------------------- |
| editFile validation | Low        | Simple check, comprehensive tests         |
| grep rename         | Low        | Metadata only, backward compatible        |
| Explorer button     | Low        | Existing API, cross-platform tested       |
| Terminal launch     | Medium     | Fallback chain, platform testing          |
| Workspace override  | Medium     | Path validation, security checks          |
| editLineInFile      | High       | Detailed design doc, stakeholder approval |

---

## Success Metrics

- ✅ Agents discover and use searchLineInFile without prompting
- ✅ Zero unnecessary file writes from identical editFile calls
- ✅ Users can access workspace via native tools (Explorer/Terminal)
- ✅ editLineInFile reduces multi-step editing from 5+ calls to 1 call
- ✅ No security vulnerabilities from custom workspace paths

---

## Code Locations (Reference)

**Backend:**

- File Operations: `src-tauri/src/mcp/builtin/workspace/file_operations/`
- Session Management: `src-tauri/src/session.rs`
- Tool Definitions: `src-tauri/src/mcp/builtin/workspace/tools/file_tools.rs`

**Frontend:**

- Workspace Panel: `src/features/agent/components/AgentWorkspacePanel.tsx`
- Backend Client: `src/lib/backend/workspace.ts`

---

## Next Actions

1. ✅ Begin Phase 1 implementation (quick wins)
2. 🔶 Test on all platforms (Windows, macOS, Linux)
3. 🔶 Decide workspace override scope for Phase 2
4. ⏸️ Create detailed design doc for editLineInFile before Phase 3

---

**Sprint Status:** Ready to Start  
**Last Updated:** January 18, 2026
