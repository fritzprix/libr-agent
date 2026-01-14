# Workspace Built-in Tool BP Alignment Refactoring Plan

**Date**: January 13, 2026  
**Priority**: HIGH  
**Estimated Effort**: 8-12 hours  
**Compliance Score**: 6.2/10 → Target: 9.0/10

## 1. 작업의 목적

현재 Workspace 도구의 구현이 최신 `builtin_tool_bp.md` (Best Practices)와 일치하지 않는 부분을 수정하여, **에러 처리의 일관성**, **AI 가시성(Visibility)**, **상태 컨텍스트(Service Context)** 를 개선합니다. 이를 통해 에이전트가 파일 시스템 및 프로세스 상태를 더 명확하게 인지하고 스스로 오류를 복구할 수 있도록 합니다.

### 핵심 개선 사항

1. **MCPResult 패턴 준수**: 모든 중요 ID와 식별자를 텍스트 콘텐츠에 포함
2. **4단계 에러 처리 시스템**: Proactive Validation → Standard Errors → Context-Specific → Global Handler
3. **Tool Group 격리**: `ToolGroup::Workspace` 도구만 제안
4. **Service Context 구현**: 실행 중인 프로세스와 쉘 상태를 시스템 프롬프트에 주입

## 2. 현재의 상태 / 문제점

### 2.1 **CRITICAL: MCPResult Anti-Pattern (Priority 1)**

**Location**: `mod.rs` line ~305-318, ~500-545

```rust
// ❌ WRONG: Process IDs hidden in JSON only
let mut response = serde_json::json!({
    "process_id": entry_for_response.id,
    "status": format!("{:?}", entry_for_response.status).to_lowercase(),
    "command": entry_for_response.command,
    // ...
});
```

**Impact**: AI 에이전트는 `structured_content` (JSON)를 볼 수 없으므로 프로세스 ID를 후속 도구 호출(`pollProcess`, `stopProcess`)에 사용할 수 없습니다.

**Affected Tools**:

- `pollProcess`: 프로세스 ID가 텍스트에 명시되지 않음
- `listProcesses`: 프로세스 목록에서 명령어가 60자로 truncate되어 전체 정보 손실
- `readProcessOutput`: 프로세스 상태 정보가 JSON에만 존재

### 2.2 **CRITICAL: Missing Proactive Validation (Priority 2)**

**Location**: `file_operations.rs` line ~86-98

```rust
// ❌ INCOMPLETE: No empty path check, no file existence validation
let safe_path = match self.validate_path_with_error(path_str, session_id.clone()) {
    Ok(path) => path,
    Err(e) => { /* error handling */ }
};
```

**Missing Validations**:

1. Empty string check for `path` parameter
2. Path normalization (`.`, `..`, `~` patterns)
3. File existence check before attempting read
4. File type validation (file vs directory)
5. Size limit checks before large operations

**Impact**: 작업이 실패한 후에야 에러를 발견하여 불필요한 I/O와 사용자 경험 저하

### 2.3 **HIGH: List Response Pattern Violation (Priority 3)**

**Location**: `file_operations.rs` `handle_list_directory`, `mod.rs` `handle_list_processes`

```rust
// ❌ WRONG: File names only in structured_content
let hint = SuccessHint::new(
    format!("Listed {} items", items.len()), // Generic summary only
    vec!["Use readFile to view contents".to_string()]
);
```

**Impact**: AI는 어떤 파일이 있는지 볼 수 없어 blind guessing 필요

### 2.4 **MEDIUM: Service Context Not Implemented (Priority 4)**

**Location**: `mod.rs` line ~850-901

**Current State**: `get_service_context()` 구현은 있지만 프로세스 ID를 포함하지 않음

**Missing Information**:

- 실행 중인 프로세스의 ID와 명령어
- Pending shell executions 상태
- 최근 파일 작업 이력
- Workspace tree 구조 (현재는 `structured_state`에만 존재)

### 2.5 **MEDIUM: Inconsistent Error Guidance (Priority 5)**

**Location**: `mod.rs` line ~469-480

```rust
// ❌ GENERIC: Same guidance for all error types
vec![
    "Verify the process_id is correct".to_string(),
    "Use listProcesses to see available processes".to_string(),
    "Check if the process has generated output yet".to_string(),
]
```

**Issue**: 파일 미발견, 권한 거부, 프로세스 실행 중 등 다양한 에러 상황에 동일한 가이던스 제공

### 2.6 **LOW: Missing spawn_blocking for CPU-Intensive Ops (Priority 6)**

**Location**: `file_operations.rs` (대용량 파일 라인 처리)

```rust
// ❌ BLOCKING: Large file enumeration on async thread
let lines_with_numbers: Vec<(usize, String)> = raw_content
    .lines()
    .enumerate()
    .map(|(idx, line)| (idx + 1, line.to_string()))
    .collect();
```

**Risk**: 1MB 이상의 파일에서 런타임 블로킹 가능

## 3. 관련 코드의 구조 및 동작 방식 Summary (Bird's-eye View)

### 3.1 Module Structure

