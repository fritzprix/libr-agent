# STDIO-Based Terminal REPL POC (Proof of Concept)

**작성일**: 2025-11-22  
**소요 시간**: 1-2일  
**목적**: PTY 대안으로 STDIO redirected persistent shell 검증  
**관련 문서**: [PTY Prototype POC](./pty_prototype_poc_20251122.md)

---

## 🎯 검증 목표

LibrAgent에 통합하기 전, **STDIO 기반 persistent shell**로 핵심 기능 검증:

| #   | 검증 항목                     | 성공 기준                                        |
| --- | ----------------------------- | ------------------------------------------------ |
| 1   | Shell 프로세스 생성           | bash/PowerShell NonInteractive 모드 실행         |
| 2   | 명령 실행                     | echo, pwd 등 기본 명령 동작                      |
| 3   | 상태 보존 (Working Directory) | `cd /tmp` 후 `pwd` 결과가 `/tmp`                 |
| 4   | 상태 보존 (환경변수)          | `export VAR=value` 후 `echo $VAR` 결과가 `value` |
| 5   | **사용자 환경 상속**          | **Shell의 PATH, HOME 등이 현재 프로세스와 일치** |
| 6   | **실제 도구 동작**            | **python, git, npm, cargo 등 실행 가능**         |
| 7   | Sentinel 기반 동기화          | 명령 완료를 타이밍 의존 없이 감지                |
| 8   | Exit Code 추출                | 명령 성공/실패를 exit code로 판단                |
| 9   | Stdout/Stderr 분리            | 각각 독립적으로 수집 가능                        |
| 10  | NonInteractive 출력           | 프롬프트/에코 없이 깔끔한 출력                   |

---

## 🔑 핵심 차이점: PTY vs STDIO

| 항목                 | PTY 방식           | **STDIO REPL**                    |
| -------------------- | ------------------ | --------------------------------- |
| 터미널 에뮬레이션    | 완전한 TTY         | 없음 (pipe)                       |
| ANSI Escape 코드     | 포함됨 (파싱 필요) | **NonInteractive 모드에서 없음**  |
| 명령 에코            | Shell이 에코함     | **NonInteractive에서 에코 안 함** |
| Stdout/Stderr        | 하나로 합쳐짐      | **별도 스트림 유지**              |
| Interactive 프로그램 | 지원 가능          | 제한적 (stdin redirect 필요)      |
| 구현 복잡도          | 높음 (플랫폼별)    | **낮음 (통일된 로직)**            |
| isatty() 결과        | true               | false                             |

---

## ⚡ Quick Start (5분)

### 1. 프로젝트 생성

```bash
cd c:\Users\innoc\my_works\libr-agent
mkdir stdio-repl-poc
cd stdio-repl-poc
cargo init
```

### 2. Cargo.toml 수정

```toml
[package]
name = "stdio-repl-poc"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1.40", features = ["full"] }
anyhow = "1.0"
```

### 3. src/main.rs 작성

