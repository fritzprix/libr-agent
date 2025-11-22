# Persistent Shell Integration Refactoring Plan

**작성일**: 2025-11-22  
**작성자**: AI Agent (Claude Sonnet 4.5)  
**관련 POC**: [STDIO REPL POC](./stdio_repl_poc_20251122.md)  
**목표**: STDIO 기반 persistent shell을 LibrAgent workspace 도구에 통합하여 상태 보존 문제 해결

---

## 🎯 작업의 목적

### 배경

현재 LibrAgent의 `execute_shell` 도구는 **매 명령마다 새로운 프로세스를 생성**하는 one-shot 실행 방식을 사용합니다. 이로 인해 다음과 같은 문제가 발생합니다:

1. **상태 손실**: `cd`, `export`, Python venv 활성화 등의 상태가 명령 간 유지되지 않음
2. **성능 오버헤드**: 매 명령마다 50-100ms의 프로세스 spawn 비용 발생
3. **사용자 경험 저하**: AI 에이전트가 상태 보존을 위해 복잡한 workaround 사용 (e.g., `cd /tmp && ls`)

### 목표

STDIO 기반 persistent shell 세션을 도입하여:

- ✅ 작업 디렉토리 상태 보존 (`cd` 명령 유지)
- ✅ 환경변수 지속성 (`export VAR=value` 유지)
- ✅ Python/Node.js venv 활성화 상태 유지
- ✅ 성능 개선 (프로세스 재사용으로 오버헤드 제거)
- ✅ 크로스 플랫폼 통일 아키텍처 (Unix bash + Windows PowerShell)

### 비목표 (Out of Scope)

- PTY 기반 구현 (복잡도 높음, Windows ConPTY 문제)
- Interactive TUI 지원 (vim, less 등) - Two-Tool Pattern으로 처리
- 기존 one-shot 실행 방식 완전 제거 (backward compatibility 유지)

---

## 📊 현재의 상태 / 문제점

### 현재 아키텍처 (Birdeye View)

```
┌─────────────────────────────────────────────────────────────┐
│                     Frontend (React)                        │
│  - Chat interface sends tool calls via MCP                  │
└─────────────────┬───────────────────────────────────────────┘
                  │ MCP Protocol
┌─────────────────▼───────────────────────────────────────────┐
│              WorkspaceServer (Rust)                         │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ handle_execute_shell(command, timeout, run_mode)      │  │
│  │   ↓                                                   │  │
│  │ execute_shell_with_isolation()                        │  │
│  │   ↓                                                   │  │
│  │ SessionIsolationManager::create_isolated_command()    │  │
│  │   ↓                                                   │  │
│  │ spawn_and_stream_to_files() [ONE-SHOT PROCESS]       │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  Components:                                                │
│  - process_registry: ProcessRegistry (async 프로세스 추적)   │
│  - pending_executions: PendingExecutions (interactive 입력)  │
│  - isolation_manager: SessionIsolationManager (보안 격리)    │
└─────────────────────────────────────────────────────────────┘
```

### 핵심 코드 구조

#### 1. `code_execution.rs` - 명령 실행 로직

**현재 동작**:

```rust
async fn execute_shell_with_isolation(
    &self,
    command: &str,
    isolation_level: IsolationLevel,
    timeout_secs: u64,
) -> Result<MCPResult, String> {
    // 1. SessionIsolationManager로 격리된 Command 생성
    let cmd = self.isolation_manager
        .create_isolated_command(IsolatedProcessConfig {
            session_id,
            workspace_path,
            command: normalized_command,
            args: vec![],
            env_vars: HashMap::new(),
            isolation_level,
        })
        .await?;

    // 2. 매번 새 프로세스 spawn (ONE-SHOT)
    let (pid, exit_code, stdout, stderr) =
        Self::spawn_and_stream_to_files(
            cmd,
            stdout_path,
            stderr_path,
            process_label,
            cancel_token,
        ).await?;

    // 3. 결과 반환 후 프로세스 종료
    Ok(MCPResult::TextContent { ... })
}
```

**문제점**:

- 매 호출마다 `spawn()` 실행 → 상태 유지 불가
- `cd /tmp`를 실행해도 다음 명령은 원래 디렉토리에서 시작
- 환경변수 설정이 다음 명령에 전파되지 않음

#### 2. `session_isolation.rs` - 보안 격리 및 환경 설정

**역할**:

- 플랫폼별 shell 명령 생성 (Unix: bash, Windows: PowerShell)
- 환경변수 격리 (workspace 디렉토리 제한)
- 프로세스 그룹 격리 (Medium isolation)

**현재 구조**:

```rust
async fn create_basic_isolated_command(
    &self,
    config: IsolatedProcessConfig,
) -> Result<AsyncCommand, String> {
    #[cfg(windows)]
    let mut cmd = AsyncCommand::new("powershell.exe");

    #[cfg(windows)]
    {
        cmd.arg("-NoProfile");
        cmd.arg("-NoLogo");
        cmd.arg("-Command");  // ONE-SHOT 실행 모드
        cmd.arg(&config.command);
    }

    // 환경변수 설정
    cmd.env("USERPROFILE", &config.workspace_path);
    cmd.current_dir(&config.workspace_path);

    Ok(cmd)
}
```