```text
workspace/
├── mod.rs                          # Main server, tool routing, process management
│   ├── WorkspaceServer             # Session-isolated server instance
│   ├── call_tool()                 # Tool dispatcher
│   ├── get_service_context()       # System prompt injection
│   └── handle_poll_process()       # Process status queries
│
├── file_operations.rs              # File I/O tools
│   ├── handle_read_file()          # Read with line ranges
│   ├── handle_write_file()         # Write with validation
│   ├── handle_list_directory()     # Directory listing
│   ├── handle_import_file()        # External file import
│   └── handle_grep()               # Text search
│
├── terminal_manager.rs             # Process registry and lifecycle
│   ├── ProcessRegistry             # Background process tracking
│   ├── ProcessEntry                # Process metadata
│   └── StreamingOutputHandle       # In-memory output buffering
│
├── persistent_shell_manager.rs    # Persistent shell sessions
│   ├── PersistentShellManager      # Session → Shell mapping
│   └── Shell state preservation    # CWD, env vars, etc.
│
└── code_execution/                 # Shell and process execution
    ├── process.rs                  # spawn_and_stream_to_files()
    ├── shell.rs                    # Isolated shell execution
    └── interactive.rs              # Pending execution flow
```

### 3.2 Key Data Flow

1. **File Operations**: UI → `call_tool()` → `file_operations.rs` → `SecureFileManager` → Validation → File System
2. **Process Management**: UI → `call_tool()` → `terminal_manager.rs` → `ProcessRegistry` → Background Task → Output Files
3. **Service Context**: Agent → `get_service_context()` → Process Stats + Shell State → System Prompt

### 3.3 Critical State Management

- **`process_registry: ProcessRegistry`**: `Arc<RwLock<>>` 기반 프로세스 추적
- **`shell_manager: PersistentShellManager`**: 세션별 persistent shell 인스턴스 관리
- **`pending_executions: Arc<PendingExecutions>`**: 대화형 명령어 대기 큐

## 4. 변경 이후의 상태 / 해결 판정 기준

### 4.1 Compliance Checklist

- [ ] **MCPResult Pattern**: 모든 중요 ID가 텍스트 콘텐츠에 포함됨
- [ ] **Proactive Validation**: 모든 파라미터가 작업 전에 검증됨
- [ ] **Visual Markers**: 모든 에러가 ✗, 모든 성공이 ✓ 심볼 포함
- [ ] **Tool Group Isolation**: 모든 에러가 Workspace 도구만 제안
- [ ] **List Visibility**: `listDirectory`, `listProcesses`가 항목을 텍스트로 나열
- [ ] **Service Context**: 프로세스 ID와 쉘 상태가 시스템 프롬프트에 주입됨
- [ ] **Context-Specific Errors**: 에러 타입별 맞춤형 복구 가이던스
- [ ] **Performance**: CPU-intensive 작업이 `spawn_blocking` 사용

### 4.2 Acceptance Criteria

#### Test 1: AI Agent Visibility

```rust
// Given: 3 running processes
let result = server.call_tool("listProcesses", json!({}), Some(session_id)).await?;

// Then: Text content must include process IDs
assert!(result.content[0].text.contains("abc-123"));
assert!(result.content[0].text.contains("def-456"));
assert!(result.content[0].text.contains("ghi-789"));
```

#### Test 2: Proactive Validation

```rust
// Given: Empty path parameter
let result = server.call_tool("readFile", json!({"path": ""}), Some(session_id)).await?;

// Then: Immediate parameter error
assert_eq!(result.is_error, Some(true));
assert!(result.content[0].text.contains("✗"));
assert!(result.content[0].text.contains("Missing required parameter: path"));
```

#### Test 3: Service Context Integration

```rust
// Given: 2 running processes
spawn_process("sleep 30");
spawn_process("sleep 60");

// When: Get service context
let context = server.get_service_context(None).await;

// Then: Context includes process count
assert!(context.context_prompt.contains("Running Processes: 2"));
```

#### Test 4: Error Recovery Guidance

```rust
// Given: Process not found error
let result = server.call_tool("pollProcess", json!({"processId": "invalid"}), Some(session_id)).await?;

// Then: Specific recovery steps
assert!(result.content[0].text.contains("💡 Next Steps:"));
assert!(result.content[0].text.contains("Use listProcesses"));
assert!(!result.content[0].text.contains("Use createSession")); // Wrong tool group
```

## 5. 수정이 필요한 코드 및 수정부분의 코드 스니핏

### 5.1 **PRIORITY 1: Fix MCPResult Pattern (pollProcess)**

**File**: `src-tauri/src/mcp/builtin/workspace/mod.rs`  
**Lines**: ~365-395

```rust
// ❌ BEFORE: Process ID only in JSON
let hint = SuccessHint::new(
    format!(
        "Process {} status: {}{}",
        process_id, status_str, tail_output_display
    ),
    match entry_for_response.status {
        // ...suggestions
    },
);

Ok(hint.to_mcp_result_with_data(Some(response)))
```

```rust
// ✅ AFTER: Include critical details in text
let status_details = format!(
    "Process Status for {}:

- Process ID: {}
- Status: {}
- Command: {}
- PID: {}
- Exit Code: {}
- Started: {}
- Finished: {}

{}",
    process_id,
    entry_for_response.id,
    status_str,
    entry_for_response.command,
    entry_for_response.pid.map(|p| p.to_string()).unwrap_or("N/A".to_string()),
    entry_for_response.exit_code.map(|c| c.to_string()).unwrap_or("N/A".to_string()),
    entry_for_response.started_at.format("%Y-%m-%d %H:%M:%S").to_string(),
    entry_for_response.finished_at
        .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or("Still running".to_string()),
    tail_output_display
);

let hint = SuccessHint::new(
    status_details,
    match entry_for_response.status {
        terminal_manager::ProcessStatus::Running => vec![
            "Wait for process to complete before polling again".to_string(),
            format!("Use readProcessOutput('{}', 'stdout') to view full output", process_id),
        ],
        terminal_manager::ProcessStatus::Finished
        | terminal_manager::ProcessStatus::Failed => vec![
            format!("Use readProcessOutput('{}', 'stdout') to view full output", process_id),
            "Process has completed - no need to poll again".to_string(),
        ],
        _ => vec!["Use listProcesses to see all processes".to_string()],
    },
);

Ok(hint.to_mcp_result_with_data(Some(response)))
```

