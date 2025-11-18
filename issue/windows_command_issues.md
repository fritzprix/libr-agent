# Windows Command Execution Issue Analysis

## 개요

Windows 환경에서 명령어 실행 시 발생하는 문제들을 분석한 문서

## 테스트 환경

- **OS**: Windows
- **환경**: PowerShell 기반 명령 실행
- **테스트 일시**: 2025년 11월 18일

---

## 🔍 근본 원인 분석 (Root Cause Analysis)

### 핵심 문제: 따옴표(Quote) 정규화 로직의 치명적 결함

**위치**: `src-tauri/src/mcp/builtin/workspace/code_execution.rs`의 `normalize_shell_command()` 함수

#### 문제가 발생하는 흐름

1. **입력 명령어**: `python -c "print('Python inline test')"`
   - 정상적으로 Python에 전달되어야 하는 명령어

2. **정규화 과정**:

   ```rust
   // normalize_shell_command() 내부 로직
   // Single quotes를 double quotes로 변환
   normalized = normalized.replace('\'', "\"");
   ```

   **결과**: `python -c "print("Python inline test")"`
   - 중첩된 따옴표가 모두 double quote로 변환됨
   - Python 문법상 잘못된 명령어가 됨

3. **PowerShell 래핑**:

   ```powershell
   powershell -Command "$ErrorActionPreference = 'Stop'; try { python -c `"print(`"Python inline test`")`" } catch { ... }"
   ```

   - 이미 잘못 변환된 명령어가 PowerShell 이스케이프 처리됨

4. **실행 결과**:
   - Python 파서가 `print("Python inline test")` 대신
   - `print("Python` 와 `inline` , `test")` 로 인식
   - 구문 오류(SyntaxError) 발생
   - Exit code: 1
   - **stderr 파일에는 238 bytes 기록되었으나, 읽기 시 0 bytes 반환**

#### 로그 증거

```log
[14:23:11] Windows command normalized: python -c "print('Python inline test')"
           -> python -c "print("Python inline test")"

[14:23:11] PowerShell execution with error redirection:
           powershell -Command "$ErrorActionPreference = 'Stop';
           try { python -c `"print(`"Python inline test`")`" }
           catch { [Console]::Error.WriteLine($_.Exception.Message); exit 1 }"

[14:23:14] Process stderr streaming completed, total bytes: 238
[14:23:14] Process output sizes: stdout=0 bytes, stderr=0 bytes
[14:23:14] WARN: returned non-zero exit code 1 but both stdout and stderr are empty
```

**핵심 모순**: stderr 파일에는 238 bytes가 기록되었으나, 읽을 때는 0 bytes로 나옴

---

## 🐛 추가 발견된 문제들

### 문제 1: stderr 파일 읽기 실패

**증거**:

- stderr 스트리밍: 238 bytes 기록됨
- 파일 읽기 결과: 0 bytes
- 실제 오류 메시지가 사용자에게 전달되지 않음

**추정 원인**:

- 파일 핸들이 닫히기 전에 읽기 시도
- 비동기 파일 I/O 타이밍 문제
- 파일 flush() 미호출 가능성

### 문제 2: 따옴표 정규화 로직의 설계 결함

**현재 로직**:

```rust
fn normalize_shell_command(input: &str) -> String {
    let mut normalized = input.to_string();
    // Single quotes를 double quotes로 무조건 변환
    normalized = normalized.replace('\'', "\"");
    // ... 추가 처리
    normalized
}
```

**문제점**:

1. **컨텍스트 무시**: 문자열 내부의 따옴표까지 모두 변환
2. **중첩 따옴표 지원 불가**: `"print('test')"` → `"print("test")"`
3. **언어별 문법 미고려**: Python, JavaScript 등의 문법이 깨짐

---

## 📋 테스트 케이스 분석

### 성공 케이스

```bash
# 테스트 1: 간단한 echo 명령
echo "Simple test"
# 결과: 성공 - "Simple test"

# 테스트 2: 따옴표 없는 echo
echo Simple test
# 결과: 성공 - "Simple test"

# 테스트 3: dir 명령
# 결과: 성공 - 디렉토리 목록 출력
```

