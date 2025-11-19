# Windows Command Execution Issue Analysis

## Overview

This document analyzes issues that occur when executing commands in the Windows environment.

## Test Environment

- **OS**: Windows
- **Environment**: PowerShell-based command execution
- **Test Date**: November 18, 2025

---

## 🔍 Root Cause Analysis

### Core Issue: Critical Flaw in Quote Normalization Logic

**Location**: `normalize_shell_command()` function in `src-tauri/src/mcp/builtin/workspace/code_execution.rs`

#### Problem Flow

1. **Input Command**: `python -c "print('Python inline test')"`
   - A command that should be passed normally to Python

2. **Normalization Process**:

   ```rust
   // Logic inside normalize_shell_command()
   // Convert single quotes to double quotes
   normalized = normalized.replace('\'', "\"");
   ```

   **Result**: `python -c "print("Python inline test")"`
   - All nested quotes are converted to double quotes
   - Results in syntactically incorrect Python command

3. **PowerShell Wrapping**:

   ```powershell
   powershell -Command "$ErrorActionPreference = 'Stop'; try { python -c `"print(`"Python inline test`")`" } catch { ... }"
   ```

   - The already incorrectly converted command is PowerShell-escaped

4. **Execution Result**:
   - Python parser recognizes `print("Python inline test")` as
   - `print("Python` , `inline` , `test")` separately
   - SyntaxError occurs
   - Exit code: 1
   - **238 bytes written to stderr file, but 0 bytes returned when reading**

#### Log Evidence

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

**Core Contradiction**: 238 bytes were written to the stderr file, but 0 bytes were read

---

## 🐛 Additional Issues Discovered

### Issue 1: Stderr File Read Failure

**Evidence**:

- Stderr streaming: 238 bytes written
- File read result: 0 bytes
- Actual error message not delivered to user

**Suspected Cause**:

- Attempting to read file before file handle is closed
- Asynchronous file I/O timing issue
- Possible missing flush() call

### Issue 2: Design Flaw in Quote Normalization Logic

**Current Logic**:

```rust
fn normalize_shell_command(input: &str) -> String {
    let mut normalized = input.to_string();
    // Unconditionally convert single quotes to double quotes
    normalized = normalized.replace('\'', "\"");
    // ... additional processing
    normalized
}
```

**Problems**:

1. **Ignores Context**: Converts all quotes including those inside strings
2. **No Support for Nested Quotes**: `"print('test')"` → `"print("test")"`
3. **Language-Specific Syntax Not Considered**: Breaks Python, JavaScript, etc. syntax

---

## 📋 Test Case Analysis

### Successful Cases

```bash
# Test 1: Simple echo command
echo "Simple test"
# Result: Success - "Simple test"

# Test 2: Echo without quotes
echo Simple test
# Result: Success - "Simple test"

# Test 3: dir command
# Result: Success - Directory listing displayed
```

### Python Command Execution (Failed)

```bash
# Test 4: Get-Command python
Get-Command python
# Result: Success - Python path confirmed
# Output:
# CommandType     Name                                               Version    Source
# -----------     ----                                               -------    ------
# Application     python.exe                                         3.12.61... C:\Python312\python.exe

# Test 5: Python execution with direct path
C:\Python312\python.exe -c "print('Direct path test')"
# Result: Failed - Exit code 1, no output

# Test 6: Python execution using PowerShell variable
$PYTHON_PATH = (Get-Command python).Source; & $PYTHON_PATH -c "print('Test using PowerShell call operator')"
# Result: Failed - Exit code 1
```

### PowerShell Command Execution (Failed)

```bash
# Test 7: Complex PowerShell command
powershell -Command "Write-Host 'PowerShell direct command'; $python = Get-Command python; Write-Host \"Python path: $python\"; & $python.Source -c \"print('Python test')\""
# Result: Failed - Exit code 1

# Test 8: Simplified PowerShell
powershell -NoProfile -NonInteractive -Command "Write-Host 'PowerShell direct'"
# Result: Failed - Exit code 1
```

---

## 🏗️ Current Architecture Analysis (As-Is)

### Async Mode and Poll Process Interaction Structure

#### 1. Data Structure (`terminal_manager.rs`)

```rust
pub enum ProcessStatus {
    Starting,  // Process being created
    Running,   // Executing
    Finished,  // Normal termination (exit_code == 0)
    Failed,    // Error termination (exit_code != 0)
    Killed,    // Terminated by user/system
}