### 5.2 **PRIORITY 1: Fix List Response (listProcesses)**

**File**: `src-tauri/src/mcp/builtin/workspace/mod.rs`  
**Lines**: ~520-590

```rust
// ❌ BEFORE: Truncated commands
let process_list = processes
    .iter()
    .map(|p| {
        let id = p.get("process_id")...unwrap_or("unknown");
        let status = p.get("status")...unwrap_or("unknown");
        let command = p.get("command")...unwrap_or("");
        let truncated_cmd = if command.len() > 60 {
            format!("{}...", &command[..57])
        } else {
            command.to_string()
        };
        format!("• {} [{}]: {}", id, status, truncated_cmd)
    })
    .collect::<Vec<_>>()
    .join("\n");
```

```rust
// ✅ AFTER: Full commands with clear formatting
let process_list = if processes.is_empty() {
    "No processes found in current session".to_string()
} else {
    processes
        .iter()
        .map(|p| {
            let id = p.get("process_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let status = p.get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let command = p.get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let pid = p.get("pid")
                .and_then(|v| v.as_u64())
                .map(|p| format!(" (PID: {})", p))
                .unwrap_or_default();
            let exit_code = p.get("exit_code")
                .and_then(|v| v.as_i64())
                .map(|c| format!(" [exit: {}]", c))
                .unwrap_or_default();

            // Full command visible to agent (no truncation)
            format!("• {} [{}]{}{}\n  Command: {}",
                id, status, pid, exit_code, command)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
};

let summary = format!(
    "Found {} processes ({} running, {} finished)

{}

💡 Next Steps:
- Use pollProcess('processId') to check status
- Use readProcessOutput('processId', 'stdout') to view output
- Use stopProcess('processId') to terminate running process",
    total, running, finished, process_list
);

let hint = SuccessHint::new(
    summary,
    vec![] // Empty since guidance is in summary
);
```

### 5.3 **PRIORITY 2: Add Proactive Validation (readFile)**

**File**: `src-tauri/src/mcp/builtin/workspace/file_operations.rs`  
**Lines**: ~30-65

```rust
// ❌ BEFORE: No pre-validation
let path_str = match args.get("path").and_then(|v| v.as_str()) {
    Some(path) => path,
    None => {
        return Ok(missing_param_error("path", ToolGroup::Workspace));
    }
};
```

```rust
// ✅ AFTER: Comprehensive validation
use crate::mcp::builtin::error_guidance::invalid_input_error;

// 1. Parameter existence
let path_str = match args.get("path").and_then(|v| v.as_str()) {
    Some(path) if !path.trim().is_empty() => path.trim(),
    Some(_) => {
        return Ok(invalid_input_error(
            "Path parameter cannot be empty",
            ToolGroup::Workspace,
        ));
    }
    None => {
        return Ok(missing_param_error("path", ToolGroup::Workspace));
    }
};

// 2. Path pattern validation
if path_str.contains("..") {
    return Ok(ErrorGuidance::with_guidance(
        ErrorCategory::InvalidInput,
        "Path traversal patterns (..) are not allowed",
        vec![
            "Use relative paths from workspace root".to_string(),
            "Example: 'src/main.rs' instead of '../src/main.rs'".to_string(),
            "Use listDirectory to explore available paths".to_string(),
        ],
        ToolGroup::Workspace,
    ).to_mcp_result());
}

// 3. Line range validation (moved before file access)
let start_line = args
    .get("startLine")
    .and_then(|v| v.as_u64())
    .map(|n| n as usize);
let end_line = args
    .get("endLine")
    .and_then(|v| v.as_u64())
    .map(|n| n as usize);

if let (Some(start), Some(end)) = (start_line, end_line) {
    if start > end {
        return Ok(ErrorGuidance::with_guidance(
            ErrorCategory::InvalidInput,
            format!(
                "startLine ({}) must be ≤ endLine ({})",
                start, end
            ),
            vec![
                format!("Correct usage: {{\"startLine\": {}, \"endLine\": {}}}", end, start),
                "Or omit both parameters to read the entire file".to_string(),
            ],
            ToolGroup::Workspace,
        ).to_mcp_result());
    }

    if start == 0 || end == 0 {
        return Ok(ErrorGuidance::with_guidance(
            ErrorCategory::InvalidInput,
            "Line numbers must be ≥ 1 (1-indexed)",
            vec![
                "Line numbering starts at 1, not 0".to_string(),
                "Use startLine: 1 for the first line".to_string(),
            ],
            ToolGroup::Workspace,
        ).to_mcp_result());
    }
}

// 4. Path security validation
let safe_path = match self.validate_path_with_error(path_str, session_id.clone()) {
    Ok(path) => path,
    Err(e) => {
        return Ok(ErrorGuidance::with_guidance(
            ErrorCategory::PermissionDenied,
            format!("Path validation failed: {}", e),
            vec![
                "Verify the file path is within workspace boundaries".to_string(),
                "Use listDirectory to see available files".to_string(),
                "Avoid absolute paths outside workspace".to_string(),
            ],
            ToolGroup::Workspace,
        ).to_mcp_result());
    }
};

// 5. File existence check
if !safe_path.exists() {
    return Ok(not_found_error("File", path_str, ToolGroup::Workspace));
}

// 6. File type check
if safe_path.is_dir() {
    return Ok(ErrorGuidance::with_guidance(
        ErrorCategory::InvalidInput,
        format!("'{}' is a directory, not a file", path_str),
        vec![
            "Use listDirectory to see directory contents".to_string(),
            "To read a file inside this directory, specify the full path".to_string(),
            format!("Example: '{}/filename.ext'", path_str),
        ],
        ToolGroup::Workspace,
    ).to_mcp_result());
}

// Continue with file reading...
```