### 2. Python 명령 실행 (실패)

```bash
# 테스트 4: Get-Command python
Get-Command python
# 결과: 성공 - Python 경로 확인 가능
# Output:
# CommandType     Name                                               Version    Source
# -----------     ----                                               -------    ------
# Application     python.exe                                         3.12.61... C:\Python312\python.exe

# 테스트 5: 직접 경로로 Python 실행
C:\Python312\python.exe -c "print('Direct path test')"
# 결과: 실패 - Exit code 1, 출력 없음

# 테스트 6: PowerShell 변수로 Python 실행
$PYTHON_PATH = (Get-Command python).Source; & $PYTHON_PATH -c "print('Test using PowerShell call operator')"
# 결과: 실패 - Exit code 1
```

### 3. PowerShell 명령 실행 (실패)

```bash
# 테스트 7: 복잡한 PowerShell 명령
powershell -Command "Write-Host 'PowerShell direct command'; $python = Get-Command python; Write-Host \"Python path: $python\"; & $python.Source -c \"print('Python test')\""
# 결과: 실패 - Exit code 1

# 테스트 8: 간소화된 PowerShell
powershell -NoProfile -NonInteractive -Command "Write-Host 'PowerShell direct'"
# 결과: 실패 - Exit code 1
```

---

## 🏗️ 현재 아키텍처 분석 (As-Is)

### Async 모드와 Poll Process 상호작용 구조

#### 1. 데이터 구조 (`terminal_manager.rs`)

```rust
pub enum ProcessStatus {
    Starting,  // 프로세스 생성 중
    Running,   // 실행 중
    Finished,  // 정상 종료 (exit_code == 0)
    Failed,    // 오류 종료 (exit_code != 0)
    Killed,    // 사용자/시스템에 의해 종료
}

pub struct ProcessEntry {
    pub id: String,                    // cuid2 생성 프로세스 ID
    pub session_id: String,            // 세션 격리
    pub command: String,               // 실행된 명령어
    pub status: ProcessStatus,
    pub pid: Option<u32>,              // OS 프로세스 ID
    pub exit_code: Option<i32>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub stdout_path: String,           // 파일: workspace/tmp/process_{id}/stdout
    pub stderr_path: String,           // 파일: workspace/tmp/process_{id}/stderr
    pub stdout_size: u64,              // 파일 크기 (bytes)
    pub stderr_size: u64,

    // Poll 추적 (과도한 폴링 감지)
    pub last_poll_at: Option<DateTime<Utc>>,
    pub poll_count: u32,
    pub consecutive_running_polls: u32,
    pub first_running_poll_at: Option<DateTime<Utc>>,
}
```

#### 2. Async 실행 흐름

1. **동시 실행 제한**: 세션당 최대 20개 프로세스
2. **프로세스 ID 생성**: `cuid2::create_id()` → `workspace/tmp/process_{id}/`
3. **Registry 등록**: `status: Starting`, stdout/stderr 경로 저장
4. **백그라운드 태스크 생성**:
   ```rust
   tokio::spawn(async move {
       entry.status = Running;
       let result = spawn_and_stream_to_files(...).await;
       entry.status = Finished | Failed;
       entry.stdout_size = file_metadata.len();
   });
   ```
5. **즉시 응답**: `process_id` 반환, "Use poll_process to check status"

#### 3. Poll Process 메커니즘

**MCP 도구**: `poll_process(process_id, tail?: {src, n})`

**동작 흐름**:

1. Registry에서 `ProcessEntry` 조회 (세션 검증)
2. Poll 추적 업데이트:
   - `poll_count++`
   - `Running` 상태면 `consecutive_running_polls++`
   - Threshold(기본 5회) 초과 시 가이던스 메시지 추가
3. 응답 생성:
   - `status`, `pid`, `exit_code`, `stdout_size`, `stderr_size`
   - 선택적 `tail`: `tail_lines(file_path, n)` 호출
4. 파일 기반 출력 읽기:
   ```rust
   // tail_lines()는 파일에서 마지막 N줄 읽기
   // 프로세스 실행 중에도 호출 가능하지만 flush 보장 없음
   ```