**문제점**:

- `-Command` 모드는 명령 실행 후 즉시 종료
- 상태 보존 불가능한 구조
- persistent shell로 변경하려면 `-NonInteractive` 모드로 stdin 사용 필요

#### 3. `terminal_manager.rs` - 프로세스 레지스트리

**역할**:

- 백그라운드 프로세스(async 모드) 추적
- stdout/stderr 실시간 스트리밍
- 프로세스 상태 관리 (Starting, Running, Finished, Failed, Killed)

**구조**:

```rust
pub struct ProcessRegistry {
    entries: HashMap<String, ProcessEntry>,
    cancellation_tokens: HashMap<String, CancellationToken>,
    streaming_handles: HashMap<String, Arc<StreamingHandle>>,
}

pub struct ProcessEntry {
    pub id: String,
    pub session_id: String,
    pub command: String,
    pub status: ProcessStatus,
    pub pid: Option<u32>,
    pub exit_code: Option<i32>,
    // ...
}
```

**활용 가능성**:

- Persistent shell도 `ProcessEntry`로 관리 가능
- `session_id` 기반으로 shell 인스턴스 매핑
- 기존 레지스트리 구조 재사용 가능

#### 4. `pending_executions.rs` - Interactive 입력 처리

**역할**:

- Two-Tool Pattern 구현 (sudo, password 입력)
- 1차 호출: UIResource 반환 (execution_id)
- 2차 호출: user_input 주입하여 실행

**구조**:

```rust
pub struct PendingShellExecution {
    pub execution_id: String,
    pub session_id: String,
    pub executable_command: String,
    pub display_command: String,
    pub run_mode: String,
    pub timeout: u64,
    pub created_at: DateTime<Utc>,
}

pub struct PendingExecutions(Mutex<HashMap<String, PendingShellExecution>>);
```

**통합 고려사항**:

- Persistent shell에서도 Two-Tool Pattern 유지 필요
- stdin 입력은 persistent shell의 execute() 메서드에서 처리
- 기존 구조 변경 최소화

---

## 🔄 변경 이후의 상태 / 해결 판정 기준

### 목표 아키텍처

```
┌─────────────────────────────────────────────────────────────┐
│                     Frontend (React)                        │
└─────────────────┬───────────────────────────────────────────┘
                  │ MCP Protocol
┌─────────────────▼───────────────────────────────────────────┐
│              WorkspaceServer (Rust)                         │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ handle_execute_shell(command)                         │  │
│  │   ↓                                                   │  │
│  │ ┌─ persistent_shell_enabled? ──────────────────────┐ │  │
│  │ │   Yes: PersistentShellManager::execute()         │ │  │
│  │ │        ↓                                          │ │  │
│  │ │   get_or_create_shell(session_id)                │ │  │
│  │ │        ↓                                          │ │  │
│  │ │   shell.execute(command) [REUSES PROCESS]        │ │  │
│  │ │                                                   │ │  │
│  │ │   No: execute_shell_with_isolation() [ONE-SHOT]  │ │  │
│  │ └───────────────────────────────────────────────────┘ │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  NEW Component:                                             │
│  - shell_manager: PersistentShellManager                    │
│    - shells: HashMap<session_id, Arc<Mutex<PersistentShell>>>│
│    - max_shells_per_session: 3 (제한)                       │
└─────────────────────────────────────────────────────────────┘
```

### 새로운 데이터 플로우

1. **세션별 shell 인스턴스 관리**:

   ```rust
   // 첫 번째 명령
   execute_shell("cd /tmp")
     → shell_manager.get_or_create_shell(session_id)
     → PersistentShell::new() [bash/powershell spawn]
     → shell.execute("cd /tmp")

   // 두 번째 명령 (동일 세션)
   execute_shell("pwd")
     → shell_manager.get_or_create_shell(session_id) [기존 shell 재사용]
     → shell.execute("pwd")
     → Result: "/tmp" ✅ 상태 보존!
   ```

2. **Sentinel 기반 동기화**:

   ```rust
   async fn execute(&mut self, command: &str) -> (String, String, i32) {
       let sentinel = generate_sentinel();  // "STDIO_SENTINEL_42"

       self.stdin.write_all(command.as_bytes()).await?;
       self.stdin.write_all(b"\n").await?;

       // Platform-specific sentinel marker
       #[cfg(windows)]
       self.stdin.write_all(format!("Write-Output '{}'\n", sentinel).as_bytes()).await?;

       // Read until sentinel found (NO timing dependency)
       loop {
           tokio::select! {
               line = self.stdout.read_line() => {
                   if line.trim() == sentinel {
                       break;  // Command completed
                   }
                   stdout_lines.push(line);
               }
               line = self.stderr.read_line() => {
                   stderr_lines.push(line);
               }
           }
       }

       (stdout, stderr, exit_code)
   }
   ```

### 성공 판정 기준

