# Docker Workspace 지원 구조 심층 리뷰

**작성일:** 2026-07-05
**리뷰 대상:** LibrAgent Docker Workspace Isolation 구현
**리뷰 범위:** AI Agent path 인식, runShell CWD 매핑, OS/Platform 차이, 잠재적 문제점

---

## 1. 시스템 아키텍처 개요

Docker Workspace Isolation은 세션당 컨테이너를 생성하여 AI Agent의 shell 명령이 호스트가 아닌 컨테이너 내부에서 실행되도록 하는 기능입니다.

### 핵심 데이터 흐름

```
[AI Agent] → [MCP Tool 호출] → [WorkspaceServer] → [SessionIsolationManager]
                                                        ↓
                                              Docker 모드 감지 → docker exec -i -w /workspace
```

### 관련 파일 구조

| 파일 경로                                                                | 역할                                                                            |
| ------------------------------------------------------------------------ | ------------------------------------------------------------------------------- |
| `docker/Dockerfile`                                                      | 베이스 이미지 (Debian Bookworm Slim)                                            |
| `src-tauri/src/models/workspace_isolation.rs`                            | `WorkspaceIsolationMode` (Host/Docker), `DockerWorkspaceConfig`                 |
| `src-tauri/src/agent/lifecycle/creation.rs`                              | 세션 생성 시 Docker 컨테이너 생성                                               |
| `src-tauri/src/entity/session.rs`                                        | DB 엔티티: `DockerContainerName`, `DockerHostWorkspacePath`, `DockerConfigJson` |
| `src-tauri/src/services/workspace_runtime_manager.rs`                    | Docker 런타임 관리 (컨테이너 생성/삭제/검증)                                    |
| `src-tauri/src/session_isolation/mod.rs`                                 | `SessionIsolationManager` — Docker 모드 감지 및 라우팅                          |
| `src-tauri/src/session_isolation/path_mapper.rs`                         | `PathMappingLayer` — 호스트↔컨테이너 경로 매핑                                  |
| `src-tauri/src/mcp/builtin/workspace/persistent_shell/manager.rs`        | Docker persistent shell 생성 (`spawn_docker_persistent_shell`)                  |
| `src-tauri/src/mcp/builtin/workspace/code_execution/shell/handlers.rs`   | `runInPersistentShell` 핸들러                                                   |
| `src-tauri/src/mcp/builtin/workspace/code_execution/shell/isolated.rs`   | `runShell` (isolated) 핸들러                                                    |
| `src-tauri/src/mcp/builtin/workspace/code_execution/shell/async_exec.rs` | `spawnProcess` 핸들러                                                           |
| `src-tauri/src/mcp/builtin/workspace/context.rs`                         | 서비스 컨텍스트 프롬프트 빌드                                                   |
| `src-tauri/src/mcp/builtin/workspace/workspace_server.rs`                | 파일 도구 Docker 경로 매핑 (`map_docker_container_file_tool_path`)              |

---

## 2. AI Agent가 인지하는 Workspace Path

### 2.1 컨텍스트 프롬프트 (Service Context)

`build_context_prompt()` (`context.rs`)에서 AI에게 전달되는 정보:

```
## Workspace

### Live State
- Workspace Root: <호스트 경로 예: /home/user/.local/share/com.fritzprix.libragent/workspaces/session-xxx>
- Persistent Shell CWD: .  (또는 상대 경로)
- Running Processes: ...
```

**핵심 문제점:**

- `Workspace Root`에 **호스트** 경로가 표시됩니다.
- `Persistent Shell CWD`는 컨테이너 내부 기준으로 `.` 또는 `/workspace` 기준 상대 경로로 표시됩니다.
- AI는 "내 워크스페이스는 호스트의 `/home/user/...`에 있다"고 인식하지만, 실제 shell 명령은 **컨테이너 내부 `/workspace`**에서 실행됩니다.

### 2.2 파일 도구 (readFile, listDirectory, writeDirectory 등)

`workspace_server.rs`의 `map_docker_container_file_tool_path()`:

- 경로가 `/`로 시작하고 Docker 모드일 때 → `PathMappingLayer.container_to_host()` 호출
- `/workspace/src/main.rs` → 호스트 경로 `/home/user/.../workspaces/session-xxx/workspace/src/main.rs`

**이 부분은 정상적으로 동작** — AI가 `/workspace` 경로를 사용하면 호스트 경로로 매핑됩니다.

### 2.3 runInPersistentShell 응답

```
Command executed in 5ms (exit code: 0)

Command output:
...

Persistent shell state (maintained for next runInPersistentShell call):
  Working directory: .
  Exit code: 0
```

- `Working directory`는 `display_shell_cwd()`로 변환된 값: 컨테이너 `/workspace` 기준 상대 경로
- `structured_content`에 `cwd`는 포함되지 않음 (AI가 볼 수 있는 영역)