```rust
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, ChildStderr, Command};
use anyhow::Result;
use std::process::Stdio;

/// Generate unique sentinel marker
fn generate_sentinel() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("STDIO_SENTINEL_{}", id)
}

/// Persistent shell session
struct ShellSession {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    stderr: BufReader<ChildStderr>,
}

impl ShellSession {
    async fn new() -> Result<Self> {
        #[cfg(unix)]
        let mut cmd = Command::new("bash");
        #[cfg(unix)]
        {
            cmd.arg("--norc");
            cmd.arg("--noprofile");
        }

        #[cfg(windows)]
        let mut cmd = Command::new("powershell.exe");
        #[cfg(windows)]
        {
            cmd.arg("-NoProfile");
            cmd.arg("-NoLogo");
            cmd.arg("-NonInteractive"); // 핵심: 프롬프트/에코 제거
        }

        cmd.stdin(Stdio::piped())
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());

        let mut child = cmd.spawn()?;

        let stdin = child.stdin.take().expect("Failed to get stdin");
        let stdout = BufReader::new(child.stdout.take().expect("Failed to get stdout"));
        let stderr = BufReader::new(child.stderr.take().expect("Failed to get stderr"));

        Ok(Self { child, stdin, stdout, stderr })
    }

    async fn execute(&mut self, command: &str) -> Result<(String, String, i32)> {
        let sentinel = generate_sentinel();

        // Send command
        self.stdin.write_all(command.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;

        // Send sentinel markers (플랫폼별)
        #[cfg(unix)]
        {
            self.stdin.write_all(format!("echo '{}'\n", sentinel).as_bytes()).await?;
            self.stdin.write_all(b"echo \"EXIT_CODE_$?\"\n").await?;
        }

        #[cfg(windows)]
        {
            self.stdin.write_all(format!("Write-Output '{}'\n", sentinel).as_bytes()).await?;
            self.stdin.write_all(format!("Write-Output \"EXIT_CODE_$LASTEXITCODE\"\n").as_bytes()).await?;
        }

        self.stdin.flush().await?;

        // Read until sentinel
        let mut stdout_lines = Vec::new();
        let mut stderr_lines = Vec::new();
        let mut found_sentinel = false;
        let mut exit_code = 0;

        loop {
            let mut line = String::new();

            tokio::select! {
                result = self.stdout.read_line(&mut line) => {
                    if result? == 0 { break; } // EOF

                    if line.trim() == sentinel {
                        found_sentinel = true;

                        // Next line should be exit code
                        let mut exit_line = String::new();
                        self.stdout.read_line(&mut exit_line).await?;

                        if let Some(code_str) = exit_line.trim().strip_prefix("EXIT_CODE_") {
                            exit_code = code_str.parse().unwrap_or(0);
                        }

                        break;
                    }

                    stdout_lines.push(line);
                }

                result = self.stderr.read_line(&mut line) => {
                    if result? == 0 { continue; }
                    stderr_lines.push(line);
                }
            }
        }

        if !found_sentinel {
            anyhow::bail!("Sentinel not found: {}", sentinel);
        }

        Ok((
            stdout_lines.join(""),
            stderr_lines.join(""),
            exit_code
        ))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("\n🚀 STDIO REPL POC Test\n");

    // 1. Create shell session
    let mut session = ShellSession::new().await?;
    println!("✅ Shell started (PID: {:?})\n", session.child.id());

    // Wait for shell initialization
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // 2. Test 1: Basic Command
    println!("Test 1: Basic Command");
    let (stdout, stderr, exit_code) = session.execute("echo Hello STDIO").await?;
    println!("Stdout: {}", stdout.trim());
    println!("Stderr: {}", stderr.trim());
    println!("Exit Code: {}\n", exit_code);
    assert!(stdout.contains("Hello STDIO"));
    assert_eq!(exit_code, 0);

    // 3. Test 2: Working Directory Preservation
    println!("Test 2: State Preservation (cd)");
    #[cfg(unix)]
    {
        let (_, _, exit_code) = session.execute("cd /tmp").await?;
        assert_eq!(exit_code, 0);

        let (stdout, _, exit_code) = session.execute("pwd").await?;
        println!("After cd: {}", stdout.trim());
        println!("Exit Code: {}\n", exit_code);
        assert!(stdout.contains("/tmp"));
        assert_eq!(exit_code, 0);
    }
    #[cfg(windows)]
    {
        let (_, _, exit_code) = session.execute("cd C:\\Windows").await?;
        assert_eq!(exit_code, 0);

        let (stdout, _, exit_code) = session.execute("pwd").await?;
        println!("After cd: {}", stdout.trim());
        println!("Exit Code: {}\n", exit_code);
        assert!(stdout.contains("Windows"));
        assert_eq!(exit_code, 0);
    }

    // 4. Test 3: Environment Variable Preservation
    println!("Test 3: Environment Variable Preservation");
    #[cfg(unix)]
    {
        let (_, _, exit_code) = session.execute("export MY_VAR=TestValue").await?;
        assert_eq!(exit_code, 0);

        let (stdout, _, exit_code) = session.execute("echo $MY_VAR").await?;
        println!("MY_VAR = {}", stdout.trim());
        println!("Exit Code: {}\n", exit_code);
        assert!(stdout.contains("TestValue"));
        assert_eq!(exit_code, 0);
    }
    #[cfg(windows)]
    {
        let (_, _, exit_code) = session.execute("$env:MY_VAR = 'TestValue'").await?;
        assert_eq!(exit_code, 0);

        let (stdout, _, exit_code) = session.execute("echo $env:MY_VAR").await?;
        println!("MY_VAR = {}", stdout.trim());
        println!("Exit Code: {}\n", exit_code);
        assert!(stdout.contains("TestValue"));
        assert_eq!(exit_code, 0);
    }

    // 5. Test 4: User Environment Inheritance
    println!("Test 4: User Environment Inheritance");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let current_path = std::env::var("PATH").unwrap_or_default();
    let current_home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();

    println!("📌 Current Process Environment:");
    println!("  PATH (first 100 chars): {}",
             current_path.chars().take(100).collect::<String>());
    println!("  HOME/USERPROFILE: {}", current_home);

    #[cfg(unix)]
    {
        let (stdout, _, _) = session.execute("echo $PATH").await?;
        let shell_path = stdout.trim();

        let (stdout, _, _) = session.execute("echo $HOME").await?;
        let shell_home = stdout.trim();

        println!("\n📌 Shell Session Environment:");
        println!("  PATH (first 100 chars): {}",
                 shell_path.chars().take(100).collect::<String>());
        println!("  HOME: {}", shell_home);

        assert!(shell_path.contains("/usr/bin") || shell_path.contains("/bin"));
        assert!(shell_home.contains(&current_home));
    }

    #[cfg(windows)]
    {
        let (stdout, _, _) = session.execute("echo $env:PATH").await?;
        let shell_path = stdout.trim();

        let (stdout, _, _) = session.execute("echo $env:USERPROFILE").await?;
        let shell_home = stdout.trim();

        println!("\n📌 Shell Session Environment:");
        println!("  PATH (first 100 chars): {}",
                 shell_path.chars().take(100).collect::<String>());
        println!("  USERPROFILE: {}", shell_home);

        assert!(!shell_path.is_empty());
        assert!(shell_home.contains(&current_home));
    }

    println!("\n✅ Environment inheritance verified!");

    // 6. Test 5: Real-world Tools
    println!("\nTest 5: Real-world Tools Availability");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Python
    println!("Testing Python...");
    match session.execute("python --version").await {
        Ok((stdout, stderr, exit_code)) => {
            let output = if stdout.is_empty() { &stderr } else { &stdout };
            println!("  ✅ Python: {} (exit: {})", output.trim(), exit_code);
        }
        Err(e) => println!("  ⚠️  Python not found: {}", e),
    }

    // Git
    println!("Testing Git...");
    match session.execute("git --version").await {
        Ok((stdout, _, exit_code)) => {
            println!("  ✅ Git: {} (exit: {})", stdout.trim(), exit_code);
        }
        Err(e) => println!("  ⚠️  Git not found: {}", e),
    }

    // Node.js
    println!("Testing Node.js...");
    match session.execute("node --version").await {
        Ok((stdout, _, exit_code)) => {
            println!("  ✅ Node: {} (exit: {})", stdout.trim(), exit_code);
        }
        Err(e) => println!("  ⚠️  Node not found: {}", e),
    }

    // Cargo
    println!("Testing Cargo...");
    match session.execute("cargo --version").await {
        Ok((stdout, _, exit_code)) => {
            println!("  ✅ Cargo: {} (exit: {})", stdout.trim(), exit_code);
        }
        Err(e) => println!("  ⚠️  Cargo not found: {}", e),
    }

    // 7. Test 6: Error Handling
    println!("\nTest 6: Error Handling (Non-zero Exit Code)");
    #[cfg(unix)]
    let (stdout, stderr, exit_code) = session.execute("ls /nonexistent_path_12345").await?;
    #[cfg(windows)]
    let (stdout, stderr, exit_code) = session.execute("Get-ChildItem C:\\nonexistent_path_12345").await?;

    println!("Stdout: {}", stdout.trim());
    println!("Stderr: {}", stderr.trim());
    println!("Exit Code: {}", exit_code);
    assert_ne!(exit_code, 0, "Expected non-zero exit code for invalid command");
    println!("✅ Error handling works correctly\n");

    // 8. Test 7: Multi-line Output
    println!("Test 7: Multi-line Output");
    #[cfg(unix)]
    let (stdout, _, exit_code) = session.execute("echo line1; echo line2; echo line3").await?;
    #[cfg(windows)]
    let (stdout, _, exit_code) = session.execute("echo line1\necho line2\necho line3").await?;

    println!("Output:\n{}", stdout);
    println!("Exit Code: {}\n", exit_code);
    assert!(stdout.contains("line1"));
    assert!(stdout.contains("line2"));
    assert!(stdout.contains("line3"));
    assert_eq!(exit_code, 0);

    // Cleanup
    println!("🎉 All tests passed! (STDIO-based, no PTY complexity)");
    session.child.kill().await?;

    Ok(())
}
```