| #   | 검증 항목              | 테스트 방법                              | 기대 결과                        |
| --- | ---------------------- | ---------------------------------------- | -------------------------------- |
| 1   | 작업 디렉토리 보존     | `cd /tmp; pwd` (2개 명령)                | 두 번째 명령 결과 `/tmp`         |
| 2   | 환경변수 지속성        | `export VAR=value; echo $VAR`            | 두 번째 명령 결과 `value`        |
| 3   | Python venv 활성화     | `source venv/bin/activate; which python` | venv 내 Python 경로              |
| 4   | 성능 개선              | 100회 연속 명령 실행 시간 측정           | one-shot 대비 30% 이상 단축      |
| 5   | 에러 처리              | 존재하지 않는 명령 실행                  | stderr 출력 + non-zero exit code |
| 6   | UTF-8 인코딩           | 한글 파일명/에러 메시지 처리             | lossy 변환으로 crash 없이 처리   |
| 7   | Two-Tool Pattern       | `sudo ls /root` (interactive)            | UIResource 반환 → 입력 후 실행   |
| 8   | 세션 격리              | 다른 세션에서 `cd` 실행                  | 각 세션 독립적인 상태 유지       |
| 9   | Shell 정리             | 세션 종료 시 shell 프로세스 kill         | 좀비 프로세스 없음               |
| 10  | Backward Compatibility | `use_persistent_shell=false` 옵션        | 기존 one-shot 방식 동작          |

---

## 🔧 수정이 필요한 코드 및 코드 스니핏

### 1. `persistent_shell.rs` (NEW FILE)

**위치**: `src-tauri/src/mcp/builtin/workspace/persistent_shell.rs`

**역할**: STDIO 기반 persistent shell 세션 구현

**핵심 코드** (이미 작성 완료):

```rust
pub struct PersistentShell {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    stderr: BufReader<ChildStderr>,
    session_id: String,
}

impl PersistentShell {
    pub async fn new(session_id: String) -> Result<Self> { /* ... */ }

    pub async fn execute(&mut self, command: &str) -> Result<(String, String, i32)> {
        // Sentinel 기반 동기화 로직
    }

    /// Two-Tool Pattern: stdin으로 user input 전달 후 명령 실행
    pub async fn execute_with_input(
        &mut self,
        command: &str,
        user_input: &str,
    ) -> Result<(String, String, i32)> {
        // stdin injection 방식 (Q3 참조)
    }

    pub async fn terminate(&mut self) -> Result<()> { /* ... */ }
}
```

**추가 필요 사항**:

- ✅ 기본 구현 완료 (POC 검증됨)
- ✅ Two-Tool Pattern 통합: `execute_with_input()` 메서드 추가 (stdin 직접 주입 방식, Q3 참조)
- ⏳ Timeout 처리 추가: `tokio::time::timeout()` wrapper
- ⏳ Session isolation 통합: `new_with_isolation()` 생성자

### 2. `persistent_shell_manager.rs` (NEW FILE)

**위치**: `src-tauri/src/mcp/builtin/workspace/persistent_shell_manager.rs`

**역할**: 세션별 shell 인스턴스 관리

**새 코드**:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use super::persistent_shell::PersistentShell;

pub struct PersistentShellManager {
    /// session_id -> PersistentShell mapping
    shells: Arc<Mutex<HashMap<String, Arc<Mutex<PersistentShell>>>>>,

    /// Maximum shells per session (default: 3)
    max_shells_per_session: usize,
}

impl PersistentShellManager {
    pub fn new() -> Self {
        Self {
            shells: Arc::new(Mutex::new(HashMap::new())),
            max_shells_per_session: 3,
        }
    }

    /// Get or create persistent shell for session
    pub async fn get_or_create_shell(
        &self,
        session_id: String,
    ) -> Result<Arc<Mutex<PersistentShell>>, String> {
        let mut shells = self.shells.lock().await;

        if let Some(shell) = shells.get(&session_id) {
            // Check if shell is still alive
            if shell.lock().await.pid().is_some() {
                return Ok(shell.clone());
            } else {
                // Dead shell, remove it
                shells.remove(&session_id);
            }
        }

        // Create new shell
        let shell = PersistentShell::new(session_id.clone())
            .await
            .map_err(|e| format!("Failed to create shell: {}", e))?;

        let shell_arc = Arc::new(Mutex::new(shell));
        shells.insert(session_id.clone(), shell_arc.clone());

        Ok(shell_arc)
    }

    /// Execute command in persistent shell
    pub async fn execute(
        &self,
        session_id: String,
        command: &str,
    ) -> Result<(String, String, i32), String> {
        let shell = self.get_or_create_shell(session_id).await?;
        let mut shell_guard = shell.lock().await;

        shell_guard
            .execute(command)
            .await
            .map_err(|e| format!("Shell execution failed: {}", e))
    }

    /// Execute command with user input (Two-Tool Pattern)
    pub async fn execute_with_input(
        &self,
        session_id: String,
        command: &str,
        user_input: &str,
    ) -> Result<(String, String, i32), String> {
        let shell = self.get_or_create_shell(session_id).await?;
        let mut shell_guard = shell.lock().await;

        shell_guard
            .execute_with_input(command, user_input)
            .await
            .map_err(|e| format!("Shell execution with input failed: {}", e))
    }    /// Terminate shell for session
    pub async fn terminate_shell(&self, session_id: &str) -> Result<(), String> {
        let mut shells = self.shells.lock().await;

        if let Some(shell) = shells.remove(session_id) {
            shell.lock().await.terminate().await
                .map_err(|e| format!("Failed to terminate shell: {}", e))?;
        }

        Ok(())
    }

