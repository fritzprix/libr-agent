# Refactoring Plan: Async Execute Shell Support

**Date**: 2025-10-10  
**Author**: Development Team  
**Status**: Planning Phase  
**Version**: 1.0

---

## 📋 작업의 목적

Workspace MCP 서버의 `execute_shell` 도구에 비동기/백그라운드 실행 모드를 추가하여, LLM Agent가 장시간 실행되는 명령어(서버 시작, 파일 감시, 빌드 프로세스 등)를 non-blocking 방식으로 실행하고 모니터링할 수 있도록 개선합니다.

### 핵심 목표

1. **비동기 실행 지원**: 명령어를 백그라운드에서 실행하고 즉시 제어권 반환
2. **프로세스 모니터링**: 실행 중인 프로세스의 상태 및 출력을 조회할 수 있는 API 제공
3. **출력 스트리밍**: stdout/stderr를 파일로 스트리밍하여 대용량 출력 처리
4. **세션 격리**: 각 세션의 프로세스를 독립적으로 관리

### 사용 시나리오

```
Agent: execute_shell("npm run dev", run_mode: "async")
→ Response: { process_id: "c1a2b3", status: "starting" }

Agent: poll_process("c1a2b3", tail: {src: "stdout", n: 20})
→ Response: { status: "running", tail: ["Server started...", ...] }

Agent: read_process_output("c1a2b3", stream: "stderr", lines: 50)
→ Response: { content: ["Error log 1", "Error log 2", ...] }
```

---

## 🔍 현재의 상태 / 문제점

### 현재 동작 방식

`execute_shell` 도구는 **동기(synchronous) 실행 모드만** 지원합니다:

```rust
// src-tauri/src/mcp/builtin/workspace/code_execution.rs
pub async fn handle_execute_shell(&self, args: Value) -> MCPResponse {
    // 1. Parse command and timeout
    // 2. Create isolated command
    // 3. Execute with timeout: timeout(duration, cmd.output()).await
    // 4. Wait for completion (blocking)
    // 5. Return stdout/stderr in response
}
```

**호출 흐름**:

```
MCP Client → WorkspaceServer::call_tool("execute_shell")
    → handle_execute_shell()
        → execute_shell_with_isolation()
            → isolation_manager.create_isolated_command()
            → timeout(duration, cmd.output()).await  // 블로킹 대기
            → return MCPResponse with stdout/stderr
```

### 문제점

#### 1. **장시간 실행 명령어 처리 불가**

- 서버 프로세스, watch 모드, 빌드 등은 완료까지 수 분~수 시간 소요
- Agent는 응답을 받기 전까지 블로킹되어 다른 작업 불가
- 타임아웃 발생 시 프로세스 강제 종료

#### 2. **출력 확인 불가**

- 실행 완료 후에만 출력 확인 가능
- 실행 중 진행 상황, 에러 로그를 실시간으로 볼 수 없음
- 디버깅 및 모니터링 어려움

#### 3. **프로세스 관리 기능 부재**

- 실행 중인 프로세스 목록 조회 불가
- 프로세스 상태(실행 중, 완료, 실패) 확인 불가
- 수동 종료/재시작 불가

#### 4. **대용량 출력 처리 문제**

- 모든 stdout/stderr를 메모리에 버퍼링 (`cmd.output().await`)
- 대용량 로그 출력 시 메모리 초과 위험
- MCP 응답 크기 제한 초과 가능

---

## 🏗️ 관련 코드의 구조 및 동작 방식 (Bird's Eye View)

### 현재 아키텍처

```
┌─────────────────────────────────────────────────────────────┐
│                    MCP Client (Agent)                        │
└───────────────────────┬─────────────────────────────────────┘
                        │ call_tool("execute_shell", args)
                        ▼
┌─────────────────────────────────────────────────────────────┐
│              WorkspaceServer (mod.rs)                        │
│  - session_manager: Arc<SessionManager>                     │
│  - isolation_manager: SessionIsolationManager               │
│  - tools(): Vec<MCPTool>                                    │
│  - call_tool(name, args) → MCPResponse                      │
└───────────────────────┬─────────────────────────────────────┘
                        │ handle_execute_shell(args)
                        ▼
┌─────────────────────────────────────────────────────────────┐
│         code_execution.rs                                    │
│  - handle_execute_shell()                                   │
│      ├─ parse args (command, timeout, isolation)            │
│      ├─ normalize_shell_command()                           │
│      └─ execute_shell_with_isolation()                      │
│            ├─ build IsolatedProcessConfig                   │
│            ├─ create_isolated_command()                     │
│            ├─ timeout(duration, cmd.output()).await         │
│            └─ return stdout/stderr                          │
└───────────────────────┬─────────────────────────────────────┘
                        │ create_isolated_command(config)
                        ▼
┌─────────────────────────────────────────────────────────────┐
│       session_isolation.rs (SessionIsolationManager)         │
│  - create_isolated_command(config) → AsyncCommand           │
│      ├─ Basic: env vars, working dir                        │
│      ├─ Medium: process groups, resource limits             │
│      └─ High: unshare/sandbox-exec/job objects              │
└─────────────────────────────────────────────────────────────┘
```

### 주요 컴포넌트

#### 1. **WorkspaceServer** (`mod.rs`)

- Workspace MCP 서버의 메인 구조체
- 도구 라우팅 및 세션 관리
- **현재**: `process_registry` 없음 (추가 필요)

#### 2. **code_execution.rs**

- `execute_shell` 도구의 핵심 로직
- **현재**: 동기 실행만 지원
- **변경 필요**: `run_mode` 파라미터 추가, 비동기 spawn 로직 추가