### 2.4 runShell / spawnProcess 응답

```
Background process started successfully
• Process ID: abc123
• Command: pnpm build
• Mode: Asynchronous (non-blocking)
```

**문제점:** CWD 정보가 응답에 포함되지 않습니다. AI는 실행 디렉토리를 알 수 없습니다.

---

## 3. runShell/spawnProcess 시 실제 CWD 매핑

### 3.1 runShell (isolated execution)

```
흐름:
1. handlers.rs → handle_run_shell()
2. session_manager.get_session_workspace_dir_by_id(session_id) → 호스트 경로
3. apply_shell_policy_block() → 호스트 경로 기준 정책 검증
4. isolation_manager.create_isolated_command(IsolatedProcessConfig { workspace_path: 호스트 경로 })
5. mod.rs → Docker 모드 감지 → WorkspaceRuntimeManager.create_docker_exec_command()
6. docker exec -i -w /workspace <container> bash -lc "<command>"
```

**실제 CWD:** 컨테이너 내부 `/workspace` (docker exec의 `-w /workspace` 플래그)
**AI 인식:** runShell 응답에 CWD 정보가 없음

### 3.2 spawnProcess (async execution)

```
흐름:
1. handlers.rs → handle_spawn_process()
2. 동일하게 isolation_manager.create_isolated_command() → Docker exec 경로
3. process::spawn_and_stream_hybrid()로 비동기 실행
```

**실제 CWD:** 컨테이너 내부 `/workspace`
**AI 인식:** process_id만 반환, CWD 정보 없음

### 3.3 runInPersistentShell (persistent execution)

```
흐름:
1. handlers.rs → handle_execute_shell()
2. shell_manager.get_or_create_shell(session_id, workspace_path)
3. Docker 모드 감지 → WorkspaceRuntimeManager.spawn_docker_persistent_shell()
4. docker exec -i -w /workspace <container> bash --norc --noprofile
5. PersistentShell.from_spawned() → path_mapper 설정됨
6. 실행: shell.execute(command) → CWD 캡처 후 반환
```

**실제 CWD:** 컨테이너 내부, `cd` 등으로 변경 가능
**AI 인식:** `Working directory: <relative path from /workspace>`

---

## 4. OS/Platform 차이로 인한 문제점

### 4.1 Docker 컨테이너 OS vs 호스트 OS 불일치