    /// Cleanup all shells
    pub async fn cleanup_all(&self) -> Result<(), String> {
        let mut shells = self.shells.lock().await;

        for (_, shell) in shells.drain() {
            let _ = shell.lock().await.terminate().await;
        }

        Ok(())
    }
}
```

### 3. `mod.rs` 수정

**파일**: `src-tauri/src/mcp/builtin/workspace/mod.rs`

**변경 내용**:

```rust
// 모듈 추가
pub mod persistent_shell;
pub mod persistent_shell_manager;

// WorkspaceServer 구조체 수정
#[derive(Debug)]
pub struct WorkspaceServer {
    session_manager: Arc<SessionManager>,
    isolation_manager: crate::session_isolation::SessionIsolationManager,
    process_registry: terminal_manager::ProcessRegistry,
    pending_executions: Arc<PendingExecutions>,

    // NEW: Persistent shell manager
    shell_manager: Arc<persistent_shell_manager::PersistentShellManager>,
}

impl WorkspaceServer {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        // ...
        Self {
            // ...
            shell_manager: Arc::new(persistent_shell_manager::PersistentShellManager::new()),
        }
    }
}
```

### 4. `code_execution.rs` 수정

**파일**: `src-tauri/src/mcp/builtin/workspace/code_execution.rs`

**변경 내용**:

```rust
impl WorkspaceServer {
    pub async fn handle_execute_shell(&self, args: Value) -> Result<MCPResult, String> {
        let raw_command = /* ... */;
        let require_input = /* ... */;

        if require_input || auto_detect {
            return self.handle_interactive_shell(&args).await;
        }

        let run_mode = args.get("run_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("sync");

        // NEW: Check if persistent shell is enabled (feature flag)
        let use_persistent_shell = args.get("use_persistent_shell")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);  // Default: enabled

        if use_persistent_shell && run_mode == "sync" {
            // NEW PATH: Persistent shell execution
            return self.execute_shell_persistent(raw_command, &args).await;
        }

        // EXISTING PATH: One-shot isolation execution
        if run_mode == "async" {
            return self.execute_shell_async(raw_command, &args).await;
        }

        let timeout_secs = /* ... */;
        self.execute_shell_with_isolation(raw_command, isolation_level, timeout_secs).await
    }