pub struct ProcessEntry {
    pub id: String,                    // cuid2-generated process ID
    pub session_id: String,            // Session isolation
    pub command: String,               // Executed command
    pub status: ProcessStatus,
    pub pid: Option<u32>,              // OS process ID
    pub exit_code: Option<i32>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub stdout_path: String,           // File: workspace/tmp/process_{id}/stdout
    pub stderr_path: String,           // File: workspace/tmp/process_{id}/stderr
    pub stdout_size: u64,              // File size (bytes)
    pub stderr_size: u64,

    // Poll tracking (excessive polling detection)
    pub last_poll_at: Option<DateTime<Utc>>,
    pub poll_count: u32,
    pub consecutive_running_polls: u32,
    pub first_running_poll_at: Option<DateTime<Utc>>,
}
```

#### 2. Async Execution Flow

1. **Concurrent Execution Limit**: Maximum 20 processes per session
2. **Process ID Generation**: `cuid2::create_id()` → `workspace/tmp/process_{id}/`
3. **Registry Registration**: `status: Starting`, save stdout/stderr paths
4. **Background Task Creation**:
   ```rust
   tokio::spawn(async move {
       entry.status = Running;
       let result = spawn_and_stream_to_files(...).await;
       entry.status = Finished | Failed;
       entry.stdout_size = file_metadata.len();
   });
   ```
5. **Immediate Response**: Return `process_id`, "Use poll_process to check status"

#### 3. Poll Process Mechanism

**MCP Tool**: `poll_process(process_id, tail?: {src, n})`

**Operation Flow**:

1. Query `ProcessEntry` from Registry (session validation)
2. Update poll tracking:
   - `poll_count++`
   - If `Running` status: `consecutive_running_polls++`
   - Add guidance message if threshold (default 5) exceeded
3. Generate response:
   - `status`, `pid`, `exit_code`, `stdout_size`, `stderr_size`
   - Optional `tail`: Call `tail_lines(file_path, n)`
4. File-based output reading:
   ```rust
   // tail_lines() reads last N lines from file
   // Can be called while process is running but flush not guaranteed
   ```

#### 4. File Streaming Structure (`spawn_and_stream_to_files`)

```rust
// Stream stdout/stderr to files
tokio::spawn(async move {
    let mut file = File::create(stdout_path).await?;
    loop {
        let n = stdout.read(&mut buf).await?;
        if n == 0 { break; }
        file.write_all(&buf[..n]).await?;
        // ⚠️ Problem: No flush()!
    }
});

// Wait for process termination
child.wait().await?;
stdout_handle.await?;
stderr_handle.await?;

// ⚠️ Problem: File read immediately after streaming
// Attempting to read before file handle is dropped
let stdout = tokio::fs::read_to_string(&stdout_path).await?;
```

### Core Problems

#### 1. **Sync Mode Stderr Read Bug**

- Immediate file read after streaming completion → no flush guarantee
- Result: 238 bytes written but 0 bytes read

#### 2. **Lack of Long-running Process Support**

- Async mode uses file-based streaming
- `poll_process` reads from file but no real-time flush guarantee
- Server/watcher output may be delayed or missing

#### 3. **Quote Normalization Bug**

- `normalize_shell_command()` converts `'` → `"` indiscriminately
- Python `-c "print('test')"` → `-c "print("test")"` syntax error

### Current Support Scope

| Scenario               | Sync | Async | Notes                  |
| ---------------------- | ---- | ----- | ---------------------- |
| Simple commands (echo) | ✅   | ✅    | Normal                 |
| Python inline          | ❌   | ❌    | Quote bug              |
| Scripts under 30s      | ⚠️   | ✅    | Sync stderr bug        |
| Tasks over 30s         | ❌   | ✅    | Sync timeout           |
| Servers (npm run dev)  | ❌   | ⚠️    | Lack of real-time logs |
| Watch (vite)           | ❌   | ⚠️    | Lack of real-time logs |
| Interactive (REPL)     | ❌   | ❌    | stdin not supported    |

---

## 💡 Solutions

### Short-term Solution (Immediate Fix)

#### 1. Remove or Improve Quote Normalization Logic

**Option A: Disable Normalization**

```rust
fn normalize_shell_command(input: &str) -> String {
    // Windows PowerShell has excellent quote handling capability
    // Remove unnecessary normalization
    input.to_string()
}
```