- **호스트:** macOS, Windows, Linux 모두 가능
- **컨테이너:** 항상 Linux (Debian Bookworm Slim)
- **영향:**
  - Windows 호스트에서 `docker exec`으로 실행되는 명령은 Linux bash에서 실행됨
  - Windows 전용 명령어나 경로 구분자(`\`)가 포함된 명령은 컨테이너 내부에서 실패
  - `validate_windows_shell_syntax()`는 Windows에서만 호출되므로, Docker 모드에서는 Windows 전용 구문 검증이 누락됨

### 4.2 PathMappingLayer — 호스트↔컨테이너 경로 매핑

`path_mapper.rs`:

```rust
pub struct PathMappingLayer {
    host_workspace: PathBuf,        // 호스트 경로 (예: /home/user/... 또는 C:\Users\...)
    container_workspace: PathBuf,   // 항상 "/workspace"
}

pub fn container_to_host(&self, container_path: &str) -> Option<PathBuf> {
    let normalized = PathBuf::from(container_path.replace('\\', "/")).clean();
    if normalized == self.container_workspace { return Some(self.host_workspace.clone()); }
    if normalized.starts_with(&self.container_workspace) {
        let relative = normalized.strip_prefix(&self.container_workspace).ok()?;
        return Some(self.host_workspace.join(relative));
    }
    None
}
```

**문제점:**

- `container_workspace`는 항상 `/workspace` (포워드 슬래시)
- `host_workspace`는 플랫폼에 따라 다름:
  - Linux/macOS: `/home/user/...` → 정상 동작
  - **Windows:** `C:\Users\...` → `strip_prefix`가 실패할 수 있음 (경로 구분자 불일치)
- `strip_prefix`는 경로 구분자가 일치해야 성공 → **Windows에서 Docker 컨테이너 경로 매핑이 실패할 수 있음**

### 4.3 persistent shell — Windows 미지원

`persistent_shell/manager.rs`:

```rust
#[cfg(unix)]
let shell_type = ShellType::Bash;
#[cfg(windows)]
let shell_type = ShellType::PowerShell;
```

- Docker 컨테이너는 Linux 기반 → `bash` 또는 `sh`만 사용 가능
- Windows 호스트에서 Docker persistent shell 생성 시:
  - `spawn_docker_persistent_shell()` → `bash --norc --noprofile` 실행
  - Windows 호스트에서는 Docker Desktop 필요 → Docker 미설치 시 전체 기능 불가

### 4.4 current_uid_gid() — Unix 전용

`workspace_runtime_manager.rs`:

```rust
#[cfg(unix)]
async fn current_uid_gid() -> Option<String> { ... }

#[cfg(not(unix))]
async fn current_uid_gid() -> Option<String> { None }
```

- Windows 호스트에서 Docker 컨테이너 생성 시 `--user` 플래그가 적용되지 않음
- 컨테이너 내부 파일 소유권이 호스트 파일 소유권과 다를 수 있음 → 권한 문제 발생 가능

---

## 5. 구현의 잠재적 문제점

### 5.1 [P0-Critical] Docker 컨테이너 경로 매핑 — Windows 호스트에서 실패

**파일:** `session_isolation/path_mapper.rs`
**문제:** `container_workspace`가 항상 `/workspace`이고, `host_workspace`는 Windows에서 `C:\Users\...`와 같은 백슬래시 경로
**영향:** Windows에서 Docker 모드 사용 시 `readFile`, `listDirectory` 등이 컨테이너 경로를 호스트 경로로 매핑하지 못함
**해결 방향:** `container_workspace`와 `host_workspace` 모두 포워드 슬래시로 정규화하여 비교

### 5.2 [P0-Critical] runShell 응답에 CWD 정보 미포함

**파일:** `code_execution/shell/isolated.rs`, `async_exec.rs`
**문제:** `runShell`과 `spawnProcess` 응답에 실제 실행 CWD 정보가 없음
**영향:** AI Agent가 "어디에서 명령이 실행되었는지" 알 수 없음 → 컨테이너 내부 `/workspace` 인지 호스트인지 혼동 가능
**해결 방향:** 응답에 `cwd` 필드 추가 (예: `"cwd": "/workspace"`)

### 5.3 [P1-High] 컨텍스트 프롬프트 — 호스트 경로 vs 컨테이너 경로 혼란

**파일:** `mcp/builtin/workspace/context.rs`
**문제:** `Workspace Root`에 호스트 경로 표시, `Persistent Shell CWD`에 컨테이너 기준 경로 표시
**영향:** AI가 "워크스페이스 루트"와 "셸 CWD"가 다른 공간이라고 인식할 수 있음
**해결 방향:** Docker 모드일 때 `Workspace Root`를 `/workspace`로 표시하거나, 두 값을 명확히 구분

### 5.4 [P1-High] Docker 모드에서 isolated shell 실행 시 CWD 미표시

**파일:** `code_execution/shell/isolated.rs`
**문제:** `execute_shell_with_isolation()` 응답에 CWD가 포함되지 않음
**영향:** `runShell` 실행 후 AI가 다음 명령의 기준 디렉토리를 알 수 없음

### 5.5 [P1-High] 호스트 파일시스템 vs 컨테이너 파일시스템 분리

**파일:** `services/workspace_runtime_manager.rs`
**문제:**

- `runInPersistentShell`은 컨테이너 내부 파일시스템을 변경 (예: `npm install`, `pip install`)
- `readFile`, `listDirectory` 등은 호스트 파일시스템을 참조
  **영향:**
- 컨테이너 내부에 설치한 패키지가 호스트에서는 보이지 않음
- `runInPersistentShell`로 생성한 파일이 `listDirectory`에 안 보일 수 있음
  **해결 방향:**
- Docker 모드에서 `runInPersistentShell` 실행 후 컨테이너 파일시스템 스냅샷을 호스트에 반영
- 또는 AI에게 "이 명령은 컨테이너 내부에서 실행되며, 파일은 /workspace 마운트에 영구 저장됨" 안내

### 5.6 [P2-Medium] Docker exec 오버헤드

**파일:** `session_isolation/mod.rs`
**문제:** 매 shell 명령마다 `docker exec` 프로세스가 생성됨
**영향:**

- 각 명령마다 ~5-20ms의 Docker CLI 오버헤드 발생
- `runInPersistentShell`은 persistent shell로 이 문제를 완화
  **해결 방향:** `runShell`도 Docker persistent shell을 활용할 수 있는 옵션 제공

### 5.7 [P2-Medium] ensure_runtime — 매 명령마다 Docker 상태 확인

**파일:** `services/workspace_runtime_manager.rs`
**문제:** `create_docker_exec_command()`에서 매 호출마다 `ensure_runtime()` → `healthcheck()` → `docker --version`, `docker info`, `docker inspect` 실행
**영향:** 빈번한 shell 명령 시 Docker API 호출이 반복됨
**해결 방향:** 컨테이너 상태 캐싱 (컨테이너가 살아있는 한 재검사 생략)

### 5.8 [P2-Medium] 세션 삭제 시 컨테이너 데이터 미정리

**파일:** `services/workspace_runtime_manager.rs`
**문제:** `remove_runtime_for_session()`에서 `docker rm -f -v` 호출
**영향:**

- `-v` 플래그는 named volume만 제거 → anonymous volume은 제거 안 됨
- 컨테이너 내부에 생성된 파일이 호스트 마운트에 남아있을 수 있음
  **해결 방향:** 컨테이너 삭제 전 `/workspace` 내부 불필요 파일 정리

### 5.9 [P2-Medium] Docker 컨테이너 생성 — Race Condition

**파일:** `services/workspace_runtime_manager.rs`, `lifecycle/creation.rs`
**문제:**

```rust
// creation.rs
if session.workspace_isolation == WorkspaceIsolationMode::Docker {
    if let Err(error) = WorkspaceRuntimeManager::ensure_runtime(&session).await {
        // ...
    }
}
```

- 여러 세션이 동시에 생성될 때, `ensure_runtime()` 내부의 check-then-act 패턴으로 인해 같은 컨테이너 이름으로 중복 생성 시도 가능
  **해결 방향:** Docker 컨테이너 이름 기반으로 external lock 추가

### 5.10 [P2-Medium] 컨테이너 재시작 시孤儿 컨테이너

**파일:** `services/workspace_runtime_manager.rs`
**문제:**

- `sweep_stale_containers()`는 명시적 호출 시에만 실행
- Rust 프로세스 재시작 시孤儿 컨테이너가 호스트에 남음
  **해결 방향:** 앱 시작 시 `sweep_stale_containers()` 자동 호출

### 5.11 [P3-Low] Docker 이미지 — 필수 도구 미검증

**파일:** `workspace_runtime_manager.rs`
**문제:** `ensure_supported_shell()`에서 `bash`/`sh`만 검증. Node.js, Python, pnpm 등은 검증 안 함
**영향:** 사용자가 커스텀 Docker 이미지를 사용할 때 필요한 도구가 없을 수 있음
**해결 방향:** 선택적 도구 검증 옵션 추가

### 5.12 [P3-Low] validate_windows_shell_syntax — Docker 모드에서 누락

**파일:** `handlers.rs`
**문제:** Windows 전용 `&&` 구문 검사가 Windows 호스트에서만 호출됨
**영향:** Docker 컨테이너 내부에서는 Linux bash가 실행되므로, Windows 전용 구문 검사가 필요 없음 → 정상
**해결 방향:** 현재 설계대로 유지 (Docker 모드에서는 Linux bash 실행이므로 Windows 구문 검증 불필요)

### 5.13 [P3-Low] Docker 포트 바인딩 — 호스트 포트 충돌 검증

**파일:** `workspace_runtime_manager.rs`
**문제:** `ensure_host_port_available()`에서 포트가 사용 중인지 확인
**영향:** 정상적으로 동작하나, 동시 세션 생성 시 포트 충돌 가능
**해결 방향:** 포트 동적 할당 옵션 추가

### 5.14 [P3-Low] Volume Mount 성능 — macOS

**파일:** `docker/Dockerfile`
**문제:** macOS에서 Docker Desktop은 VM을 사용 → 호스트 ↔ 컨테이너 파일 I/O가 느림
**영향:** 대용량 파일 읽기/쓰기 시 성능 저하
**해결 방향:** AI 문서화 — "macOS에서 대용량 파일 작업 시 성능이 저하될 수 있음" 안내

---

## 6. 정리

### 정상 동작하는 부분

- Docker 컨테이너 생성/삭제/재시작 라이프사이클
- Persistent shell을 통한 컨테이너 내부 상태 유지 (cd, export 등)
- 파일 도구 경로 매핑 (Linux/macOS 호스트)
- Docker 컨테이너 소유권 검증 (label 기반)
- 컨테이너 내부 CWD 표시 (`display_shell_cwd`)

### 개선 필요 부분 (Priority 순)

1. **P0:** Windows 호스트에서 `PathMappingLayer` 실패 → `container_to_host()` 경로 구분자 불일치
2. **P0:** `runShell`/`spawnProcess` 응답에 CWD 미포함 → AI가 실행 디렉토리 인식 불가
3. **P1:** 컨텍스트 프롬프트에서 호스트 경로와 컨테이너 CWD 혼동
4. **P1:** 호스트↔컨테이너 파일시스템 분리 → 설치된 패키지가 보이지 않는 문제
5. **P1:** `ensure_runtime()` 매 호출 시 Docker API 반복 호출 → 캐싱 필요
6. **P2:** Docker exec 프로세스 생성 오버헤드
7. **P2:** 세션 삭제 시 컨테이너 데이터 미정리
8. **P2:** Race Condition — 동시 세션 생성 시 컨테이너 중복 생성 가능
9. **P2:** 앱 재시작 시 孤儿 컨테이너 잔존