#### 4. 파일 스트리밍 구조 (`spawn_and_stream_to_files`)

```rust
// stdout/stderr를 파일로 스트리밍
tokio::spawn(async move {
    let mut file = File::create(stdout_path).await?;
    loop {
        let n = stdout.read(&mut buf).await?;
        if n == 0 { break; }
        file.write_all(&buf[..n]).await?;
        // ⚠️ 문제: flush() 없음!
    }
});

// 프로세스 종료 대기
child.wait().await?;
stdout_handle.await?;
stderr_handle.await?;

// ⚠️ 문제: 스트리밍 직후 파일 읽기
// 파일 핸들이 drop되기 전에 읽기 시도
let stdout = tokio::fs::read_to_string(&stdout_path).await?;
```

### 핵심 문제점

#### 1. **Sync 모드의 stderr 읽기 버그**

- 스트리밍 완료 후 즉시 파일 읽기 → flush 보장 없음
- 결과: 238 bytes 쓰여졌으나 0 bytes 읽힘

#### 2. **Long-running 프로세스 지원 부족**

- Async 모드는 파일 기반 스트리밍
- `poll_process`는 파일에서 읽지만 실시간 flush 보장 없음
- 서버/워처는 출력이 지연되거나 누락될 수 있음

#### 3. **따옴표 정규화 버그**

- `normalize_shell_command()`가 `'` → `"` 일괄 변환
- Python `-c "print('test')"` → `-c "print("test")"` 구문 오류

### 현재 지원 범위

| 시나리오           | Sync | Async | 비고             |
| ------------------ | ---- | ----- | ---------------- |
| 단순 명령어 (echo) | ✅   | ✅    | 정상             |
| Python inline      | ❌   | ❌    | 따옴표 버그      |
| 30초 이하 스크립트 | ⚠️   | ✅    | Sync stderr 버그 |
| 30초 이상 작업     | ❌   | ✅    | Sync timeout     |
| 서버 (npm run dev) | ❌   | ⚠️    | 실시간 로그 부족 |
| Watch (vite)       | ❌   | ⚠️    | 실시간 로그 부족 |
| 대화형 (REPL)      | ❌   | ❌    | stdin 미지원     |

---

## 📋 테스트 케이스 분석

### ✅ 성공 케이스

```bash
# 1. 간단한 echo
echo "Hello World"
# 결과: 성공 - "Hello\r\nWorld"

# 2. Python 버전 확인
python --version
# 결과: 성공 - "Python 3.12.6"

# 3. where 명령
where python
# 결과: 성공 (no output)
```

### ❌ 실패 케이스

```bash
# 모든 Python -c 명령이 실패
python -c "print('Python inline test')"
python -c "print('Test with quotes'); print('Second line')"
C:\Python312\python.exe -c "print('Direct path test')"

# 공통점:
# - 중첩된 따옴표 사용
# - Exit code 1
# - stderr 238 bytes 기록, 0 bytes 읽힘
# - 실제 오류 메시지 전달 안 됨
```

---

## 💡 해결 방안

### 단기 해결책 (Immediate Fix)

#### 1. 따옴표 정규화 로직 제거 또는 개선

**Option A: 정규화 비활성화**

```rust
fn normalize_shell_command(input: &str) -> String {
    // Windows PowerShell은 따옴표 처리 능력이 우수함
    // 불필요한 정규화 제거
    input.to_string()
}
```

**Option B: 스마트 정규화 (컨텍스트 인식)**

```rust
fn normalize_shell_command(input: &str) -> String {
    // 문자열 리터럴 내부의 따옴표는 보존
    // 외부 따옴표만 정규화
    smart_quote_normalization(input)
}
```

#### 2. stderr 파일 읽기 동기화 수정

**현재 아키텍처의 근본적 한계**:

- 파일 기반 I/O는 프로세스 종료 시점에만 완전한 출력을 보장
- Long-running processes (서버, watch 모드)는 실시간 출력이 필요
- 현재 설계는 short-lived 명령어만 지원 가능

**Option A: 하이브리드 접근 (Short-lived + Long-running 분리)**