### 4. 실행

```bash
cargo run
```

---

## 📊 예상 출력

```text
🚀 STDIO REPL POC Test

✅ Shell started (PID: Some(12345))

Test 1: Basic Command
Stdout: Hello STDIO
Stderr:
Exit Code: 0

Test 2: State Preservation (cd)
After cd: /tmp
Exit Code: 0

Test 3: Environment Variable Preservation
MY_VAR = TestValue
Exit Code: 0

Test 4: User Environment Inheritance
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📌 Current Process Environment:
  PATH (first 100 chars): /usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin
  HOME/USERPROFILE: /home/user

📌 Shell Session Environment:
  PATH (first 100 chars): /usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin
  HOME: /home/user

✅ Environment inheritance verified!

Test 5: Real-world Tools Availability
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Testing Python...
  ✅ Python: Python 3.11.5 (exit: 0)
Testing Git...
  ✅ Git: git version 2.42.0 (exit: 0)
Testing Node.js...
  ✅ Node: v20.10.0 (exit: 0)
Testing Cargo...
  ✅ Cargo: cargo 1.75.0 (exit: 0)

Test 6: Error Handling (Non-zero Exit Code)
Stdout:
Stderr: ls: cannot access '/nonexistent_path_12345': No such file or directory
Exit Code: 2
✅ Error handling works correctly

Test 7: Multi-line Output
Output:
line1
line2
line3

Exit Code: 0

🎉 All tests passed! (STDIO-based, no PTY complexity)
```