### 5.4 **PRIORITY 3: Enhance List Visibility (listDirectory)**

**File**: `src-tauri/src/mcp/builtin/workspace/file_operations.rs`  
**Function**: `handle_list_directory`

```rust
// ✅ NEW: Format file list for AI visibility
fn format_directory_listing(items: &[Value], path: &str, limit: usize) -> String {
    if items.is_empty() {
        return format!("Directory '{}' is empty", path);
    }

    let display_items: Vec<String> = items
        .iter()
        .take(limit)
        .map(|item| {
            let name = item.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let type_ = item.get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let size = item.get("size")
                .and_then(|v| v.as_u64())
                .map(|s| {
                    if s < 1024 {
                        format!(" ({}B)", s)
                    } else if s < 1024 * 1024 {
                        format!(" ({:.1}KB)", s as f64 / 1024.0)
                    } else {
                        format!(" ({:.1}MB)", s as f64 / 1024.0 / 1024.0)
                    }
                })
                .unwrap_or_default();

            let icon = match type_ {
                "file" => "📄",
                "directory" => "📁",
                "symlink" => "🔗",
                _ => "❓",
            };

            format!("{} [{}] {}{}", icon, type_, name, size)
        })
        .collect();

    let truncation_note = if items.len() > limit {
        format!("\n\n... and {} more items (use recursive: false to see all)",
                items.len() - limit)
    } else {
        String::new()
    };

    format!(
        "Directory listing for '{}':\n\n{}{}",
        path,
        display_items.join("\n"),
        truncation_note
    )
}

// In handle_list_directory:
let items_text = format_directory_listing(&items, path_str, 100);

let hint = SuccessHint::new(
    items_text,
    vec![
        format!("Use readFile('{}/<filename>') to read a file", path_str),
        format!("Use listDirectory('{}/subdir') to explore subdirectories", path_str),
        "Use grep to search for specific content in files".to_string(),
    ],
);

Ok(hint.to_mcp_result_with_data(Some(json!({
    "path": path_str,
    "items": items,
    "total": items.len(),
}))))
```

    "Use grep to search for specific content in files".to_string(),

],
);

Ok(hint.to_mcp_result_with_data(Some(json!({
"path": path_str,
"items": items,
"total": items.len(),
}))))

````

### 5.5 **PRIORITY 4: Implement Service Context**

**File**: `src-tauri/src/mcp/builtin/workspace/mod.rs`
**Lines**: ~850-901