```rust
#[derive(Debug)]
enum ExecutionMode {
    Sync,       // 프로세스 종료 대기 (기존 방식)
    Background, // 백그라운드 실행 (새로운 방식)
}

async fn execute_command(
    cmd: &str,
    mode: ExecutionMode
) -> Result<CommandOutput> {
    match mode {
        ExecutionMode::Sync => {
            // 기존: 파일 기반, 프로세스 종료 대기
            let output = spawn_and_wait(cmd).await?;
            // 프로세스 종료 후 파일 읽기 - sync 보장됨
            Ok(output)
        }
        ExecutionMode::Background => {
            // 신규: 실시간 스트리밍, 프로세스 핸들 반환
            let handle = spawn_background(cmd).await?;
            Ok(CommandOutput::Background {
                pid: handle.id(),
                log_path: handle.log_path(),
            })
        }
    }
}
```

**Option B: 실시간 스트리밍 아키텍처 (권장)**

```rust
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::broadcast;

struct ProcessHandle {
    pid: u32,
    stdout_channel: broadcast::Receiver<String>,
    stderr_channel: broadcast::Receiver<String>,
    status: Arc<Mutex<ProcessStatus>>,
}

async fn spawn_with_streaming(cmd: &str) -> Result<ProcessHandle> {
    let mut child = Command::new("powershell")
        .args(&["-Command", cmd])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let (stdout_tx, stdout_rx) = broadcast::channel(1000);
    let (stderr_tx, stderr_rx) = broadcast::channel(1000);

    // stdout 실시간 스트리밍
    if let Some(stdout) = child.stdout.take() {
        let stdout_tx = stdout_tx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = stdout_tx.send(line);
            }
        });
    }

    // stderr 실시간 스트리밍
    if let Some(stderr) = child.stderr.take() {
        let stderr_tx = stderr_tx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = stderr_tx.send(line);
            }
        });
    }

    Ok(ProcessHandle {
        pid: child.id().unwrap(),
        stdout_channel: stdout_rx,
        stderr_channel: stderr_rx,
        status: Arc::new(Mutex::new(ProcessStatus::Running)),
    })
}

// MCP 프로토콜 레벨에서 스트리밍 지원
#[tauri::command]
async fn stream_process_output(
    pid: u32,
    stream_type: StreamType,
) -> Result<Vec<String>> {
    let handle = PROCESS_MANAGER.get_handle(pid)?;

    match stream_type {
        StreamType::Stdout => {
            let mut rx = handle.stdout_channel.resubscribe();
            let mut lines = Vec::new();
            while let Ok(line) = rx.try_recv() {
                lines.push(line);
            }
            Ok(lines)
        }
        StreamType::Stderr => {
            // 동일한 패턴
        }
    }
}
```

**Option C: 파일 기반 + Tail 패턴**

```rust
// 기존 파일 기반 유지하되, tail -f 방식으로 읽기
async fn spawn_and_tail(cmd: &str) -> Result<ProcessHandle> {
    let stdout_path = create_temp_file("stdout");
    let stderr_path = create_temp_file("stderr");

    let mut child = Command::new("powershell")
        .args(&["-Command", &format!("{} > {} 2> {}",
            cmd, stdout_path, stderr_path)])
        .spawn()?;

    // 파일을 주기적으로 읽는 태스크
    tokio::spawn(async move {
        let mut last_pos = 0;
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;

            if let Ok(content) = tokio::fs::read_to_string(&stdout_path).await {
                if content.len() > last_pos {
                    let new_content = &content[last_pos..];
                    // 새 내용 전송
                    OUTPUT_CHANNEL.send(new_content).await;
                    last_pos = content.len();
                }
            }

            // 프로세스 종료 확인
            if child.try_wait()?.is_some() {
                break;
            }
        }
    });

    Ok(ProcessHandle { pid: child.id().unwrap() })
}
```

### 중기 해결책 (Architecture Improvement)

#### 1. 실행 모드 분리 (Sync vs Background)

```rust
// MCP 도구 파라미터에 execution_mode 추가
#[derive(Deserialize)]
struct ExecuteCommandArgs {
    command: String,
    #[serde(default)]
    execution_mode: ExecutionMode,  // "sync" | "background"
}

// Sync 모드: 기존 동작, 프로세스 종료 대기
// Background 모드: 프로세스 핸들 반환, 실시간 로그 스트리밍
```