**Option B: Smart Normalization (Context-Aware)**

```rust
fn normalize_shell_command(input: &str) -> String {
    // Preserve quotes inside string literals
    // Normalize only external quotes
    smart_quote_normalization(input)
}
```

#### 2. Fix Stderr File Read Synchronization

**Fundamental Limitation of Current Architecture**:

- File-based I/O only guarantees complete output when process terminates
- Long-running processes (servers, watch mode) need real-time output
- Current design only supports short-lived commands

**Option A: Hybrid Approach (Separate Short-lived + Long-running)**

```rust
#[derive(Debug)]
enum ExecutionMode {
    Sync,       // Wait for process termination (existing method)
    Background, // Background execution (new method)
}

async fn execute_command(
    cmd: &str,
    mode: ExecutionMode
) -> Result<CommandOutput> {
    match mode {
        ExecutionMode::Sync => {
            // Existing: file-based, wait for process termination
            let output = spawn_and_wait(cmd).await?;
            // Read file after process termination - sync guaranteed
            Ok(output)
        }
        ExecutionMode::Background => {
            // New: real-time streaming, return process handle
            let handle = spawn_background(cmd).await?;
            Ok(CommandOutput::Background {
                pid: handle.id(),
                log_path: handle.log_path(),
            })
        }
    }
}
```

**Option B: Real-time Streaming Architecture (Recommended)**

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

    // Real-time stdout streaming
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

    // Real-time stderr streaming
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

// MCP protocol-level streaming support
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
            // Same pattern
        }
    }
}
```

**Option C: File-based + Tail Pattern**

```rust
// Maintain existing file-based approach, but read tail -f style
async fn spawn_and_tail(cmd: &str) -> Result<ProcessHandle> {
    let stdout_path = create_temp_file("stdout");
    let stderr_path = create_temp_file("stderr");

    let mut child = Command::new("powershell")
        .args(&["-Command", &format!("{} > {} 2> {}",
            cmd, stdout_path, stderr_path)])
        .spawn()?;

    // Task to periodically read file
    tokio::spawn(async move {
        let mut last_pos = 0;
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;

            if let Ok(content) = tokio::fs::read_to_string(&stdout_path).await {
                if content.len() > last_pos {
                    let new_content = &content[last_pos..];
                    // Send new content
                    OUTPUT_CHANNEL.send(new_content).await;
                    last_pos = content.len();
                }
            }

            // Check process termination
            if child.try_wait()?.is_some() {
                break;
            }
        }
    });

    Ok(ProcessHandle { pid: child.id().unwrap() })
}
```

### Mid-term Solution (Architecture Improvement)

#### 1. Separate Execution Modes (Sync vs Background)

```rust
// Add execution_mode to MCP tool parameters
#[derive(Deserialize)]
struct ExecuteCommandArgs {
    command: String,
    #[serde(default)]
    execution_mode: ExecutionMode,  // "sync" | "background"
}

// Sync mode: existing behavior, wait for process termination
// Background mode: return process handle, real-time log streaming
```

#### 2. Introduce Process Management Layer

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
        // Return real-time or accumulated output
    }

    async fn stop(&self, pid: u32) -> Result<()> {
        // Terminate process
    }
}
```

#### 3. Extend MCP Tools

```rust
// Existing: execute_windows_cmd (sync only)
// New additions:
// - start_background_process: Start background process
// - get_process_output: Query process output
// - stop_process: Stop process
// - list_processes: List running processes
```

### Long-term Solution

#### 1. Unified Process Execution Architecture

```rust
// Unified system supporting all execution scenarios
pub struct UnifiedProcessExecutor {
    // Short-lived commands: return results immediately
    sync_executor: SyncExecutor,

    // Long-running processes: real-time streaming
    background_executor: BackgroundExecutor,

    // Interactive processes: bidirectional stdin/stdout communication
    interactive_executor: InteractiveExecutor,
}

// Usage example
match command_pattern {
    "npm run dev" | "python -m http.server" => {
        // Automatically switch to background mode
        executor.background_executor.spawn(cmd).await
    }
    "python -c \"...\"" | "echo ..." => {
        // Sync mode
        executor.sync_executor.run(cmd).await
    }
    "python" | "node" => {
        // Interactive mode (REPL)
        executor.interactive_executor.start(cmd).await
    }
}
```