#### 3. **SessionIsolationManager** (`session_isolation.rs`)

- 플랫폼별 프로세스 격리 및 샌드박싱
- `tokio::process::Command` (AsyncCommand) 생성
- **변경 불필요**: 현재 API 그대로 사용 가능

#### 4. **tools/** 디렉토리

- MCP 도구 스키마 정의
- **현재**: `code_tools.rs`에 `execute_shell` 정의
- **추가 필요**: `terminal_tools.rs` (신규 도구들)

---

## 🎯 변경 이후의 상태 / 해결 판정 기준

### 목표 상태

#### 1. **execute_shell 확장**

- ✅ `run_mode: "sync"` (기본): 기존 동작 유지
- ✅ `run_mode: "async"`: 즉시 process_id 반환, 백그라운드 실행
- ✅ `run_mode: "background"`: async와 동일 (명시적 표현)

#### 2. **신규 도구 추가**

- ✅ `poll_process`: 프로세스 상태 조회 + optional tail
- ✅ `read_process_output`: stdout/stderr 읽기 (텍스트, 최대 100줄)
- ✅ `list_processes`: 세션 내 프로세스 목록

#### 3. **프로세스 레지스트리**

- ✅ 실행 중/완료된 프로세스 메타데이터 저장
- ✅ 세션별 격리 (다른 세션 접근 불가)
- ✅ 출력 파일 경로 관리

#### 4. **출력 스트리밍**

- ✅ stdout/stderr를 `tmp/process_{id}/` 디렉토리의 파일로 저장
- ✅ 실시간 append (메모리 버퍼링 없음)
- ✅ 파일 크기 추적

### 성공 판정 기준

#### 기능 검증

1. **비동기 실행**

   ```bash
   # Test: 장시간 실행 명령어
   execute_shell("sleep 60", run_mode: "async")
   → 즉시 process_id 반환 (< 1초)
   → 백그라운드에서 실행 중
   ```

2. **상태 모니터링**

   ```bash
   poll_process(process_id)
   → status: "running", pid: 12345, started_at: "..."

   # 60초 후
   poll_process(process_id)
   → status: "finished", exit_code: 0
   ```

3. **출력 조회**

   ```bash
   execute_shell("echo 'line1'; echo 'line2'", run_mode: "async")
   poll_process(process_id, tail: {src: "stdout", n: 10})
   → tail: ["line1", "line2"]

   read_process_output(process_id, stream: "stdout", lines: 50)
   → content: ["line1", "line2"]
   ```

4. **세션 격리**

   ```bash
   # Session A에서 실행
   execute_shell("echo 'session A'", run_mode: "async") → process_id: "p1"

   # Session B에서 조회 시도
   poll_process("p1") → Error: "Process not found or access denied"
   ```

#### 비기능 검증

- ✅ 메모리 사용량: 대용량 출력(10MB+)에서도 안정적
- ✅ 동시 실행: 10개 이상의 프로세스 동시 실행 가능
- ✅ 보안: 세션 간 프로세스 격리 유지
- ✅ 에러 처리: spawn 실패, 권한 부족 등 적절히 처리

---

## 🔧 수정이 필요한 코드 및 수정 부분

### 1. 신규 파일 생성

#### **`src-tauri/src/mcp/builtin/workspace/terminal_manager.rs`** (신규)

**목적**: 프로세스 레지스트리 및 출력 읽기 헬퍼

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

/// Process status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProcessStatus {
    Starting,   // Spawning in progress
    Running,    // Actively running
    Finished,   // Completed successfully
    Failed,     // Exited with error
    Killed,     // Terminated by user/system
}

/// Process metadata entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessEntry {
    pub id: String,
    pub session_id: String,
    pub command: String,
    pub status: ProcessStatus,
    pub pid: Option<u32>,
    pub exit_code: Option<i32>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub stdout_path: String,
    pub stderr_path: String,
    pub stdout_size: u64,
    pub stderr_size: u64,
}

/// Thread-safe process registry
pub type ProcessRegistry = Arc<RwLock<HashMap<String, ProcessEntry>>>;

/// Create a new process registry
pub fn create_process_registry() -> ProcessRegistry {
    Arc::new(RwLock::new(HashMap::new()))
}

/// Read last N lines from file (max 100, text only)
pub async fn tail_lines(
    file_path: &PathBuf,
    n: usize,
) -> Result<Vec<String>, String> {
    let n = n.min(100); // enforce max

    if !file_path.exists() {
        return Ok(Vec::new());
    }

    // Read file as UTF-8 (lossy conversion for non-UTF8)
    let content = tokio::fs::read_to_string(file_path)
        .await
        .map_err(|e| format!("Failed to read file: {}", e))?;

    // Get last N lines in reverse order, then reverse back
    let lines: Vec<String> = content
        .lines()
        .rev()
        .take(n)
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    Ok(lines)
}

/// Read first N lines from file (max 100, text only)
pub async fn head_lines(
    file_path: &PathBuf,
    n: usize,
) -> Result<Vec<String>, String> {
    let n = n.min(100); // enforce max

    if !file_path.exists() {
        return Ok(Vec::new());
    }

    let content = tokio::fs::read_to_string(file_path)
        .await
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let lines: Vec<String> = content
        .lines()
        .take(n)
        .map(|s| s.to_string())
        .collect();

    Ok(lines)
}