#### 2. 프로세스 관리 레이어 도입

```rust
struct ProcessManager {
    processes: Arc<Mutex<HashMap<u32, ProcessHandle>>>,
}

impl ProcessManager {
    async fn spawn(&self, cmd: &str, mode: ExecutionMode) -> Result<ProcessInfo> {
        match mode {
            ExecutionMode::Sync => self.spawn_sync(cmd).await,
            ExecutionMode::Background => self.spawn_background(cmd).await,
        }
    }

    async fn get_output(&self, pid: u32, stream: StreamType) -> Result<String> {
        // 실시간 또는 누적 출력 반환
    }

    async fn stop(&self, pid: u32) -> Result<()> {
        // 프로세스 종료
    }
}
```

#### 3. MCP 도구 확장

```rust
// 기존: execute_windows_cmd (sync만 지원)
// 신규 추가:
// - start_background_process: 백그라운드 프로세스 시작
// - get_process_output: 프로세스 출력 조회
// - stop_process: 프로세스 중단
// - list_processes: 실행 중인 프로세스 목록
```

### 장기 해결책 (Long-term Solution)

#### 1. 통합 프로세스 실행 아키텍처

```rust
// 모든 실행 시나리오를 지원하는 통합 시스템
pub struct UnifiedProcessExecutor {
    // Short-lived 명령어: 즉시 결과 반환
    sync_executor: SyncExecutor,

    // Long-running 프로세스: 실시간 스트리밍
    background_executor: BackgroundExecutor,

    // 대화형 프로세스: stdin/stdout 양방향 통신
    interactive_executor: InteractiveExecutor,
}

// 사용 예시
match command_pattern {
    "npm run dev" | "python -m http.server" => {
        // 백그라운드 모드로 자동 전환
        executor.background_executor.spawn(cmd).await
    }
    "python -c \"...\"" | "echo ..." => {
        // Sync 모드
        executor.sync_executor.run(cmd).await
    }
    "python" | "node" => {
        // 대화형 모드 (REPL)
        executor.interactive_executor.start(cmd).await
    }
}
```

#### 2. 실시간 스트리밍 프로토콜

```rust
// WebSocket 기반 실시간 출력 스트리밍
#[tauri::command]
async fn subscribe_process_output(
    window: tauri::Window,
    pid: u32,
) -> Result<()> {
    let handle = PROCESS_MANAGER.get_handle(pid)?;

    tokio::spawn(async move {
        let mut stdout_rx = handle.stdout_channel.subscribe();
        let mut stderr_rx = handle.stderr_channel.subscribe();

        loop {
            tokio::select! {
                Ok(line) = stdout_rx.recv() => {
                    window.emit("process:stdout", ProcessOutput {
                        pid,
                        stream: "stdout",
                        content: line,
                    }).ok();
                }
                Ok(line) = stderr_rx.recv() => {
                    window.emit("process:stderr", ProcessOutput {
                        pid,
                        stream: "stderr",
                        content: line,
                    }).ok();
                }
                else => break,
            }
        }
    });

    Ok(())
}
```

#### 3. 언어별 전용 실행 환경

```rust
// Python, Node.js 등 주요 런타임의 최적화된 실행
trait LanguageRuntime {
    async fn execute_inline(&self, code: &str) -> Result<Output>;
    async fn start_repl(&self) -> Result<ReplSession>;
    async fn run_script(&self, path: &Path) -> Result<Output>;
}

struct PythonRuntime {
    interpreter_path: PathBuf,
    // 따옴표 처리 로직이 언어 특성에 맞게 구현됨
}

impl LanguageRuntime for PythonRuntime {
    async fn execute_inline(&self, code: &str) -> Result<Output> {
        // python -c "..." 를 올바르게 처리
        // PowerShell 래핑 없이 직접 실행
        let output = Command::new(&self.interpreter_path)
            .arg("-c")
            .arg(code)  // 따옴표 정규화 없음!
            .output()
            .await?;
        Ok(output)
    }
}
```

#### 4. AI 에이전트 친화적 인터페이스