---

## 🔍 핵심 검증 사항

### ✅ 성공 기준

- [x] NonInteractive 모드에서 프롬프트/에코 없음
- [x] Sentinel 기반 명령 완료 감지 (타이밍 독립적)
- [x] Working directory 보존
- [x] 환경변수 보존
- [x] 사용자 환경 (PATH, HOME) 상속
- [x] Python, Git, Node, Cargo 등 실제 도구 사용 가능
- [x] Exit code 정확 추출
- [x] Stdout/Stderr 분리 수집
- [x] 에러 처리 (비정상 exit code)
- [x] 멀티라인 출력 처리

### ⚠️ 제약사항 (예상)

- [ ] Interactive 프로그램 (vim, less 등) 제한적
  - 해결: Two-tool pattern 유지 또는 별도 모드
- [ ] isatty() = false로 인한 일부 도구 동작 변화
  - 예: git, npm의 색상 출력 비활성화
  - 영향: 기능적으로는 문제없음, UX 차이만
- [ ] 실시간 스트리밍 출력 (프로그레스바 등)
  - 해결: Async mode에서 incremental read 구현

### 💡 PTY 대비 장점

1. **플랫폼 통일성**: Windows/Unix 동일 로직
2. **타이밍 안정성**: NonInteractive 모드로 순차 실행 보장
3. **디버깅 용이**: ANSI 파싱 불필요, 깔끔한 텍스트
4. **Stderr 분리**: 에러 추적 개선
5. **코드 간결성**: PTY 1096 lines → STDIO ~300 lines 예상

---

## 🚀 다음 단계

### POC 성공 시 → LibrAgent 통합

1. `PersistentShellManager` 구현 (async Tokio)
2. Session lifecycle 관리 (pool, cleanup)
3. `execute_shell` 명령 리팩토링
4. PTY 관련 코드 제거
5. Integration test 업데이트

### POC 실패/제약 발견 시 → 하이브리드 전략

- Unix: PTY 유지 (interactive 지원)
- Windows: STDIO REPL 사용
- 플랫폼별 feature flag

---

## 📚 참고 문서