/// Get file size in bytes
pub async fn get_file_size(file_path: &PathBuf) -> u64 {
    tokio::fs::metadata(file_path)
        .await
        .map(|m| m.len())
        .unwrap_or(0)
}
```

---

#### **`src-tauri/src/mcp/builtin/workspace/tools/terminal_tools.rs`** (신규)

**목적**: 신규 MCP 도구 스키마 정의

```rust
use crate::mcp::{utils::schema_builder::*, MCPTool};
use serde_json::json;
use std::collections::HashMap;

/// Create poll_process tool
pub fn create_poll_process_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert(
        "process_id".to_string(),
        string_prop_required("Process ID returned by execute_shell (async mode)"),
    );

    // Optional tail parameter
    props.insert(
        "tail".to_string(),
        object_prop(
            vec![
                ("src".to_string(), enum_prop(vec!["stdout", "stderr"], "stdout")),
                ("n".to_string(), integer_prop_with_default(1, 100, 10, Some("Number of lines (max 100)"))),
            ],
            Vec::new(),
            Some("Get last N lines from stdout or stderr"),
        ),
    );

    MCPTool {
        name: "poll_process".to_string(),
        title: Some("Poll Process Status".to_string()),
        description: "Check the status of an asynchronously running process. \
                      Optionally retrieve the last N lines of output (max 100 lines). \
                      Only processes from the current session can be queried.".to_string(),
        input_schema: object_schema(props, vec!["process_id".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

/// Create read_process_output tool
pub fn create_read_process_output_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert(
        "process_id".to_string(),
        string_prop_required("Process ID"),
    );

    props.insert(
        "stream".to_string(),
        enum_prop_required(vec!["stdout", "stderr"], "Stream to read from"),
    );

    props.insert(
        "mode".to_string(),
        enum_prop(vec!["tail", "head"], "tail"),
    );

    props.insert(
        "lines".to_string(),
        integer_prop_with_default(1, 100, 20, Some("Number of lines to read (max 100)")),
    );

    MCPTool {
        name: "read_process_output".to_string(),
        title: Some("Read Process Output".to_string()),
        description: "Read stdout or stderr from a background process. \
                      TEXT OUTPUT ONLY. Maximum 100 lines per request. \
                      Use 'tail' mode for last N lines, 'head' for first N lines.".to_string(),
        input_schema: object_schema(props, vec!["process_id".to_string(), "stream".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

/// Create list_processes tool
pub fn create_list_processes_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert(
        "status_filter".to_string(),
        enum_prop(vec!["all", "running", "finished"], "all"),
    );

    MCPTool {
        name: "list_processes".to_string(),
        title: Some("List Processes".to_string()),
        description: "List all background processes in the current session. \
                      Filter by status: 'all' (default), 'running', or 'finished'.".to_string(),
        input_schema: object_schema(props, Vec::new()),
        output_schema: None,
        annotations: None,
    }
}

/// Export all terminal tools
pub fn terminal_tools() -> Vec<MCPTool> {
    vec![
        create_poll_process_tool(),
        create_read_process_output_tool(),
        create_list_processes_tool(),
    ]
}
```

---

### 2. 기존 파일 수정

#### **`src-tauri/src/mcp/builtin/workspace/mod.rs`** (수정)

**변경 사항**:

1. `terminal_manager` 모듈 추가
2. `WorkspaceServer`에 `process_registry` 필드 추가
3. `call_tool()` 메서드에 신규 도구 라우팅
4. `get_service_context()`에 실행 중인 프로세스 정보 추가
5. `tools()` 메서드에 `terminal_tools()` 포함

```rust
// Module imports
pub mod code_execution;
pub mod export_operations;
pub mod file_operations;
pub mod tools;
pub mod ui_resources;
pub mod utils;
pub mod terminal_manager;  // NEW

#[derive(Debug)]
pub struct WorkspaceServer {
    session_manager: Arc<SessionManager>,
    isolation_manager: crate::session_isolation::SessionIsolationManager,
    process_registry: terminal_manager::ProcessRegistry,  // NEW
}

impl WorkspaceServer {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        info!("WorkspaceServer using session-based workspace management");
        Self {
            session_manager,
            isolation_manager: crate::session_isolation::SessionIsolationManager::new(),
            process_registry: terminal_manager::create_process_registry(),  // NEW
        }
    }

    // NEW: Poll process handler
    pub async fn handle_poll_process(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        // Parse process_id
        let process_id = match args.get("process_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: process_id",
                );
            }
        };

        // Get current session
        let session_id = self.session_manager
            .get_current_session()
            .unwrap_or_else(|| "default".to_string());

        // Get process entry
        let registry = self.process_registry.read().await;
        let entry = match registry.get(process_id) {
            Some(e) => e.clone(),
            None => {
                return Self::error_response(
                    request_id,
                    -32603,
                    "Process not found or access denied",
                );
            }
        };

        // Verify session access
        if entry.session_id != session_id {
            return Self::error_response(
                request_id,
                -32603,
                "Process not found or access denied",
            );
        }
        drop(registry);

        // Build response
        let mut response = json!({
            "process_id": entry.id,
            "status": format!("{:?}", entry.status).to_lowercase(),
            "command": entry.command,
            "pid": entry.pid,
            "exit_code": entry.exit_code,
            "started_at": entry.started_at.to_rfc3339(),
            "finished_at": entry.finished_at.map(|t| t.to_rfc3339()),
            "stdout_size": entry.stdout_size,
            "stderr_size": entry.stderr_size,
        });

        // Optional tail
        if let Some(tail_obj) = args.get("tail").and_then(|v| v.as_object()) {
            let src = tail_obj.get("src")
                .and_then(|v| v.as_str())
                .unwrap_or("stdout");
            let n = tail_obj.get("n")
                .and_then(|v| v.as_u64())
                .unwrap_or(10) as usize;

            let file_path = if src == "stdout" {
                std::path::PathBuf::from(&entry.stdout_path)
            } else {
                std::path::PathBuf::from(&entry.stderr_path)
            };

            match terminal_manager::tail_lines(&file_path, n).await {
                Ok(lines) => {
                    response["tail"] = json!({
                        "src": src,
                        "lines": lines,
                    });
                }
                Err(e) => {
                    tracing::warn!("Failed to read tail: {}", e);
                }
            }
        }

        Self::success_response(
            request_id,
            &serde_json::to_string_pretty(&response).unwrap_or_default(),
        )
    }

    // NEW: Read process output handler
    pub async fn handle_read_process_output(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        // Parse parameters
        let process_id = match args.get("process_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: process_id",
                );
            }
        };

        let stream = match args.get("stream").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: stream",
                );
            }
        };

        let mode = args.get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("tail");

        let lines = args.get("lines")
            .and_then(|v| v.as_u64())
            .unwrap_or(20) as usize;

        // Get current session
        let session_id = self.session_manager
            .get_current_session()
            .unwrap_or_else(|| "default".to_string());

        // Get process entry
        let registry = self.process_registry.read().await;
        let entry = match registry.get(process_id) {
            Some(e) => e.clone(),
            None => {
                return Self::error_response(
                    request_id,
                    -32603,
                    "Process not found or access denied",
                );
            }
        };

        // Verify session access
        if entry.session_id != session_id {
            return Self::error_response(
                request_id,
                -32603,
                "Process not found or access denied",
            );
        }
        drop(registry);

        // Get file path
        let file_path = if stream == "stdout" {
            std::path::PathBuf::from(&entry.stdout_path)
        } else {
            std::path::PathBuf::from(&entry.stderr_path)
        };

        // Read lines based on mode
        let content = match mode {
            "head" => terminal_manager::head_lines(&file_path, lines).await,
            _ => terminal_manager::tail_lines(&file_path, lines).await,
        };

        match content {
            Ok(lines_vec) => {
                let response = json!({
                    "process_id": process_id,
                    "stream": stream,
                    "mode": mode,
                    "lines_requested": lines.min(100),
                    "lines_returned": lines_vec.len(),
                    "content": lines_vec,
                    "total_size": terminal_manager::get_file_size(&file_path).await,
                    "note": "Text output only. Max 100 lines per request.",
                });

                Self::success_response(
                    request_id,
                    &serde_json::to_string_pretty(&response).unwrap_or_default(),
                )
            }
            Err(e) => {
                Self::error_response(
                    request_id,
                    -32603,
                    &format!("Failed to read output: {}", e),
                )
            }
        }
    }

    // NEW: List processes handler
    pub async fn handle_list_processes(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        let status_filter = args.get("status_filter")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        // Get current session
        let session_id = self.session_manager
            .get_current_session()
            .unwrap_or_else(|| "default".to_string());

        // Filter processes by session
        let registry = self.process_registry.read().await;
        let mut processes: Vec<Value> = registry
            .values()
            .filter(|e| e.session_id == session_id)
            .filter(|e| {
                match status_filter {
                    "running" => matches!(e.status, terminal_manager::ProcessStatus::Running),
                    "finished" => matches!(e.status, terminal_manager::ProcessStatus::Finished),
                    _ => true,
                }
            })
            .map(|e| json!({
                "process_id": e.id,
                "command": e.command,
                "status": format!("{:?}", e.status).to_lowercase(),
                "pid": e.pid,
                "started_at": e.started_at.to_rfc3339(),
                "exit_code": e.exit_code,
            }))
            .collect();

        processes.sort_by(|a, b| {
            let a_time = a.get("started_at").and_then(|v| v.as_str()).unwrap_or("");
            let b_time = b.get("started_at").and_then(|v| v.as_str()).unwrap_or("");
            b_time.cmp(a_time) // descending order
        });

        let total = processes.len();
        let running = registry.values()
            .filter(|e| e.session_id == session_id)
            .filter(|e| matches!(e.status, terminal_manager::ProcessStatus::Running))
            .count();
        let finished = registry.values()
            .filter(|e| e.session_id == session_id)
            .filter(|e| matches!(e.status, terminal_manager::ProcessStatus::Finished))
            .count();

        drop(registry);

        let response = json!({
            "processes": processes,
            "total": total,
            "running": running,
            "finished": finished,
        });

        Self::success_response(
            request_id,
            &serde_json::to_string_pretty(&response).unwrap_or_default(),
        )
    }
}