```typescript
// Frontend에서 사용하기 쉬운 추상화
class ProcessExecutor {
  // 단순 명령어 실행
  async run(command: string): Promise<CommandResult> {
    return invoke('execute_command', { command, mode: 'sync' });
  }

  // 서버/워처 시작
  async startServer(command: string): Promise<ProcessHandle> {
    const handle = await invoke('start_background_process', { command });

    // 실시간 로그 구독
    await listen(`process:${handle.pid}:stdout`, (event) => {
      console.log('[STDOUT]', event.payload);
    });

    return handle;
  }

  // 대화형 세션
  async startRepl(language: 'python' | 'node'): Promise<ReplSession> {
    return new ReplSession(language);
  }
}
```

---

## 🎯 권장 조치 사항

### 즉시 조치 (P0 - Critical)

1. **`normalize_shell_command()` 함수 비활성화**
   - 위치: `src-tauri/src/mcp/builtin/workspace/code_execution.rs`
   - 단순히 input을 그대로 반환하도록 수정

2. **Short-lived 명령어에 대한 stderr 동기화 수정**
   - 프로세스가 종료될 때까지 대기
   - 프로세스 종료 후 파일 읽기 (자동으로 sync 보장됨)
   - 현재 버그: 프로세스 실행 중에 파일을 읽으려고 시도

### 단기 조치 (P1 - High)

1. **Python inline 실행 직접 구현**
   - PowerShell 래핑 제거
   - `python.exe -c "code"` 직접 실행
   - 따옴표 정규화 완전히 제거

2. **실행 모드 감지 로직 추가**
   - 명령어 패턴으로 long-running 여부 판단
   - 서버/워처 명령어 자동 감지: `npm run dev`, `vite`, `python -m http.server` 등
   - Background 모드로 자동 전환

3. **에러 메시지 전달 개선**
   - Sync 모드: 프로세스 종료 후 stderr 읽기
   - Background 모드: 실시간 stderr 스트리밍

### 중기 조치 (P2 - Medium)

1. **프로세스 관리 시스템 구축**
   - `ProcessManager` 글로벌 인스턴스 생성
   - PID 기반 프로세스 추적
   - 실행 중인 프로세스 상태 관리

2. **Background 실행 지원**
   - MCP 도구 추가: `start_background_process`, `get_process_output`, `stop_process`
   - 실시간 stdout/stderr 스트리밍 (broadcast channel 사용)
   - 프로세스 생명주기 관리

3. **통합 테스트 추가**
   - Short-lived 명령어 테스트
   - Long-running 프로세스 테스트 (서버 시작/중지)
   - 실시간 출력 스트리밍 테스트
   - CI에서 Windows 환경 테스트

---

## 📊 영향 범위

### 영향받는 기능

- ✅ 기본 Windows 명령 (dir, echo): **정상**
- ❌ Python inline 실행: **완전 실패**
- ❌ Node.js inline 실행: **실패 예상**
- ❌ 중첩 따옴표 사용 명령: **실패**
- ⚠️ PowerShell 스크립트: **부분 실패 가능**

### 영향받는 사용자

- AI 에이전트의 코드 실행 기능 사용자
- Python/Node.js 스크립트 실행이 필요한 워크플로우
- 복잡한 명령어 체인 사용자

---

## 🔬 추가 조사 필요 사항

1. **stderr 파일 읽기 실패 원인 규명**
   - Windows 파일 시스템 동기화 이슈?
   - tokio async I/O 버그?
   - 파일 핸들 타이밍 문제?

2. **다른 플랫폼에서의 동작 확인**
   - Unix/Linux에서는 정상 동작하는지?
   - macOS에서의 테스트 결과는?

3. **PowerShell 버전별 차이**
   - PowerShell 5.1 vs 7.x
   - `-Command` vs `-File` 옵션 비교

---

## 참고 사항

- 테스트 일자: 2025년 11월 18일
- 환경: Windows PowerShell 기반
- 실행 방식: builtin_workspace\_\_execute_windows_cmd 사용
- 로그 파일: `log.txt` (1811 lines)
- 진단 로그 추가됨 (2025-11-18)
