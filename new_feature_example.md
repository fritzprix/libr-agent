네, 제시해주신 내장 MCP 서버 구현 계획은 매우 구체적이고 훌륭합니다. Tauri V2와 Rust 환경에서 각 기능을 구현하는 데 도움이 될 만한 라이브러리(Crate)들을 추천해 드리겠습니다.

### 🎯 **핵심 기능별 추천 라이브러리**

계획서의 핵심인 `Filesystem Server`와 `Sandbox Server`를 중심으로 라이브러리를 정리했습니다.

---

### 📂 Filesystem Server (`builtin:filesystem`)

파일 시스템 조작은 Rust 표준 라이브러리와 `tokio`만으로도 대부분 구현할 수 있습니다. 보안에 특히 신경 써야 합니다.

1.  **비동기 파일 처리: `tokio`**
    - Tauri는 `tokio` 런타임을 사용하므로, 파일 I/O 작업 시 UI가 멈추지 않도록 비동기 처리를 하는 것이 필수적입니다. `tokio::fs`를 사용하세요.
    - **사용**: `tokio::fs::read_to_string`, `tokio::fs::write`, `tokio::fs::read_dir` 등

2.  **경로 보안 및 정규화: `path-clean`**
    - `../` 같은 경로 조작으로 상위 디렉터리에 접근하는 **Directory Traversal 공격**을 막기 위해 경로 정규화가 매우 중요합니다. `path-clean`은 이를 간단하게 처리해 줍니다.
    - **사용법**:

      ```rust
      use path_clean::PathClean;

      // 사용자 입력 경로
      let user_path = "some/dir/../../secret.txt";

      // 정규화된 경로
      let clean_path = std::path::PathBuf::from(user_path).clean();
      // -> 결과: "secret.txt"
      ```

    - **보안 로직**: `path-clean`으로 경로를 정리한 후, 반드시 **실행 위치의 하위 디렉터리인지** 검증하는 로직을 추가해야 합니다.

---

### 🔒 Sandbox Server (`builtin:sandbox`)

코드 실행 샌드박스는 가장 복잡하고 보안이 중요한 부분입니다. 여러 접근 방식을 고려할 수 있습니다.

#### **1. 기본 프로세스 실행: `tokio::process::Command`**

가장 기본이 되는 라이브러리입니다. `python`, `ts-node` 등 외부 명령어를 실행할 때 사용합니다.

- **주요 기능**:
  - 프로세스 생성 및 실행: `Command::new("python").arg("-c").arg(code)`
  - 표준 입출력/에러 리다이렉션
  - **환경 변수 격리**: `.env_clear()` 메서드로 부모 프로세스의 환경 변수 상속을 막을 수 있습니다.

#### **2. 보안 샌드박스 구축**

단순히 `Command`만 사용하는 것은 보안에 취약합니다. 아래 라이브러리들을 조합하여 강력한 샌드박스를 구축해야 합니다.

- **임시 디렉토리/파일: `tempfile`**
  - 코드를 실행할 격리된 임시 디렉터리를 만들 때 매우 유용합니다. 사용이 끝나면 자동으로 삭제되어 편리합니다.
  - **사용법**:

    ```rust
    use tempfile::tempdir;

    // 임시 디렉토리 생성
    let dir = tempdir()?;
    // 코드 파일을 이 디렉토리 안에 생성
    let file_path = dir.path().join("script.py");
    // ... 코드 실행 후 dir이 scope를 벗어나면 자동 삭제
    ```

- **실행 시간 제한: `tokio::time::timeout`**
  - `tokio`의 기본 기능으로, 지정된 시간 안에 프로세스가 끝나지 않으면 강제로 종료시킬 수 있습니다.
  - **사용법**:

    ```rust
    use std::time::Duration;
    use tokio::time::timeout;

    let child_process = Command::new(...).spawn()?;
    let timeout_duration = Duration::from_secs(30);

    // 30초 타임아웃 설정
    if let Err(_) = timeout(timeout_duration, child_process.wait()).await {
        // 타임아웃 발생! 자식 프로세스 강제 종료
        child_process.kill().await?;
    }
    ```

- **시스템 콜 제한 (Linux): `seccompiler`**
  - **최고 수준의 보안**을 원한다면 고려해볼 수 있습니다. 실행할 코드가 파일 열기, 네트워크 접속 등 특정 시스템 콜(OS 기능 호출)을 사용하지 못하도록 원천 차단합니다.
  - Linux 환경에서만 동작하며, `seccomp-bpf`를 사용합니다.
  - **단점**: 설정이 복잡하고, 어떤 시스템 콜을 허용/차단할지 정책을 세우기 어렵습니다. 크로스플랫폼 지원이 안 됩니다.

- **WebAssembly (WASM) 런타임: `wasmer` 또는 `wasmtime`**
  - **가장 강력하고 안전한 크로스플랫폼 샌드박스** 방식입니다. 코드를 네이티브로 실행하는 대신 WASM 런타임 안에서 실행합니다.
  - **장점**:
    - 기본적으로 파일 시스템, 네트워크 접근이 모두 차단됩니다.
    - WASI(WebAssembly System Interface)를 통해 특정 디렉토리나 기능만 선택적으로 허용할 수 있습니다.
    - 메모리, CPU 사용량 제한이 용이합니다.
  - **단점**: Python(`RustPython` 등)이나 TypeScript 인터프리터를 WASM으로 컴파일하거나 찾아야 하므로 초기 구현 복잡도가 매우 높습니다. 하지만 장기적으로 가장 이상적인 모델입니다.

---

### ✨ **기타 유용한 라이브러리**

개발 전반에 도움이 되는 라이브러리들입니다.

- **JSON 처리: `serde` & `serde_json`**
  - Tauri 커맨드의 인자(`args: Value`)를 Rust 구조체로 변환하거나, 결과를 JSON으로 만들 때 필수적입니다. 아마 이미 사용하고 계실 겁니다.

- **에러 처리: `thiserror` & `anyhow`**
  - MCP 프로토콜 표준 에러를 정의하고 관리할 때 `thiserror`가 유용합니다.
  - Tauri 커맨드 내에서 다양한 종류의 에러를 통합하여 `Result<_, String>` 형태로 반환할 때 `anyhow`가 편리합니다.

- **로깅: `tracing`**
  - 내장 서버의 동작, 에러, 성능 등을 추적하기 위해 체계적인 로깅 시스템을 구축하는 것이 좋습니다. `tracing`은 비동기 환경에 최적화되어 있습니다.

---

### 🚀 **추천 구현 조합**

1.  **(Phase 1-3) 현실적인 초기 구현:**
    - **Filesystem**: `tokio::fs` + `path-clean`으로 경로 검증 철저히.
    - **Sandbox**: `tokio::process::Command` + `tempfile` (임시 디렉토리) + `tokio::time::timeout` (시간 제한) + `.env_clear()` (환경 변수 격리).
    - 이 조합만으로도 계획서에 명시된 대부분의 보안 요구사항을 만족시킬 수 있습니다.

2.  **(향후 확장) 최고 수준의 보안:**
    - **Sandbox**: `wasmer` 또는 `wasmtime` 같은 WASM 런타임을 도입하여 언어 인터프리터를 직접 실행하는 방식을 고려해볼 수 있습니다. 이는 네트워크 접근 제한 등 고급 보안 기능을 훨씬 쉽게 구현할 수 있게 해줍니다.

제시해주신 계획이 워낙 탄탄하여, 위 라이브러리들을 활용하신다면 성공적으로 내장 MCP 서버를 구현하실 수 있을 것입니다.