    /// NEW: Execute command using persistent shell
    async fn execute_shell_persistent(
        &self,
        command: &str,
        args: &Value,
    ) -> Result<MCPResult, String> {
        let session_id = self.session_manager
            .get_current_session()
            .ok_or("No active session")?
            .id;

        // Normalize command
        let normalized_command = Self::normalize_shell_command(command);

        // Execute with timeout
        let timeout_secs = utils::validate_timeout(args.get("timeout").and_then(|v| v.as_u64()));
        let timeout_duration = Duration::from_secs(timeout_secs);

        let execution_result = tokio::time::timeout(
            timeout_duration,
            self.shell_manager.execute(session_id, &normalized_command),
        )
        .await;

        match execution_result {
            Ok(Ok((stdout, stderr, exit_code))) => {
                // Success case
                let mut content_parts = Vec::new();

                if !stdout.is_empty() {
                    content_parts.push(json!({
                        "type": "text",
                        "text": format!("stdout:\n{}", stdout.trim())
                    }));
                }

                if !stderr.is_empty() {
                    content_parts.push(json!({
                        "type": "text",
                        "text": format!("stderr:\n{}", stderr.trim())
                    }));
                }

                content_parts.push(json!({
                    "type": "text",
                    "text": format!("exit_code: {}", exit_code)
                }));

                Ok(MCPResult::ToolResponse {
                    content: content_parts,
                    is_error: Some(exit_code != 0),
                })
            }
            Ok(Err(e)) => {
                // Execution error
                Err(format!("Persistent shell execution failed: {}", e))
            }
            Err(_) => {
                // Timeout
                Err(format!("Command execution timeout after {} seconds", timeout_secs))
            }
        }
    }
}
```

### 5. `code_tools.rs` 수정 (Optional)

**파일**: `src-tauri/src/mcp/builtin/workspace/tools/code_tools.rs`

**변경 내용** (tool schema에 옵션 추가):

```rust
pub fn create_execute_shell_tool() -> MCPTool {
    let mut props = HashMap::new();

    // Existing properties...

    // NEW: Optional feature flag for persistent shell
    props.insert(
        "use_persistent_shell".to_string(),
        {
            let mut schema = boolean_prop(Some(
                "Use persistent shell for state preservation (cd, export, venv). \
                 Default: true. Set to false for isolated one-shot execution."
            ));
            schema.default = Some(json!(true));
            schema
        },
    );

    // ...
}
```

---

## 📦 재사용 가능한 연관 코드

### 1. POC 검증 코드

**위치**: `stdio-repl-poc/src/main.rs`

**재사용 가능 부분**:

- ✅ `read_line_lossy()`: UTF-8 lossy 변환 로직
- ✅ `generate_sentinel()`: Atomic counter 기반 sentinel 생성
- ✅ `PersistentShell::new()`: Shell 초기화 로직
- ✅ `PersistentShell::execute()`: Sentinel 기반 명령 실행

**통합 방법**: 그대로 `persistent_shell.rs`로 복사 (이미 완료)

### 2. 기존 코드베이스 활용

| 코드                          | 위치                   | 재사용 방법                                            |
| ----------------------------- | ---------------------- | ------------------------------------------------------ |
| `ProcessRegistry`             | `terminal_manager.rs`  | Persistent shell도 `ProcessEntry`로 등록하여 통합 관리 |
| `SessionIsolationManager`     | `session_isolation.rs` | Persistent shell 생성 시 환경변수 격리 로직 재사용     |
| `PendingExecutions`           | `mod.rs`               | Interactive 입력 처리 로직 그대로 사용                 |
| `normalize_shell_command()`   | `code_execution.rs`    | 명령어 정규화 로직 재사용                              |
| `spawn_and_stream_to_files()` | `code_execution.rs`    | Fallback one-shot 실행에 계속 사용                     |

### 3. 환경변수 격리 통합

**기존 코드** (`session_isolation.rs`):

```rust
async fn create_basic_isolated_command(
    &self,
    config: IsolatedProcessConfig,
) -> Result<AsyncCommand, String> {
    #[cfg(windows)]
    {
        cmd.env("USERPROFILE", &config.workspace_path);
        cmd.env("HOME", &config.workspace_path);
        cmd.env("TEMP", config.workspace_path.join("tmp"));
        // ...
    }
}
```

**재사용 방법** (`persistent_shell.rs`에 통합):

```rust
impl PersistentShell {
    pub async fn new_with_isolation(
        session_id: String,
        workspace_path: PathBuf,
    ) -> Result<Self> {
        #[cfg(windows)]
        let mut cmd = Command::new("powershell.exe");

        #[cfg(windows)]
        {
            cmd.arg("-NoProfile");
            cmd.arg("-NoLogo");
            cmd.arg("-NonInteractive");

            // Reuse isolation logic
            cmd.env("USERPROFILE", &workspace_path);
            cmd.env("HOME", &workspace_path);
            cmd.env("TEMP", workspace_path.join("tmp"));
            cmd.current_dir(&workspace_path);
        }

        // ...
    }
}
```

---

## 🧪 Test Code 추가 및 수정 가이드

### 1. Unit Tests (`persistent_shell.rs`)

**이미 작성된 테스트** (POC):

```rust
#[tokio::test]
async fn test_basic_command() -> Result<()> { /* ... */ }

#[tokio::test]
async fn test_working_directory_persistence() -> Result<()> { /* ... */ }

#[tokio::test]
async fn test_environment_variable_persistence() -> Result<()> { /* ... */ }
```

**추가 필요 테스트**:

```rust
#[tokio::test]
async fn test_timeout_handling() -> Result<()> {
    let mut shell = PersistentShell::new("test-timeout".to_string()).await?;

    let result = tokio::time::timeout(
        Duration::from_secs(1),
        shell.execute("sleep 10"),
    ).await;

    assert!(result.is_err()); // Timeout should occur
    shell.terminate().await?;
    Ok(())
}

#[tokio::test]
async fn test_error_output_capture() -> Result<()> {
    let mut shell = PersistentShell::new("test-error".to_string()).await?;

    #[cfg(unix)]
    let (_, stderr, exit_code) = shell.execute("ls /nonexistent").await?;

    #[cfg(windows)]
    let (_, stderr, exit_code) = shell.execute("Get-Item C:\\nonexistent").await?;

    assert_ne!(exit_code, 0);
    assert!(!stderr.is_empty());

    shell.terminate().await?;
    Ok(())
}

#[tokio::test]
async fn test_utf8_lossy_conversion() -> Result<()> {
    // Test Korean/CP949 error messages
    let mut shell = PersistentShell::new("test-utf8".to_string()).await?;

    #[cfg(windows)]
    {
        let (_, stderr, _) = shell.execute("Get-ChildItem C:\\존재하지않음").await?;
        // Should not panic, lossy conversion handles invalid UTF-8
        assert!(!stderr.is_empty());
    }

    shell.terminate().await?;
    Ok(())
}
```

### 2. Integration Tests (`code_execution.rs`)

**새 테스트 파일**: `src-tauri/src/mcp/builtin/workspace/tests/persistent_shell_integration_tests.rs`

```rust
use super::*;

#[tokio::test]
async fn test_persistent_shell_state_preservation() {
    let workspace = WorkspaceServer::new(/* ... */);

    // First command: cd
    let args1 = json!({
        "command": "cd /tmp",
        "use_persistent_shell": true
    });
    let result1 = workspace.handle_execute_shell(args1).await;
    assert!(result1.is_ok());

    // Second command: pwd (should be /tmp)
    let args2 = json!({
        "command": "pwd",
        "use_persistent_shell": true
    });
    let result2 = workspace.handle_execute_shell(args2).await;
    // Verify result contains "/tmp"
}