#[async_trait]
impl BuiltinMCPServer for WorkspaceServer {
    // ... existing methods ...

    fn tools(&self) -> Vec<MCPTool> {
        let mut tools = Vec::new();
        tools.extend(tools::file_tools());
        tools.extend(tools::code_tools());
        tools.extend(tools::export_tools());
        tools.extend(tools::terminal_tools());  // NEW
        tools
    }

    fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        let workspace_dir_path = self.get_workspace_dir();
        let workspace_dir = workspace_dir_path.to_string_lossy().to_string();
        let tree_output = self.get_workspace_tree(&workspace_dir, 2);

        // NEW: Add running processes summary
        let registry = self.process_registry.blocking_read();
        let session_id = self.session_manager
            .get_current_session()
            .unwrap_or_else(|| "default".to_string());

        let running_count = registry.values()
            .filter(|e| e.session_id == session_id)
            .filter(|p| matches!(p.status, terminal_manager::ProcessStatus::Running))
            .count();

        drop(registry);

        let context_prompt = format!(
            "workspace: Active, {} tools, dir: {}, {} running processes",
            self.tools().len(),
            workspace_dir,
            running_count
        );

        ServiceContext {
            context_prompt,
            structured_state: Some(Value::String(workspace_dir)),
        }
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse {
        match tool_name {
            // Existing tools
            "read_file" => self.handle_read_file(args).await,
            "write_file" => self.handle_write_file(args).await,
            "list_directory" => self.handle_list_directory(args).await,
            "replace_lines_in_file" => self.handle_replace_lines_in_file(args).await,
            "import_file" => self.handle_import_file(args).await,
            "execute_shell" => self.handle_execute_shell(args).await,
            "export_file" => self.handle_export_file(args).await,
            "export_zip" => self.handle_export_zip(args).await,
            // NEW: Terminal tools
            "poll_process" => self.handle_poll_process(args).await,
            "read_process_output" => self.handle_read_process_output(args).await,
            "list_processes" => self.handle_list_processes(args).await,
            _ => {
                let request_id = Self::generate_request_id();
                Self::error_response(request_id, -32601, &format!("Tool '{tool_name}' not found"))
            }
        }
    }
}
```

---

#### **`src-tauri/src/mcp/builtin/workspace/code_execution.rs`** (수정)

**변경 사항**:

1. `handle_execute_shell()`에 `run_mode` 파라미터 처리 추가
2. `execute_shell_async()` 메서드 추가 (비동기 실행 로직)
3. 프로세스 모니터링 태스크 spawn

```rust
use super::terminal_manager;  // NEW