#### 2. Real-time Streaming Protocol

```rust
// WebSocket-based real-time output streaming
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

#### 3. Language-Specific Execution Environments

```rust
// Optimized execution for major runtimes like Python, Node.js
trait LanguageRuntime {
    async fn execute_inline(&self, code: &str) -> Result<Output>;
    async fn start_repl(&self) -> Result<ReplSession>;
    async fn run_script(&self, path: &Path) -> Result<Output>;
}

struct PythonRuntime {
    interpreter_path: PathBuf,
    // Quote handling logic implemented to match language characteristics
}

impl LanguageRuntime for PythonRuntime {
    async fn execute_inline(&self, code: &str) -> Result<Output> {
        // Properly handle python -c "..."
        // Execute directly without PowerShell wrapping
        let output = Command::new(&self.interpreter_path)
            .arg("-c")
            .arg(code)  // No quote normalization!
            .output()
            .await?;
        Ok(output)
    }
}
```

#### 4. AI Agent-Friendly Interface

```typescript
// Easy-to-use abstraction on frontend
class ProcessExecutor {
  // Execute simple command
  async run(command: string): Promise<CommandResult> {
    return invoke('execute_command', { command, mode: 'sync' });
  }

  // Start server/watcher
  async startServer(command: string): Promise<ProcessHandle> {
    const handle = await invoke('start_background_process', { command });

    // Subscribe to real-time logs
    await listen(`process:${handle.pid}:stdout`, (event) => {
      console.log('[STDOUT]', event.payload);
    });

    return handle;
  }

  // Interactive session
  async startRepl(language: 'python' | 'node'): Promise<ReplSession> {
    return new ReplSession(language);
  }
}
```

---

## 🎯 Recommended Actions

### Immediate Actions (P0 - Critical)

1. **Disable `normalize_shell_command()` function**
   - Location: `src-tauri/src/mcp/builtin/workspace/code_execution.rs`
   - Simply modify to return input as-is

2. **Fix stderr synchronization for short-lived commands**
   - Wait until process terminates
   - Read file after process termination (automatically sync guaranteed)
   - Current bug: attempting to read file while process is running

### Short-term Actions (P1 - High)

1. **Implement Python inline execution directly**
   - Remove PowerShell wrapping
   - Execute `python.exe -c "code"` directly
   - Completely remove quote normalization

2. **Add execution mode detection logic**
   - Determine long-running status by command pattern
   - Auto-detect server/watcher commands: `npm run dev`, `vite`, `python -m http.server`, etc.
   - Auto-switch to background mode

3. **Improve error message delivery**
   - Sync mode: read stderr after process termination
   - Background mode: real-time stderr streaming

### Mid-term Actions (P2 - Medium)

1. **Build process management system**
   - Create global `ProcessManager` instance
   - Track processes by PID
   - Manage running process state

2. **Support background execution**
   - Add MCP tools: `start_background_process`, `get_process_output`, `stop_process`
   - Real-time stdout/stderr streaming (using broadcast channel)
   - Process lifecycle management

3. **Add integration tests**
   - Test short-lived commands
   - Test long-running processes (server start/stop)
   - Test real-time output streaming
   - Test Windows environment in CI

---

## 📊 Impact Scope

### Affected Features

- ✅ Basic Windows commands (dir, echo): **Normal**
- ❌ Python inline execution: **Complete failure**
- ❌ Node.js inline execution: **Expected failure**
- ❌ Commands using nested quotes: **Failure**
- ⚠️ PowerShell scripts: **Partial failures possible**

### Affected Users

- Users of AI agent code execution features
- Workflows requiring Python/Node.js script execution
- Users of complex command chains

---

## 🔬 Further Investigation Needed

1. **Identify cause of stderr file read failure**
   - Windows file system synchronization issue?
   - tokio async I/O bug?
   - File handle timing problem?

2. **Verify behavior on other platforms**
   - Does it work normally on Unix/Linux?
   - macOS test results?

3. **PowerShell version differences**
   - PowerShell 5.1 vs 7.x
   - Compare `-Command` vs `-File` options

---

## References

- Test date: November 18, 2025
- Environment: Windows PowerShell-based
- Execution method: Using builtin_workspace\_\_execute_windows_cmd
- Log file: `log.txt` (1811 lines)
- Diagnostic logs added (2025-11-18)