#[tokio::test]
async fn test_persistent_vs_oneshot_isolation() {
    let workspace = WorkspaceServer::new(/* ... */);

    // Persistent mode: cd should persist
    let args1 = json!({
        "command": "cd /tmp; pwd",
        "use_persistent_shell": true
    });
    let result1 = workspace.handle_execute_shell(args1).await;

    // One-shot mode: cd should NOT persist
    let args2 = json!({
        "command": "cd /tmp; pwd",
        "use_persistent_shell": false
    });
    let result2 = workspace.handle_execute_shell(args2).await;

    // Results should be same (both return /tmp) but for different reasons
}
```

### 3. Performance Benchmark Tests

**새 파일**: `src-tauri/benches/persistent_shell_bench.rs`

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_oneshot_vs_persistent(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("oneshot 100 commands", |b| {
        b.to_async(&rt).iter(|| async {
            for _ in 0..100 {
                // Execute one-shot command
                execute_shell_with_isolation("echo hello").await;
            }
        });
    });

    c.bench_function("persistent 100 commands", |b| {
        b.to_async(&rt).iter(|| async {
            let shell = PersistentShell::new("bench".to_string()).await.unwrap();
            for _ in 0..100 {
                shell.execute("echo hello").await;
            }
        });
    });
}

criterion_group!(benches, benchmark_oneshot_vs_persistent);
criterion_main!(benches);
```

**실행 방법**:

```bash
cargo bench --bench persistent_shell_bench
```

**기대 결과**: Persistent shell이 30% 이상 빠름

---

## 📝 추가 분석 과제

### 1. Two-Tool Pattern 통합 검증

**현재 상태**:

- `PendingExecutions`에 `executable_command` 저장
- `execute_pending_shell`에서 stdin 입력 후 one-shot 실행

**결정 사항** (Q3 답변 기반):

- ✅ **Option B 채택**: `execute_with_input()` 메서드 추가
- ✅ stdin에 `user_input + \n` 전송 후 명령 실행
- ✅ 기존 one-shot 구현과 동일한 패턴 (stdin pipe 사용)

**구현 방법**:

```rust
impl PersistentShell {
    pub async fn execute_with_input(
        &mut self,
        command: &str,
        user_input: &str,
    ) -> Result<(String, String, i32)> {
        // 1. stdin에 user_input 먼저 전달
        self.stdin.write_all(user_input.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;

        // 2. 명령 실행 (기존 execute 로직 재사용)
        self.execute(command).await
    }
}
```

**검증 계획**:

1. ✅ sudo 명령 테스트 시나리오 작성
2. ⚠️ Multiple prompts는 미지원 (현재 구현과 동일한 제약)
3. ✅ 보안성: 프로세스 명령줄에 노출되지 않음 (stdin pipe)

### 2. Session Cleanup 전략

**현재 상태**:

- `WorkspaceServer::cleanup_old_processes()`: 24시간 후 프로세스 정리
- `SessionManager`: 세션 종료 시 cleanup 이벤트

**분석 필요 사항**:

- ❓ Persistent shell을 언제 종료할 것인가?
  - Option A: 세션 종료 시 즉시 terminate
  - Option B: 24시간 idle 후 자동 정리
  - Option C: 명시적 `close_shell` tool 추가
- ❓ 좀비 프로세스 방지 전략
- ❓ Shell crash 시 재생성 로직

**검증 계획**:

1. 세션 종료 이벤트 핸들러 확인
2. Drop trait 구현으로 자동 정리 검증
3. Crash recovery 테스트 시나리오 작성

### 3. Cross-Platform 동작 검증

**현재 상태**:

- POC는 Windows에서만 테스트됨
- Unix 로직은 `#[cfg(unix)]` 분기로 작성

**분석 필요 사항**:

- ❓ Linux bash 동작 검증 (Ubuntu, Fedora)
- ❓ macOS zsh vs bash 차이점
- ❓ Windows PowerShell Core (pwsh) 지원 여부

**검증 계획**:

1. GitHub Actions CI에 크로스 플랫폼 테스트 추가
2. Docker 컨테이너로 Linux 환경 테스트
3. macOS 테스트 환경 구축 (필요시)

### 4. 보안 영향 평가

**현재 상태**:

- `SessionIsolationManager`로 환경변수 격리
- Medium isolation: process group + resource limits

**분석 필요 사항**:

- ❓ Persistent shell이 보안 격리를 우회할 수 있는가?
  - Shell 내에서 `export PATH=...` 실행 시 격리 무효화?
- ❓ Shell escape 공격 가능성
- ❓ Resource exhaustion (무한 루프 명령)

**검증 계획**:

1. 보안 테스트 시나리오 작성 (path injection, command injection)
2. Resource limit 테스트 (CPU, memory)
3. 필요시 추가 격리 메커니즘 도입

---

## 🚀 단계별 구현 계획

### Phase 1: Core Implementation (1-2일)

**작업**:

1. ✅ `persistent_shell.rs` 작성 (완료)
2. `persistent_shell_manager.rs` 작성
3. `mod.rs`에 모듈 통합
4. Unit test 작성 및 실행

**검증**:

- `cargo test --package libragent --lib mcp::builtin::workspace::persistent_shell`
- 모든 테스트 통과

### Phase 2: Integration (2-3일)

**작업**:

1. `code_execution.rs`에 `execute_shell_persistent()` 추가
2. `handle_execute_shell()`에 feature flag 통합
3. Timeout 처리 추가
4. Integration test 작성

**검증**:

- 기존 테스트 모두 통과 (regression 없음)
- Persistent shell 테스트 통과
- 수동 테스트: 실제 workspace에서 cd, export 동작 확인

### Phase 3: Two-Tool Pattern 통합 (2-3일)

**작업**:

1. ✅ `PersistentShell::execute_with_input()` 메서드 구현
2. ✅ `PersistentShellManager::execute_with_input()` wrapper 추가
3. `execute_pending_shell()`에 persistent shell 경로 추가:

   ```rust
   if use_persistent_shell {
       shell_manager.execute_with_input(session_id, command, user_input).await?
   } else {
       // 기존 one-shot 방식
   }
   ```

4. Interactive test 작성 (sudo, password 입력)
5. 기존 UIResource 동작 확인

**검증**:

- ✅ stdin 전달 방식 동일 (one-shot과 일관성)
- ✅ sudo 명령 테스트 시나리오
- ✅ password 입력 보안성 확인 (프로세스 명령줄 노출 없음)
- ⚠️ Multiple prompts 제약 사항 문서화

### Phase 4: Cleanup & Optimization (1-2일)

**작업**:

1. Session cleanup 로직 추가
2. Shell crash recovery 구현
3. Performance benchmark 실행
4. 문서 업데이트

**검증**:

- Benchmark 결과: 30% 이상 성능 개선
- Memory leak 없음 (Valgrind/Windows Performance Analyzer)
- 장시간 실행 안정성 테스트 (24시간)

### Phase 5: Cross-Platform 검증 (1-2일)

**작업**:

1. Linux 환경 테스트
2. macOS 환경 테스트 (가능한 경우)
3. CI/CD 파이프라인 업데이트
4. 플랫폼별 버그 수정

**검증**:

- GitHub Actions 모든 플랫폼 통과
- 크로스 플랫폼 regression test 통과

### Phase 6: Production Release (1일)

**작업**:

1. Feature flag default 설정 (`use_persistent_shell: true`)
2. Release notes 작성
3. User documentation 업데이트
4. Rollback plan 준비

**검증**:

- Beta 테스터 피드백 수집
- Production monitoring 설정
- Rollback 시나리오 테스트

**총 예상 기간**: 7-14일 (플랫폼 테스트 포함)

---

## ❓ Clarification Q-list (의사 결정 필요 사항)

### Q1: Feature Flag 전략

**질문**: Persistent shell을 기본적으로 활성화할 것인가, 아니면 opt-in으로 할 것인가?

**Option A**: Default enabled (`use_persistent_shell: true`)

- 장점: 즉시 성능 향상 및 상태 보존
- 단점: 예상치 못한 side effect 가능

**Option B**: Default disabled (`use_persistent_shell: false`)

- 장점: 안전한 rollout, 점진적 마이그레이션
- 단점: 사용자가 명시적으로 활성화해야 함

**Option C**: Session-level configuration

- 장점: 세션별로 persistent/one-shot 선택
- 단점: 복잡도 증가

**권장**: Option B → 검증 후 Option A로 전환

답변: Option A

### Q2: Shell Lifecycle 관리

**질문**: Persistent shell을 언제 종료할 것인가?

**Option A**: Session 종료 시 즉시

- 장점: 리소스 즉시 회수
- 단점: Session reconnect 시 상태 손실

**Option B**: 24시간 idle timeout

- 장점: Session reconnect 시 상태 유지
- 단점: 리소스 장기 점유

**Option C**: 명시적 `close_shell` tool

- 장점: 사용자 제어
- 단점: AI agent가 호출하지 않을 수 있음

**권장**: Option A + Option C 조합

답변: Option A

### Q3: Two-Tool Pattern stdin 처리

**질문**: Interactive 명령에서 stdin을 어떻게 주입할 것인가?

**Option A**: 명령 실행 전 stdin에 미리 작성

```rust
shell.stdin.write_all(user_input.as_bytes()).await?;
shell.stdin.write_all(b"\n").await?;
shell.execute(command).await?;
```

**Option B**: `execute_with_input()` 메서드 추가

```rust
shell.execute_with_input(command, user_input).await?;
```

**Option C**: One-shot으로 fallback

```rust
if require_user_input {
    // Use one-shot execution with stdin
    return execute_shell_with_isolation(...);
}
```

**권장**: Option A (POC 검증 후 결정)

**답변**: Option B - `execute_with_input()` 메서드 추가

**이유**:

- 현재 one-shot 구현이 이미 stdin 직접 주입 방식 사용 중 (`code_execution.rs:1219`)
- Persistent shell에서도 동일한 안전한 패턴 적용
- 명령 조합 방식 불필요 (보안성 동일, 구현 단순)

**구현 방법**:

```rust
impl PersistentShell {
    pub async fn execute_with_input(
        &mut self,
        command: &str,
        user_input: &str
    ) -> Result<(String, String, i32)> {
        // 1. stdin에 user_input 먼저 전달
        self.stdin.write_all(user_input.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;

        // 2. 명령 실행 (기존 execute 로직)
        self.execute(command).await
    }
}

// execute_pending_shell 통합:
if use_persistent_shell {
    shell_manager.execute_with_input(session_id, command, user_input).await?
} else {
    // 기존 one-shot 방식 (stdin 전달)
    execute_shell_with_isolation(...)
}
```

**보안성**: 기존 구현과 동일 - stdin pipe 사용으로 프로세스 명령줄에 노출되지 않음 ✅

**개인 PC 환경 고려사항**:

- **위험도**: 개인 PC(단일 사용자)에서는 명령 조합 방식도 실질적으로 큰 문제 없음
- **권장 이유**:
  - 구현 복잡도 동일 (stdin이 더 간단)
  - Defense in Depth 원칙 (악성코드 감염 시 stdin이 더 안전)
  - Best Practice 준수 (서버 환경 확장 대비)
- **결론**: stdin 방식 유지 (현재 구현과 일관성, 보안 습관 형성)

### Q4: Security Isolation 강화

**질문**: Persistent shell에서 환경변수 변경을 허용할 것인가?

**Scenario**:

```bash
# Agent executes
export PATH=/malicious:$PATH
# Now all subsequent commands use malicious PATH
```

**Option A**: 허용 (현재 POC 방식)

- 장점: 유연성 (venv 활성화 가능)
- 단점: 보안 리스크

**Option B**: 금지 (매 명령마다 env reset)

- 장점: 강력한 격리
- 단점: venv 등 상태 보존 불가

**Option C**: Whitelist 방식

- 장점: 허용된 변수만 변경 가능
- 단점: 복잡도 증가

**권장**: Option A (현재), Option C (향후 검토)

답변: 허용, 기본적으로 사용자가 할 수 있는 모든것을 하도록 함

### Q5: Error Recovery 전략

**질문**: Shell process가 crash 시 어떻게 처리할 것인가?

**Option A**: 자동 재생성

```rust
if shell.execute(command).await.is_err() {
    // Recreate shell and retry
    shell = PersistentShell::new(session_id).await?;
    shell.execute(command).await?;
}
```

**Option B**: Error 반환 + 명시적 재생성 tool

- Agent가 `reset_shell` tool 호출

**Option C**: One-shot으로 fallback

- Crash 감지 시 자동으로 one-shot 실행

**권장**: Option A (1회 retry) → 실패 시 Option C

답변: 권장 제안을 따름

---

## 📚 참고 자료

- [STDIO REPL POC](./stdio_repl_poc_20251122.md) - 검증 완료된 prototype
- [PTY Prototype POC](./pty_prototype_poc_20251122.md) - PTY 방식의 한계점 분석
- [Refactoring Plan (Original PTY)](./refactoring_20251122_1400.md) - PTY 기반 계획 (참고용)
- [Tokio Process Documentation](https://docs.rs/tokio/latest/tokio/process/)
- [AsyncBufRead Trait](https://docs.rs/tokio/latest/tokio/io/trait.AsyncBufRead.html)

---

## 🎯 최종 체크리스트

**구현 전 확인 사항**:

- [x] POC 코드 재검토 및 테스트 통과 확인 ✅
- [x] Two-Tool Pattern stdin 처리 방식 결정 (Q3) ✅ Option B: execute_with_input()
- [x] Feature flag 전략 결정 (Q1) ✅ Option A: Default enabled
- [x] Shell lifecycle 관리 방식 결정 (Q2) ✅ Option A: Session 종료 시 즉시
- [x] Security isolation 정책 결정 (Q4) ✅ Option A: 환경변수 변경 허용

**구현 중 확인 사항**:

- [ ] Unit test 모두 작성 및 통과
- [ ] Integration test 작성 및 통과
- [ ] Regression test 모두 통과 (기존 기능 영향 없음)
- [ ] Performance benchmark 실행 (30% 개선 확인)
- [ ] Memory leak 없음

**배포 전 확인 사항**:

- [ ] Cross-platform 테스트 통과 (Windows, Linux, macOS)
- [ ] 24시간 안정성 테스트 통과
- [ ] Documentation 업데이트 완료
- [ ] Rollback plan 준비 완료
- [ ] Beta 테스터 피드백 반영

---

**작성자 노트**:
이 계획은 POC 검증 결과를 바탕으로 작성되었습니다. 모든 의사결정(Q1-Q5)이 완료되었으며, stdin 방식의 `execute_with_input()` 메서드를 통한 Two-Tool Pattern 통합이 확정되었습니다. Phase 1부터 순차적으로 구현을 진행할 수 있습니다.

**주요 결정 사항**:

- ✅ Feature Flag: Default enabled (Option A)
- ✅ Shell Lifecycle: Session 종료 시 즉시 terminate (Option A)
- ✅ Two-Tool Pattern: `execute_with_input()` stdin 방식 (Option B)
- ✅ Security: 환경변수 변경 허용 (Option A)
- ✅ Error Recovery: 1회 retry → one-shot fallback (권장안)