impl WorkspaceServer {
    pub async fn handle_execute_shell(&self, args: Value) -> MCPResponse {
        let request_id = Self::generate_request_id();

        let raw_command = match args.get("command").and_then(|v| v.as_str()) {
            Some(cmd) => cmd,
            None => {
                return Self::error_response(
                    request_id,
                    -32602,
                    "Missing required parameter: command",
                );
            }
        };

        // NEW: Check run_mode
        let run_mode = args
            .get("run_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("sync");

        // Sync mode: existing behavior
        if run_mode == "sync" {
            let timeout_secs = utils::validate_timeout(
                args.get("timeout").and_then(|v| v.as_u64())
            );
            let isolation_level = args
                .get("isolation")
                .and_then(|v| v.as_str())
                .map(|level| match level {
                    "basic" => IsolationLevel::Basic,
                    "high" => IsolationLevel::High,
                    _ => IsolationLevel::Medium,
                })
                .unwrap_or(IsolationLevel::Medium);

            return self
                .execute_shell_with_isolation(raw_command, isolation_level, timeout_secs)
                .await;
        }

        // Async/Background mode: NEW
        self.execute_shell_async(raw_command, &args).await
    }

    // NEW: Async execution handler
    async fn execute_shell_async(
        &self,
        command: &str,
        args: &Value,
    ) -> MCPResponse {
        let request_id = Self::generate_request_id();

        // Generate process ID
        let process_id = cuid2::create_id();

        // Get session info
        let session_id = self
            .session_manager
            .get_current_session()
            .unwrap_or_else(|| "default".to_string());

        let workspace_path = self.get_workspace_dir();

        // Create process tmp directory
        let process_tmp_dir = workspace_path
            .join("tmp")
            .join(format!("process_{}", process_id));

        if let Err(e) = tokio::fs::create_dir_all(&process_tmp_dir).await {
            return Self::error_response(
                request_id,
                -32603,
                &format!("Failed to create process directory: {}", e),
            );
        }

        let stdout_path = process_tmp_dir.join("stdout");
        let stderr_path = process_tmp_dir.join("stderr");

        // Normalize command
        let normalized_command = Self::normalize_shell_command(command);

        // Determine isolation level
        let isolation_level = args
            .get("isolation")
            .and_then(|v| v.as_str())
            .map(|level| match level {
                "basic" => IsolationLevel::Basic,
                "high" => IsolationLevel::High,
                _ => IsolationLevel::Medium,
            })
            .unwrap_or(IsolationLevel::Medium);

        // Create isolation config
        let isolation_config = IsolatedProcessConfig {
            session_id: session_id.clone(),
            workspace_path: workspace_path.clone(),
            command: normalized_command.clone(),
            args: vec![],
            env_vars: HashMap::new(),
            isolation_level,
        };

        // Create isolated command
        let mut cmd = match self
            .isolation_manager
            .create_isolated_command(isolation_config)
            .await
        {
            Ok(cmd) => cmd,
            Err(e) => {
                return Self::error_response(
                    request_id,
                    -32603,
                    &format!("Failed to create isolated command: {}", e),
                );
            }
        };

        // Setup stdout/stderr to pipes
        use tokio::process::Stdio;
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // Register process in registry
        let entry = terminal_manager::ProcessEntry {
            id: process_id.clone(),
            session_id: session_id.clone(),
            command: command.to_string(),
            status: terminal_manager::ProcessStatus::Starting,
            pid: None,
            exit_code: None,
            started_at: chrono::Utc::now(),
            finished_at: None,
            stdout_path: stdout_path.to_string_lossy().to_string(),
            stderr_path: stderr_path.to_string_lossy().to_string(),
            stdout_size: 0,
            stderr_size: 0,
        };

        {
            let mut registry = self.process_registry.write().await;
            registry.insert(process_id.clone(), entry.clone());
        }

        // Spawn monitoring task
        let registry = self.process_registry.clone();
        let pid_copy = process_id.clone();

        tokio::spawn(async move {
            // Spawn process
            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    // Update registry: failed
                    let mut reg = registry.write().await;
                    if let Some(entry) = reg.get_mut(&pid_copy) {
                        entry.status = terminal_manager::ProcessStatus::Failed;
                        entry.finished_at = Some(chrono::Utc::now());
                    }
                    tracing::error!("Failed to spawn process: {}", e);
                    return;
                }
            };

            let pid = child.id();

            // Update registry: running
            {
                let mut reg = registry.write().await;
                if let Some(entry) = reg.get_mut(&pid_copy) {
                    entry.status = terminal_manager::ProcessStatus::Running;
                    entry.pid = pid;
                }
            }

            // Stream stdout to file
            if let Some(mut stdout) = child.stdout.take() {
                let stdout_path_clone = stdout_path.clone();
                tokio::spawn(async move {
                    if let Ok(mut file) = tokio::fs::File::create(&stdout_path_clone).await {
                        let _ = tokio::io::copy(&mut stdout, &mut file).await;
                    }
                });
            }

            // Stream stderr to file
            if let Some(mut stderr) = child.stderr.take() {
                let stderr_path_clone = stderr_path.clone();
                tokio::spawn(async move {
                    if let Ok(mut file) = tokio::fs::File::create(&stderr_path_clone).await {
                        let _ = tokio::io::copy(&mut stderr, &mut file).await;
                    }
                });
            }

            // Wait for completion
            let exit_status = child.wait().await;

            // Update registry: finished
            let mut reg = registry.write().await;
            if let Some(entry) = reg.get_mut(&pid_copy) {
                match exit_status {
                    Ok(status) => {
                        entry.status = if status.success() {
                            terminal_manager::ProcessStatus::Finished
                        } else {
                            terminal_manager::ProcessStatus::Failed
                        };
                        entry.exit_code = status.code();
                    }
                    Err(e) => {
                        entry.status = terminal_manager::ProcessStatus::Failed;
                        tracing::error!("Process wait error: {}", e);
                    }
                }
                entry.finished_at = Some(chrono::Utc::now());

                // Update file sizes
                entry.stdout_size = terminal_manager::get_file_size(&stdout_path).await;
                entry.stderr_size = terminal_manager::get_file_size(&stderr_path).await;
            }

            info!(
                "Process {} completed with status: {:?}",
                pid_copy,
                reg.get(&pid_copy).map(|e| &e.status)
            );
        });