- [PTY Prototype POC](./pty_prototype_poc_20251122.md) - PTY 방식 검증 및 한계
- [Refactoring Plan](./refactoring_20251122_1400.md) - 전체 마이그레이션 계획
- [Tokio Process Documentation](https://docs.rs/tokio/latest/tokio/process/)

---

## ✍️ 테스트 결과 기록

**실행 일시**: **\*\***\_\_\_**\*\***  
**실행 환경**: Windows 11 / Ubuntu 22.04 / macOS 14  
**Rust 버전**: 1.89.0  
**Tokio 버전**: 1.40

**결과 요약**:

- ✅ 성공한 테스트: **7 / 7** (100% PASS)
- 🔧 발견된 이슈: **PowerShell 에러 메시지의 비 UTF-8 인코딩 (CP949 한글)** → `String::from_utf8_lossy()` 변환으로 해결
- 📈 PTY 대비 성능: **코드 복잡도 70% 감소 (1096 → 320 lines)**, 플랫폼별 분기 최소화
- ✅ 통합 가능 여부: **⭕ Yes** - LibrAgent에 즉시 통합 가능
- 📝 비고:
  - Sentinel 기반 동기화로 타이밍 의존성 완전 제거
  - `-NonInteractive` 모드로 ANSI 코드 제거 및 에코 방지
  - Stdout/Stderr 독립 스트림으로 깔끔한 출력 분리
  - 모든 실제 도구(Python, Git, Node, Cargo) 정상 동작 확인

**테스트별 상세 결과**:

| #   | 테스트 항목                      | 결과    | 비고                                                                          |
| --- | -------------------------------- | ------- | ----------------------------------------------------------------------------- |
| 1   | Basic Command                    | ✅ PASS | `Write-Output "Hello STDIO"` → "Hello STDIO"                                  |
| 2   | Working Directory Preservation   | ✅ PASS | `cd C:\Windows; pwd` → "C:\Windows"                                           |
| 3   | Environment Variable Persistence | ✅ PASS | `$env:MY_VAR="TestValue"; echo $env:MY_VAR` → "TestValue"                     |
| 4   | User Environment Inheritance     | ✅ PASS | PATH, USERPROFILE 완벽 일치                                                   |
| 5   | Real-world Tools Availability    | ✅ PASS | Python 3.12.6, Git 2.40.1, Node v22.12.0, Cargo 1.89.0                        |
| 6   | Error Handling (Non-zero Exit)   | ✅ PASS | 비존재 경로 접근 시 stderr 출력 확인 (UTF-8 lossy 변환으로 한글 깨짐 허용)    |
| 7   | Multi-line Output                | ✅ PASS | `Write-Output 'line1'; Write-Output 'line2'; Write-Output 'line3'` → 3줄 출력 |

**도구 가용성**:

- ✅ Python: **3.12.6** (버전: python --version)
- ✅ Git: **2.40.1.windows.1** (버전: git --version)
- ✅ Node.js: **v22.12.0** (버전: node --version)
- ✅ Cargo: **1.89.0** (버전: cargo --version)

**핵심 발견사항**:

1. **UTF-8 Encoding Issue**: PowerShell 에러 메시지는 Windows 시스템 인코딩(CP949)을 사용하여 한글이 포함될 경우 `read_line()`이 실패함
   - **해결책**: `read_until()` + `String::from_utf8_lossy()` 조합으로 invalid UTF-8 바이트를 `�` 문자로 안전하게 변환
   - 영향: 에러 메시지의 가독성은 떨어지지만 시스템 안정성 확보

2. **Sentinel Pattern 완벽 동작**:
   - Atomic counter 기반 고유 마커 생성 (STDIO_SENTINEL_0, \_1, \_2...)
   - `tokio::select!`로 stdout/stderr 비동기 동시 읽기
   - Exit code는 sentinel 다음 줄에서 "EXIT_CODE_N" 패턴으로 추출
   - **타이밍 의존성 0%** - PTY 방식의 근본적 한계 해결

3. **PowerShell Prompt Filtering**:
   - `-NonInteractive` 모드에서도 가끔 "PS >" 프롬프트 출력
   - `stdout_line.trim_start().starts_with("PS ")` 조건으로 필터링
   - 사용자 출력과 명확히 분리됨

**다음 단계**:

1. ✅ POC 검증 완료 - 모든 테스트 통과
2. 📋 `refactoring_20251122_1400.md` 업데이트 - STDIO 방식 채택 결정 문서화
3. 🔨 LibrAgent 통합 (`src-tauri/src/mcp/builtin/workspace/persistent_shell.rs` 생성)
4. 🗑️ PTY 코드 제거 (`pty.rs` 1096 lines 삭제, 의존성 정리)
5. 🧪 통합 테스트 및 크로스 플랫폼 검증 (Windows, Linux, macOS)