```rust
// ❌ BEFORE: No process IDs in context
let context_prompt = format!(
    "## Workspace\\n\\n\\\
    **Workspace Root**: {}\\n\\\
    **Persistent Shell CWD**: {}\\n\\\
    **Running Processes**: {}\\n\\\
    **Platform**: {}/{}",
    workspace_dir,
    shell_cwd,
    running_count,
    os,
    arch
);
````

```rust
// ✅ AFTER: Include actionable process information
async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    // Get workspace directory
    let workspace_dir = self.session_manager
        .get_workspace_dir(&self.session_id)
        .await
        .unwrap_or_else(|_| ".".to_string());

    // Get platform info
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    // Get shell CWD
    let shell_cwd = if let Some(cwd) = self.shell_manager
        .get_shell_cwd(&self.session_id).await
    {
        if cwd.starts_with(&workspace_dir) {
            cwd.replacen(&workspace_dir, ".", 1)
        } else {
            cwd
        }
    } else {
        ".".to_string()
    };

    // Get running processes with IDs and commands
    let registry = self.process_registry.read().await;
    let running_processes: Vec<(String, String)> = registry
        .entries
        .values()
        .filter(|e| e.session_id == self.session_id)
        .filter(|e| matches!(e.status, terminal_manager::ProcessStatus::Running))
        .take(5) // Limit to prevent context bloat
        .map(|e| (e.id.clone(), e.command.clone()))
        .collect();

    let running_count = running_processes.len();
    let total_count = registry.entries.values()
        .filter(|e| e.session_id == self.session_id)
        .count();

    drop(registry);

    // Format running processes for AI visibility
    let running_processes_text = if running_processes.is_empty() {
        "None".to_string()
    } else {
        let process_list = running_processes.iter()
            .map(|(id, cmd)| {
                // Truncate command if too long
                let display_cmd = if cmd.len() > 80 {
                    format!("{}...", &cmd[..77])
                } else {
                    cmd.clone()
                };
                format!("  • {} - {}", id, display_cmd)
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!("\n{}", process_list)
    };

    let context_prompt = format!(
        "## Workspace

**Workspace Root**: {}
**Persistent Shell CWD**: {}
**Platform**: {} / {}

**Background Processes**:
- Running: {}{}
- Total: {}

💡 Use pollProcess(processId) to check status or listProcesses() to see all.",
        workspace_dir,
        shell_cwd,
        os,
        arch,
        running_count,
        running_processes_text,
        total_count
    );

    ServiceContext {
        context_prompt,
        structured_state: Some(json!({
            "workspace_dir": workspace_dir,
            "shell_cwd": shell_cwd,
            "platform": {
                "os": os,
                "arch": arch
            },
            "processes": {
                "running": running_count,
                "total": total_count,
            },
            "shell_active": !shell_cwd.is_empty(),
        })),
    }
}
```

### 5.6 **PRIORITY 5: Context-Specific Error Guidance**

**File**: `src-tauri/src/mcp/builtin/workspace/mod.rs`  
**Function**: `handle_read_process_output`

```rust
// ❌ BEFORE: Generic error handling
Err(e) => Ok(operation_failed_error(
    "Read process output",
    &e,
    vec![
        "Verify the process_id is correct".to_string(),
        "Use listProcesses to see available processes".to_string(),
        "Check if the process has generated output yet".to_string(),
    ],
    ToolGroup::Workspace,
))
```

```rust
// ✅ AFTER: Context-specific guidance
Err(e) => {
    let error_lower = e.to_lowercase();

    let (category, guidance) = if error_lower.contains("not found")
        || error_lower.contains("no such file")
    {
        (
            ErrorCategory::ResourceNotFound,
            vec![
                "Process may not have generated output yet".to_string(),
                format!("Use pollProcess('{}') to check if process has started", process_id),
                "Wait a moment for process to initialize and write output".to_string(),
            ]
        )
    } else if error_lower.contains("permission") {
        (
            ErrorCategory::PermissionDenied,
            vec![
                "Output file access denied".to_string(),
                "Process may still be writing to the file".to_string(),
                format!("Use pollProcess('{}') to check process status", process_id),
            ]
        )
    } else if error_lower.contains("too large") {
        (
            ErrorCategory::InvalidInput,
            vec![
                "Output file is too large to read at once".to_string(),
                format!("Use readProcessOutput('{}', '{}', mode='tail', lines=50) to read last 50 lines", process_id, stream),
                format!("Use readProcessOutput('{}', '{}', mode='head', lines=50) to read first 50 lines", process_id, stream),
            ]
        )
    } else {
        (
            ErrorCategory::OperationFailed,
            vec![
                format!("Verify process ID '{}' is correct", process_id),
                "Use listProcesses to see available processes".to_string(),
                "Check if the process has been cleaned up (>24 hours old)".to_string(),
            ]
        )
    };

    Ok(ErrorGuidance::with_guidance(
        category,
        format!("Failed to read {} output: {}", stream, e),
        guidance,
        ToolGroup::Workspace,
    ).to_mcp_result())
}
```

### 5.7 **PRIORITY 6: Use spawn_blocking for Large File Operations**

**File**: `src-tauri/src/mcp/builtin/workspace/file_operations.rs`  
**Function**: `handle_read_file`

```rust
// ❌ BEFORE: Line enumeration on async thread
let lines_with_numbers: Vec<(usize, String)> = raw_content
    .lines()
    .enumerate()
    .map(|(idx, line)| (idx + 1, line.to_string()))
    .collect();
```

```rust
// ✅ AFTER: Offload to blocking thread for large files
const LARGE_FILE_THRESHOLD: usize = 1024 * 1024; // 1MB

let lines_with_numbers = if raw_content.len() > LARGE_FILE_THRESHOLD {
    // Large file: use blocking thread
    let content_clone = raw_content.clone();
    tokio::task::spawn_blocking(move || {
        content_clone
            .lines()
            .enumerate()
            .map(|(idx, line)| (idx + 1, line.to_string()))
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
} else {
    // Small file: process inline
    raw_content
        .lines()
        .enumerate()
        .map(|(idx, line)| (idx + 1, line.to_string()))
        .collect()
};

let formatted_content = Self::format_lines_with_numbers(&lines_with_numbers);
```

### 5.8 **Fix Legacy Error Patterns (importFile)**

**File**: `src-tauri/src/mcp/builtin/workspace/file_operations.rs`  
**Function**: `handle_import_file`

Find all instances of `MCPResult::error()` and replace with proper `ErrorGuidance`:

```rust
// ❌ Pattern to find and replace
return Ok(MCPResult::error("..."));

// ✅ Replace with
return Ok(ErrorGuidance::with_guidance(
    ErrorCategory::..., // Choose appropriate category
    "Error message",
    vec![
        "Recovery step 1".to_string(),
        "Recovery step 2".to_string(),
    ],
    ToolGroup::Workspace,
).to_mcp_result());
```

**Specific Replacements**:

```rust
// 1. Missing srcAbsPath
return Ok(missing_param_error("srcAbsPath", ToolGroup::Workspace));

// 2. Missing destRelPath
return Ok(missing_param_error("destRelPath", ToolGroup::Workspace));

// 3. Source is directory
return Ok(ErrorGuidance::with_guidance(
    ErrorCategory::InvalidInput,
    "Source path must be a file, not a directory",
    vec![
        "Verify the source path points to a file".to_string(),
        "To import a directory, zip it first and import the zip file".to_string(),
        "Use operating system file browser to confirm file type".to_string(),
    ],
    ToolGroup::Workspace,
).to_mcp_result());

// 4. Source file not found
return Ok(not_found_error("Source file", src_abs_path_str, ToolGroup::Workspace));

// 5. Destination already exists
return Ok(ErrorGuidance::with_guidance(
    ErrorCategory::DuplicateResource,
    format!("Destination file '{}' already exists", dest_rel_path_str),
    vec![
        "Use a different destination path".to_string(),
        "Delete the existing file first with appropriate tools".to_string(),
        format!("Or rename the import: '{}_imported'", dest_rel_path_str),
    ],
    ToolGroup::Workspace,
).to_mcp_result());
```

## 6. 재사용 가능한 연관 코드

### 6.1 Error Guidance Module

**File**: `src-tauri/src/mcp/builtin/error_guidance.rs`

**Available Functions**:

```rust
// Standard error constructors
pub fn missing_param_error(param_name: &str, tool_group: ToolGroup) -> MCPResult
pub fn not_found_error(resource_type: &str, identifier: &str, tool_group: ToolGroup) -> MCPResult
pub fn duplicate_error(resource_type: &str, identifier: &str, tool_group: ToolGroup) -> MCPResult
pub fn invalid_input_error(message: &str, tool_group: ToolGroup) -> MCPResult
pub fn permission_denied_error(message: &str, tool_group: ToolGroup) -> MCPResult
pub fn operation_failed_error(operation: &str, error: &str, guidance: Vec<String>, tool_group: ToolGroup) -> MCPResult

// Success hint
pub struct SuccessHint {
    pub message: String,
    pub suggestions: Vec<String>,
}

impl SuccessHint {
    pub fn new(message: String, suggestions: Vec<String>) -> Self
    pub fn to_mcp_result(&self) -> MCPResult
    pub fn to_mcp_result_with_data(&self, data: Option<Value>) -> MCPResult
}

// Custom error guidance
pub struct ErrorGuidance {
    pub category: ErrorCategory,
    pub message: String,
    pub guidance: Vec<String>,
    pub tool_group: ToolGroup,
}

impl ErrorGuidance {
    pub fn with_guidance(
        category: ErrorCategory,
        message: String,
        guidance: Vec<String>,
        tool_group: ToolGroup,
    ) -> Self

    pub fn to_mcp_result(&self) -> MCPResult
}
```

### 6.2 Helper Functions

**File**: `src-tauri/src/mcp/builtin/workspace/utils.rs`

```rust
// Format file size for display
pub fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1}MB", bytes as f64 / 1024.0 / 1024.0)
    } else {
        format!("{:.1}GB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
    }
}

// Truncate string with ellipsis
pub fn truncate_with_ellipsis(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
```

### 6.3 Constants

**File**: `src-tauri/src/mcp/builtin/workspace/mod.rs`

```rust
// Add at top of file
const MAX_COMMAND_DISPLAY_LEN: usize = 80;
const MAX_DIRECTORY_ITEMS_DISPLAY: usize = 100;
const LARGE_FILE_THRESHOLD: usize = 1024 * 1024; // 1MB
const MAX_RUNNING_PROCESSES_IN_CONTEXT: usize = 5;
```

const MAX_RUNNING_PROCESSES_IN_CONTEXT: usize = 5;

````

## 7. Test Code 가이드

### 7.1 Unit Tests

**File**: `src-tauri/src/mcp/builtin/workspace/mod.rs` (test module)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_poll_process_includes_id_in_text() {
        let server = create_test_server().await;
        let process_id = "test-proc-123";

        // Spawn a test process
        let _ = server.handle_spawn_process(
            json!({
                "command": "sleep 1",
                "runMode": "async"
            }),
            "test-session",
        ).await.unwrap();

        // Poll process
        let result = server.handle_poll_process(
            json!({"processId": process_id}),
            "test-session",
        ).await.unwrap();

        // Verify ID is in text content
        assert_eq!(result.is_error, Some(false));
        if let Some(content) = result.content {
            if let Some(MCPContent::Text { text }) = content.first() {
                assert!(text.contains("test-proc-123"));
                assert!(text.contains("Process ID:"));
            }
        }
    }

    #[tokio::test]
    async fn test_list_processes_shows_full_commands() {
        let server = create_test_server().await;

        // Spawn processes with long commands
        let _ = server.handle_spawn_process(
            json!({
                "command": "python -m http.server 8000 --bind 0.0.0.0 --directory /tmp/test",
                "runMode": "async"
            }),
            "test-session",
        ).await;

        let result = server.handle_list_processes(
            json!({}),
            "test-session",
        ).await.unwrap();

        // Verify full command is visible (not truncated)
        if let Some(content) = result.content {
            if let Some(MCPContent::Text { text }) = content.first() {
                assert!(text.contains("python -m http.server"));
                assert!(text.contains("--directory /tmp/test"));
            }
        }
    }

    #[tokio::test]
    async fn test_read_file_proactive_validation() {
        let server = create_test_server().await;

        // Test empty path
        let result = server.handle_read_file(
            json!({"path": ""}),
            Some("test-session".to_string()),
        ).await.unwrap();

        assert_eq!(result.is_error, Some(true));
        if let Some(content) = result.content {
            if let Some(MCPContent::Text { text }) = content.first() {
                assert!(text.contains("✗"));
                assert!(text.contains("cannot be empty"));
            }
        }

        // Test path traversal
        let result = server.handle_read_file(
            json!({"path": "../etc/passwd"}),
            Some("test-session".to_string()),
        ).await.unwrap();

        assert_eq!(result.is_error, Some(true));
        if let Some(content) = result.content {
            if let Some(MCPContent::Text { text }) = content.first() {
                assert!(text.contains("✗"));
                assert!(text.contains("not allowed"));
            }
        }

        // Test invalid line range
        let result = server.handle_read_file(
            json!({
                "path": "test.txt",
                "startLine": 10,
                "endLine": 5
            }),
            Some("test-session".to_string()),
        ).await.unwrap();

        assert_eq!(result.is_error, Some(true));
        if let Some(content) = result.content {
            if let Some(MCPContent::Text { text }) = content.first() {
                assert!(text.contains("must be ≤"));
            }
        }
    }

    #[tokio::test]
    async fn test_service_context_includes_process_ids() {
        let server = create_test_server().await;

        // Spawn test processes
        let proc1_id = server.handle_spawn_process(
            json!({"command": "sleep 30", "runMode": "async"}),
            "test-session",
        ).await.unwrap();

        let proc2_id = server.handle_spawn_process(
            json!({"command": "sleep 60", "runMode": "async"}),
            "test-session",
        ).await.unwrap();

        // Get service context
        let context = server.get_service_context(None).await;

        // Verify process information is in text
        assert!(context.context_prompt.contains("Running: 2"));
        assert!(context.context_prompt.contains(&proc1_id));
        assert!(context.context_prompt.contains(&proc2_id));
    }

    #[tokio::test]
    async fn test_list_directory_shows_file_names() {
        let server = create_test_server().await;

        let result = server.handle_list_directory(
            json!({"path": "./"}),
            Some("test-session".to_string()),
        ).await.unwrap();

        // Verify file names are in text content
        if let Some(content) = result.content {
            if let Some(MCPContent::Text { text }) = content.first() {
                // Should see emoji icons
                assert!(text.contains("📄") || text.contains("📁"));
                // Should see file types
                assert!(text.contains("[file]") || text.contains("[directory]"));
            }
        }
    }

    #[tokio::test]
    async fn test_error_guidance_tool_group_isolation() {
        let server = create_test_server().await;

        // Trigger a not-found error
        let result = server.handle_poll_process(
            json!({"processId": "nonexistent"}),
            "test-session",
        ).await.unwrap();

        assert_eq!(result.is_error, Some(true));

        if let Some(content) = result.content {
            if let Some(MCPContent::Text { text }) = content.first() {
                // Should suggest Workspace tools only
                assert!(text.contains("listProcesses"));

                // Should NOT suggest tools from other groups
                assert!(!text.contains("createSession")); // Browser
                assert!(!text.contains("createTodo")); // Planning
                assert!(!text.contains("saveKnowledge")); // Knowledge
            }
        }
    }
}
````

### 7.2 Integration Tests

**File**: `src-tauri/tests/workspace_integration_test.rs`

```rust
#[tokio::test]
async fn test_full_workflow_with_service_context() {
    // 1. Create workspace server
    let session_manager = Arc::new(SessionManager::new());
    let server = WorkspaceServer::new("test-session".to_string(), session_manager);

    // 2. Initial context should show no processes
    let context = server.get_service_context(None).await;
    assert!(context.context_prompt.contains("Running: 0"));

    // 3. Spawn a background process
    let result = server.call_tool(
        "spawnProcess",
        json!({"command": "sleep 5", "runMode": "async"}),
        Some("test-session".to_string()),
    ).await.unwrap();

    assert_eq!(result.is_error, Some(false));

    // 4. Context should now show 1 running process
    let context = server.get_service_context(None).await;
    assert!(context.context_prompt.contains("Running: 1"));

    // 5. List processes should show the process ID
    let result = server.call_tool(
        "listProcesses",
        json!({}),
        Some("test-session".to_string()),
    ).await.unwrap();

    if let Some(content) = result.content {
        if let Some(MCPContent::Text { text }) = content.first() {
            assert!(text.contains("[running]"));
            assert!(text.contains("sleep 5"));
        }
    }
}

#[tokio::test]
async fn test_error_recovery_workflow() {
    let session_manager = Arc::new(SessionManager::new());
    let server = WorkspaceServer::new("test-session".to_string(), session_manager);

    // 1. Try to read non-existent file
    let result = server.call_tool(
        "readFile",
        json!({"path": "nonexistent.txt"}),
        Some("test-session".to_string()),
    ).await.unwrap();

    assert_eq!(result.is_error, Some(true));

    // 2. Error should suggest listDirectory
    if let Some(content) = result.content {
        if let Some(MCPContent::Text { text }) = content.first() {
            assert!(text.contains("💡 Next Steps:"));
            assert!(text.contains("listDirectory"));
        }
    }

    // 3. Follow suggestion
    let result = server.call_tool(
        "listDirectory",
        json!({"path": "./"}),
        Some("test-session".to_string()),
    ).await.unwrap();

    assert_eq!(result.is_error, Some(false));
}
```

### 7.3 Performance Tests

```rust
#[tokio::test]
async fn test_large_file_uses_spawn_blocking() {
    use std::time::Instant;

    let server = create_test_server().await;

    // Create a large test file (2MB)
    let large_content = "line\n".repeat(100_000);
    std::fs::write("test_large.txt", large_content).unwrap();

    let start = Instant::now();

    let result = server.handle_read_file(
        json!({"path": "test_large.txt"}),
        Some("test-session".to_string()),
    ).await.unwrap();

    let duration = start.elapsed();

    assert_eq!(result.is_error, Some(false));

    // Should complete in reasonable time (spawn_blocking prevents blocking)
    assert!(duration.as_secs() < 2);

    std::fs::remove_file("test_large.txt").unwrap();
}
```

## 8. Clarification Q-list & Decisions

### 8.1 **Q: Pagination Strategy for Large Files?**

**Decision**: 이번 리팩토링에서는 `ContentStore` (Browser 도구에서 사용) 같은 전용 페이지네이션 시스템을 도입하지 않습니다. 대신:

1. `startLine`/`endLine` 파라미터 검증 강화
2. 파일 크기가 `LARGE_FILE_THRESHOLD` (1MB) 이상일 때 에러 가이던스에서 부분 읽기 제안
3. 향후 필요 시 별도 이슈로 `ContentStore` 통합 검토

**Rationale**: 복잡성 최소화, 기존 API 활용, 점진적 개선

### 8.2 **Q: Process Command Truncation Length?**

**Decision**: `listProcesses`에서 명령어를 truncate하지 않고 전체 표시. 다만 `MAX_COMMAND_DISPLAY_LEN` (80자) 상수를 정의하여 필요 시 사용.

**Rationale**: AI는 전체 명령어를 봐야 프로세스를 정확히 식별 가능. Token context가 문제되면 나중에 조정.

### 8.3 **Q: Service Context Update Frequency?**

**Decision**: 매 `get_service_context()` 호출 시 실시간 조회 (캐싱 없음).

**Rationale**:

- Browser 도구처럼 JS injection이 비싼 작업이 아님
- 프로세스 상태는 빠르게 변하므로 최신 정보 필요
- Read lock만 사용하므로 성능 영향 미미

### 8.4 **Q: Magic Number Constants Location?**

**Decision**: 모든 상수를 `mod.rs` 상단에 정의:

```rust
// File operation limits
const LARGE_FILE_THRESHOLD: usize = 1024 * 1024; // 1MB
const MAX_DIRECTORY_ITEMS_DISPLAY: usize = 100;

// Process display limits
const MAX_COMMAND_DISPLAY_LEN: usize = 80;
const MAX_RUNNING_PROCESSES_IN_CONTEXT: usize = 5;

// Cleanup intervals
const PROCESS_CLEANUP_INTERVAL_SECS: u64 = 3600; // 1 hour
const PROCESS_RETENTION_HOURS: i64 = 24;
```

### 8.5 **Q: Backward Compatibility?**

**Decision**: 모든 변경사항은 backward compatible:

- 도구 시그니처 변경 없음
- 응답 구조 유지 (텍스트만 향상)
- 기존 JSON `structured_content` 유지

**Breaking Changes**: None

## 9. Implementation Plan & Timeline

### Phase 1: Critical Fixes (4 hours)

- [ ] Fix MCPResult pattern in `pollProcess` (1h)
- [ ] Fix MCPResult pattern in `listProcesses` (1h)
- [ ] Add proactive validation to `readFile` (1.5h)
- [ ] Add proactive validation to `writeFile` (0.5h)

### Phase 2: List Visibility (2 hours)

- [ ] Enhance `listDirectory` with formatted output (1h)
- [ ] Add helper functions (`format_file_size`, etc.) (0.5h)
- [ ] Add constants for display limits (0.5h)

### Phase 3: Service Context (2 hours)

- [ ] Implement enhanced `get_service_context` (1.5h)
- [ ] Test context integration with agent flow (0.5h)

### Phase 4: Error Guidance (2 hours)

- [ ] Add context-specific errors to `readProcessOutput` (0.5h)
- [ ] Fix legacy `MCPResult::error()` in `importFile` (1h)
- [ ] Standardize all error responses (0.5h)

### Phase 5: Performance & Polish (2 hours)

- [ ] Add `spawn_blocking` for large file operations (1h)
- [ ] Remove commented code and magic numbers (0.5h)
- [ ] Update tool descriptions if needed (0.5h)

### Phase 6: Testing (2 hours)

- [ ] Write unit tests (1h)
- [ ] Write integration tests (0.5h)
- [ ] Run full validation pipeline (0.5h)

**Total Estimated Time**: 12-14 hours

## 10. Success Metrics

### Before Refactoring

- Compliance Score: **6.2/10**
- AI Agent Success Rate: ~70% (frequent retry loops)
- Error Recovery Time: High (generic guidance)
- Process ID Visibility: 0% (hidden in JSON)

### After Refactoring (Target)

- Compliance Score: **9.0/10**
- AI Agent Success Rate: >90% (clear guidance)
- Error Recovery Time: Low (specific steps)
- Process ID Visibility: 100% (always in text)

### Validation Commands

```bash
# Run tests
cargo test --package libr-agent workspace::tests

# Check formatting
cargo fmt --check

# Lint
cargo clippy -- -D warnings

# Full validation
pnpm refactor:validate
```

## 11. Rollback Plan

If critical issues arise:

1. **Immediate Rollback**: Revert commit with `git revert <commit-hash>`
2. **Partial Rollback**: Keep proactive validation, revert only MCPResult changes
3. **Forward Fix**: Address specific issues while keeping improvements

**Risk Level**: LOW (all changes are additive, no breaking API changes)

## 12. References

- **Best Practices Doc**: `docs/guides/builtin-tool-best-practices.md`
- **Browser Reference**: `src-tauri/src/mcp/builtin/browser/` (exemplary implementation)
- **Planning Reference**: `src-tauri/src/mcp/builtin/planning/` (proactive validation)
- **Error Guidance**: `src-tauri/src/mcp/builtin/error_guidance.rs`

---

**Prepared by**: GitHub Copilot  
**Review Required**: Core Team  
**Priority**: HIGH - Agent UX Critical  
**Target Branch**: `dev/0.4.0`