        // Return immediate response
        let response_msg = format!(
            "Process started in background.\n\
             Process ID: {}\n\
             Command: {}\n\
             \n\
             Use 'poll_process' to check status and view output:\n\
             poll_process(process_id: \"{}\", tail: {{src: \"stdout\", n: 20}})",
            process_id, command, process_id
        );

        Self::success_response(request_id, &response_msg)
    }

    // ... existing methods (execute_shell_with_isolation, etc.) ...
}
```

---

#### **`src-tauri/src/mcp/builtin/workspace/tools/mod.rs`** (수정)

**변경 사항**: `terminal_tools()` 함수 추가 및 export

```rust
pub mod code_tools;
pub mod export_tools;
pub mod file_tools;
pub mod terminal_tools;  // NEW

pub use code_tools::*;
pub use export_tools::*;
pub use file_tools::*;
pub use terminal_tools::*;  // NEW

// Export terminal tools
pub fn terminal_tools() -> Vec<crate::mcp::MCPTool> {
    vec![
        terminal_tools::create_poll_process_tool(),
        terminal_tools::create_read_process_output_tool(),
        terminal_tools::create_list_processes_tool(),
    ]
}
```

---

#### **`src-tauri/src/mcp/builtin/workspace/tools/code_tools.rs`** (수정)

**변경 사항**: `execute_shell` 도구 스키마에 `run_mode` 파라미터 추가

```rust
pub fn create_execute_shell_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "command".to_string(),
        string_prop_with_examples(
            Some(1),
            Some(1000),
            Some("Shell command to execute (POSIX sh compatible)"),
            vec![
                json!("ls -la"),
                json!("grep -r 'pattern' ."),
                json!(". script.sh"),
            ],
        ),
    );
    props.insert(
        "timeout".to_string(),
        integer_prop_with_default(
            Some(1),
            Some(crate::config::max_execution_timeout() as i64),
            crate::config::default_execution_timeout() as i64,
            Some("Timeout in seconds (default: 30, sync mode only)"),
        ),
    );
    // NEW: run_mode parameter
    props.insert(
        "run_mode".to_string(),
        enum_prop(
            vec!["sync", "async", "background"],
            "sync",
            Some("Execution mode: 'sync' (wait for completion), 'async'/'background' (return immediately)"),
        ),
    );
    props.insert(
        "isolation".to_string(),
        enum_prop(
            vec!["basic", "medium", "high"],
            "medium",
            Some("Isolation level: 'basic' (env only), 'medium' (process groups), 'high' (sandboxing)"),
        ),
    );

    MCPTool {
        name: "execute_shell".to_string(),
        title: Some("Execute Shell Command".to_string()),
        description: "Execute a shell command in a sandboxed environment using POSIX sh shell. \
                      \n\n\
                      MODES:\n\
                      - 'sync' (default): Wait for completion, return stdout/stderr\n\
                      - 'async'/'background': Run in background, return process_id immediately\n\
                      \n\
                      For async mode, use 'poll_process' to check status and view output.\n\
                      \n\
                      Note: bash-specific commands like 'source' are not available - use '.' instead.".to_string(),
        input_schema: object_schema(props, vec!["command".to_string()]),
        output_schema: None,
        annotations: None,
    }
}
```

---

## 🔗 재사용 가능한 연관 코드

### 기존 코드 활용

#### 1. **SessionIsolationManager** (`src-tauri/src/session_isolation.rs`)

- **목적**: 플랫폼별 프로세스 격리 및 샌드박싱
- **재사용**: `create_isolated_command()` 메서드를 그대로 사용
- **인터페이스**:

  ```rust
  pub async fn create_isolated_command(
      &self,
      config: IsolatedProcessConfig,
  ) -> Result<AsyncCommand, String>
  ```

#### 2. **SessionManager** (`src-tauri/src/session/`)

- **목적**: 세션별 워크스페이스 디렉토리 관리
- **재사용**: `get_current_session()`, `get_session_workspace_dir()`
- **인터페이스**:

  ```rust
  pub fn get_current_session(&self) -> Option<String>
  pub fn get_session_workspace_dir(&self) -> PathBuf
  ```

#### 3. **utils 모듈** (`src-tauri/src/mcp/builtin/workspace/utils.rs`)

- **목적**: 공통 유틸리티 함수
- **재사용**: `generate_request_id()`, `validate_timeout()`, response 생성 함수
- **인터페이스**:

  ```rust
  pub fn generate_request_id() -> Value
  pub fn create_success_response(request_id: Value, message: &str) -> MCPResponse
  pub fn create_error_response(request_id: Value, code: i32, message: &str) -> MCPResponse
  ```

#### 4. **schema_builder** (`src-tauri/src/mcp/utils/schema_builder.rs`)

- **목적**: MCP 도구 스키마 빌더 헬퍼
- **재사용**: `string_prop_*`, `integer_prop_*`, `enum_prop`, `object_schema`
- **인터페이스**:

  ```rust
  pub fn string_prop_required(description: &str) -> Value
  pub fn integer_prop_with_default(min, max, default, description) -> Value
  pub fn enum_prop(values: Vec<&str>, default: &str) -> Value
  pub fn object_schema(props: HashMap, required: Vec<String>) -> Value
  ```

---

## 🧪 Test Code 가이드

### 단위 테스트 (Unit Tests)

#### **테스트 파일**: `src-tauri/src/mcp/builtin/workspace/terminal_manager.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tokio::fs;

    #[tokio::test]
    async fn test_create_process_registry() {
        let registry = create_process_registry();
        assert!(registry.read().await.is_empty());
    }

    #[tokio::test]
    async fn test_tail_lines() {
        // Create temp file
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_tail.txt");

        let content = "line1\nline2\nline3\nline4\nline5\n";
        fs::write(&test_file, content).await.unwrap();

        // Test tail
        let lines = tail_lines(&test_file, 3).await.unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "line3");
        assert_eq!(lines[2], "line5");

        // Cleanup
        let _ = fs::remove_file(&test_file).await;
    }

    #[tokio::test]
    async fn test_tail_lines_max_limit() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_tail_max.txt");

        // Create 200 lines
        let mut content = String::new();
        for i in 1..=200 {
            content.push_str(&format!("line{}\n", i));
        }
        fs::write(&test_file, content).await.unwrap();

        // Request 200 lines, should get max 100
        let lines = tail_lines(&test_file, 200).await.unwrap();
        assert_eq!(lines.len(), 100);
        assert_eq!(lines[0], "line101");
        assert_eq!(lines[99], "line200");

        // Cleanup
        let _ = fs::remove_file(&test_file).await;
    }

    #[tokio::test]
    async fn test_head_lines() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_head.txt");

        let content = "line1\nline2\nline3\nline4\nline5\n";
        fs::write(&test_file, content).await.unwrap();

        let lines = head_lines(&test_file, 3).await.unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "line1");
        assert_eq!(lines[2], "line3");

        let _ = fs::remove_file(&test_file).await;
    }
}
```

### 통합 테스트 (Integration Tests)

#### **테스트 파일**: `src-tauri/tests/workspace_async_execute.rs` (신규)

```rust
use serde_json::json;

