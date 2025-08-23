# Refactoring Plan: 파일 시스템 API 구조 개선 - ✅ COMPLETED

**Status**: ✅ **COMPLETED** (2025-08-23 22:15)  
**Completion Document**: `refactoring_20250823_2230_COMPLETED.md`

## 작업의 목적

파일 읽기/쓰기 등 파일 시스템 관련 API를 MCP 서버와 Tauri 커맨드에서 일관된 방식으로 사용할 수 있도록 구조를 개선한다.  
중복된 파일 접근 로직을 제거하고, 보안 검증 및 에러 처리를 단일 서비스(SecureFileManager)에서 관리하여 유지보수성과 확장성을 높인다.

## 현재의 상태 / 문제점

- `lib.rs`에서 파일 읽기/쓰기 커맨드(`read_file`, `write_file`)가 직접 파일 시스템을 다룸.
- `filesystem.rs`의 MCP 서버(`FilesystemServer`)도 별도의 파일 접근 로직을 구현하고 있음.
- 경로 검증(`SecurityValidator`)은 각 위치에서 개별적으로 사용되고 있어, 코드 중복 및 정책 불일치 위험이 존재.
- 프론트엔드(`use-rust-backend.ts`)에서는 MCP와 커맨드가 서로 다른 방식으로 파일 API를 호출함.

## 변경 이후의 상태 / 해결 판정 기준

- 파일 시스템 접근 로직을 `SecureFileManager`라는 단일 서비스로 분리하여, MCP 서버와 Tauri 커맨드 모두 동일한 인스턴스를 사용.
- `SecureFileManager`는 Tauri AppState로 등록되어, 모든 파일 관련 커맨드와 MCP 도구에서 안전하게 접근 가능.
- 경로 검증, 파일 크기 제한, 에러 처리 등 모든 보안 정책이 일관되게 적용됨.
- 프론트엔드에서는 MCP와 커맨드 모두 동일한 파일 API를 호출할 수 있음.
- 코드 중복이 제거되고, 테스트 및 유지보수가 용이해짐.

## 수정이 필요한 코드 및 수정부분의 코드 스니핏

### 1. SecureFileManager 서비스 구조체 추가

```rust
// src-tauri/src/services/secure_file_manager.rs
use crate::mcp::builtin::utils::SecurityValidator;

pub struct SecureFileManager {
  security: SecurityValidator,
}

impl SecureFileManager {
  pub fn new() -> Self {
    Self {
      security: SecurityValidator::new(),
    }
  }

  pub async fn read_file(&self, path: &str) -> Result<Vec<u8>, String> {
    let safe_path = self.security.validate_path(path)
      .map_err(|e| format!("Security error: {}", e))?;
    // ...파일 존재 및 타입 체크, 파일 크기 제한 등...
    tokio::fs::read(&safe_path).await.map_err(|e| format!("Failed to read file: {}", e))
  }

  pub async fn write_file(&self, path: &str, content: &[u8]) -> Result<(), String> {
    let safe_path = self.security.validate_path(path)
      .map_err(|e| format!("Security error: {}", e))?;
    // ...디렉터리 생성, 파일 크기 제한 등...
    tokio::fs::write(&safe_path, content).await.map_err(|e| format!("Failed to write file: {}", e))
  }
}
```

### 2. AppState로 등록 및 커맨드에서 사용

```rust
// src-tauri/src/lib.rs
use tauri::State;
use services::SecureFileManager;

#[tauri::command]
async fn read_file(file_path: String, manager: State<'_, SecureFileManager>) -> Result<Vec<u8>, String> {
  manager.read_file(&file_path).await
}

#[tauri::command]
async fn write_file(file_path: String, content: Vec<u8>, manager: State<'_, SecureFileManager>) -> Result<(), String> {
  manager.write_file(&file_path, &content).await
}

// Tauri 앱 초기화 시 등록
fn run() {
  tauri::Builder::default()
    .manage(SecureFileManager::new())
    // ...기존 설정...
}
```

### 3. MCP 서버에서 SecureFileManager 사용

```rust
// src-tauri/src/mcp/builtin/filesystem.rs
use tauri::AppHandle;
use services::SecureFileManager;

pub struct FilesystemServer {
  file_manager: std::sync::Arc<SecureFileManager>,
}

impl FilesystemServer {
  pub fn new(app_handle: &AppHandle) -> Self {
    let file_manager = app_handle.state::<SecureFileManager>().clone();
    Self { file_manager }
  }

  async fn handle_read_file(&self, args: Value) -> MCPResponse {
    // ...args 파싱...
    match self.file_manager.read_file(path_str).await {
      Ok(content) => { /* MCPResponse 성공 반환 */ }
      Err(e) => { /* MCPResponse 에러 반환 */ }
    }
  }

  async fn handle_write_file(&self, args: Value) -> MCPResponse {
    // ...args 파싱...
    match self.file_manager.write_file(path_str, content.as_bytes()).await {
      Ok(_) => { /* MCPResponse 성공 반환 */ }
      Err(e) => { /* MCPResponse 에러 반환 */ }
    }
  }
}
```

### 4. 프론트엔드에서 파일 API 호출 방식 통일

```typescript
// src/hooks/use-rust-backend.ts
export const useRustBackend = () => {
  // ...
  const readFile = async (filePath: string): Promise<number[]> => {
    return safeInvoke<number[]>('read_file', { filePath });
  };

  const writeFile = async (filePath: string, content: number[]): Promise<void> => {
    return safeInvoke<void>('write_file', { filePath, content });
  };

  return {
    // ...
    readFile,
    writeFile,
    // ...
  };
};
```

## file 규칙

- 본 계획 및 결과 문서는 `./docs/history/refactoring_{yyyyMMdd_hhmm}.md` 형식으로 저장한다.