#[tokio::test]
async fn test_async_execute_shell() {
    // Setup workspace server
    let session_manager = /* create test session manager */;
    let server = WorkspaceServer::new(session_manager);

    // Test 1: Async execute
    let args = json!({
        "command": "echo 'test output'",
        "run_mode": "async"
    });

    let response = server.handle_execute_shell(args).await;
    // Verify immediate response with process_id
    assert!(response.is_success());

    // Extract process_id from response
    let process_id = /* parse from response */;

    // Test 2: Poll process
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let poll_args = json!({
        "process_id": process_id,
        "tail": {
            "src": "stdout",
            "n": 10
        }
    });

    let poll_response = server.handle_poll_process(poll_args).await;
    // Verify status and output
    assert!(poll_response.is_success());

    // Test 3: Read output
    let read_args = json!({
        "process_id": process_id,
        "stream": "stdout",
        "mode": "tail",
        "lines": 20
    });

    let read_response = server.handle_read_process_output(read_args).await;
    assert!(read_response.is_success());
    // Verify "test output" in content
}

#[tokio::test]
async fn test_session_isolation() {
    // Create two sessions
    let session_a = /* ... */;
    let session_b = /* ... */;

    // Session A: execute command
    let server_a = WorkspaceServer::new(session_a);
    let response_a = server_a.handle_execute_shell(json!({
        "command": "echo 'session A'",
        "run_mode": "async"
    })).await;

    let process_id_a = /* extract process_id */;

    // Session B: try to poll Session A's process
    let server_b = WorkspaceServer::new(session_b);
    let response_b = server_b.handle_poll_process(json!({
        "process_id": process_id_a
    })).await;

    // Should get error: access denied
    assert!(response_b.is_error());
    assert!(response_b.error_message().contains("access denied"));
}

#[tokio::test]
async fn test_large_output_streaming() {
    let server = /* setup */;

    // Execute command with large output (e.g., 10MB)
    let response = server.handle_execute_shell(json!({
        "command": "seq 1 1000000",
        "run_mode": "async"
    })).await;

    let process_id = /* extract */;

    // Wait for completion
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Verify file exists and has correct size
    let registry = server.process_registry.read().await;
    let entry = registry.get(&process_id).unwrap();
    assert!(entry.stdout_size > 1_000_000);

    // Read output should succeed and return max 100 lines
    let read_response = server.handle_read_process_output(json!({
        "process_id": process_id,
        "stream": "stdout",
        "lines": 100
    })).await;

    assert!(read_response.is_success());
    // Verify line count is exactly 100
}
```

### 테스트 체크리스트

- [ ] **Unit Tests**
  - [ ] `tail_lines()` 함수 (정상 케이스)
  - [ ] `tail_lines()` 최대 100줄 제한
  - [ ] `head_lines()` 함수
  - [ ] `get_file_size()` 함수
  - [ ] ProcessRegistry CRUD 연산

- [ ] **Integration Tests**
  - [ ] Async execute + poll (정상 완료)
  - [ ] Async execute + poll (실패 케이스)
  - [ ] read_process_output (stdout/stderr)
  - [ ] list_processes (필터링)
  - [ ] 세션 격리 (다른 세션 접근 불가)
  - [ ] 대용량 출력 스트리밍 (10MB+)
  - [ ] 동시 다중 프로세스 실행

- [ ] **Manual Tests**
  - [ ] 장시간 실행 명령어 (npm run dev)
  - [ ] 실시간 출력 확인 (tail -f)
  - [ ] 에러 발생 시 stderr 조회
  - [ ] 프로세스 완료 후 상태 확인

---

## 📝 추가 고려사항

### 1. 프로세스 정리 (Cleanup)

- **문제**: 오래된 프로세스 메타데이터 및 출력 파일 누적
- **해결책**:
  - 완료된 프로세스는 24시간 후 자동 삭제
  - `cleanup_old_processes()` 함수 추가 (백그라운드 태스크)
  - 세션 종료 시 해당 세션의 모든 프로세스 정리

### 2. 프로세스 강제 종료 (Kill)

- **Phase 2 기능** (현재 MVP 제외)
- `kill_process(process_id)` 도구 추가
- 모니터링 태스크와 통신하는 채널 필요 (`tokio::sync::mpsc`)

### 3. 보안 강화

- **세션 검증**: 모든 도구 호출 시 session_id 검증
- **경로 제한**: 출력 파일은 반드시 세션 워크스페이스 내부
- **리소스 제한**: 동시 실행 프로세스 수 제한 (예: 최대 20개)

### 4. 성능 최적화

- **파일 읽기**: 대용량 파일 tail 시 역방향 버퍼 읽기 최적화
- **레지스트리**: 완료된 프로세스는 별도 저장소로 이동 (메모리 절약)
- **출력 압축**: 오래된 출력 파일 자동 압축 (gzip)

### 5. 에러 처리

- **Spawn 실패**: 명령어 찾을 수 없음, 권한 부족 등
- **파일 쓰기 실패**: 디스크 공간 부족, 권한 문제
- **비정상 종료**: 시그널 kill, 시스템 재시작 등

---

## 📅 구현 일정 (예상)

### Phase 1: Core Infrastructure (2-3 days)

- [ ] Day 1: `terminal_manager.rs` 구현 및 단위 테스트
- [ ] Day 2: `mod.rs` 수정 (레지스트리 통합)
- [ ] Day 3: `code_execution.rs` async 로직 구현

### Phase 2: Tool Implementation (2-3 days)

- [ ] Day 4: `terminal_tools.rs` 스키마 정의
- [ ] Day 5: Poll/Read/List 핸들러 구현
- [ ] Day 6: 통합 테스트 작성

### Phase 3: Testing & Refinement (2-3 days)

- [ ] Day 7-8: 통합 테스트 실행 및 버그 수정
- [ ] Day 9: 매뉴얼 테스트 및 문서화

**Total**: 6-9 일 (약 1-1.5주)

---

## ✅ 체크리스트 (작업 시작 전)

- [ ] 코드 리뷰: 기존 `execute_shell` 동작 완전히 이해
- [ ] 테스트 환경: 개발/테스트용 세션 및 워크스페이스 준비
- [ ] 의존성 확인: `cuid2`, `chrono`, `tokio` 버전 호환성
- [ ] 보안 검토: 세션 격리 로직 검증
- [ ] 문서 업데이트: API 문서, 사용 가이드 작성 계획

---

## 🔗 참고 자료

### 프로젝트 문서

- [Chat Feature Architecture](../architecture/chat-feature-architecture.md)
- [Built-in Tools Documentation](../builtin-tools.md)
- [Coding Standards](../contributing/coding-standards.md)

### 외부 자료

- [Tokio Process Documentation](https://docs.rs/tokio/latest/tokio/process/)
- [MCP Protocol Specification](https://modelcontextprotocol.io/docs)

---

**Document Version**: 1.0  
**Last Updated**: 2025-10-10  
**Next Review**: After Phase 1 completion
