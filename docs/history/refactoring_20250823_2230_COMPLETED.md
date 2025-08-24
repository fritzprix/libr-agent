# Refactoring Plan: SecureFileManager 구조 개선 - COMPLETED ✅

**Status**: ✅ **COMPLETED** (2025-08-23 22:30)  
**Original Plan**: `refactoring_2.md`

## 작업의 목적

파일 읽기/쓰기 등 파일 시스템 관련 API를 MCP 서버와 Tauri 커맨드에서 일관된 방식으로 사용할 수 있도록 구조를 개선한다.  
중복된 파일 접근 로직을 제거하고, 보안 검증 및 에러 처리를 단일 서비스(SecureFileManager)에서 관리하여 유지보수성과 확장성을 높인다.

## 해결된 문제점

- ✅ `lib.rs`에서 파일 읽기/쓰기 커맨드(`read_file`, `write_file`)가 직접 파일 시스템을 다룸
- ✅ `filesystem.rs`의 MCP 서버(`FilesystemServer`)가 별도의 파일 접근 로직을 구현하고 있음
- ✅ 경로 검증(`SecurityValidator`)이 각 위치에서 개별적으로 사용되어, 코드 중복 및 정책 불일치 위험 존재
- ✅ 프론트엔드(`use-rust-backend.ts`)에서 MCP와 커맨드가 서로 다른 방식으로 파일 API를 호출

## 구현된 해결책

### 1. ✅ SecureFileManager 서비스 구조체 추가

**파일**: `src-tauri/src/services/secure_file_manager.rs`

```rust
pub struct SecureFileManager {
    security: SecurityValidator,
}

impl SecureFileManager {
    pub fn new() -> Self {
        Self {
            security: SecurityValidator::new(),
        }
    }

    pub async fn read_file(&self, path: &str) -> Result<Vec<u8>, String> { /* ... */ }
    pub async fn write_file(&self, path: &str, content: &[u8]) -> Result<(), String> { /* ... */ }
    pub async fn read_file_as_string(&self, path: &str) -> Result<String, String> { /* ... */ }
    pub async fn write_file_string(&self, path: &str, content: &str) -> Result<(), String> { /* ... */ }
}
```

### 2. ✅ AppState로 등록 및 커맨드에서 사용

**파일**: `src-tauri/src/lib.rs`

```rust
#[tauri::command]
async fn read_file(file_path: String, manager: tauri::State<'_, SecureFileManager>) -> Result<Vec<u8>, String> {
    manager.read_file(&file_path).await
}

#[tauri::command]
async fn write_file(file_path: String, content: Vec<u8>, manager: tauri::State<'_, SecureFileManager>) -> Result<(), String> {
    manager.write_file(&file_path, &content).await
}

// Tauri 앱 초기화 시 등록
fn run() {
    tauri::Builder::default()
        .manage(SecureFileManager::new())
        // ...
}
```

### 3. ✅ MCP 서버에서 SecureFileManager 사용

**파일**: `src-tauri/src/mcp/builtin/filesystem.rs`

```rust
pub struct FilesystemServer {
    file_manager: std::sync::Arc<SecureFileManager>,
}

impl FilesystemServer {
    pub fn new(file_manager: std::sync::Arc<SecureFileManager>) -> Self {
        Self { file_manager }
    }

    async fn handle_read_file(&self, args: Value) -> MCPResponse {
        // ...
        self.file_manager.read_file_as_string(path_str).await
    }

    async fn handle_write_file(&self, args: Value) -> MCPResponse {
        // ...
        self.file_manager.write_file_string(path_str, content).await
    }
}
```

### 4. ✅ 프론트엔드에서 파일 API 호출 방식 통일

**파일**: `src/hooks/use-rust-backend.ts`

```typescript
export const useRustBackend = () => {
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

## 달성된 결과

### ✅ **통합 파일 관리**
- 파일 시스템 접근 로직이 `SecureFileManager`라는 단일 서비스로 통합
- MCP 서버와 Tauri 커맨드 모두 동일한 인스턴스 사용

### ✅ **일관된 보안 정책**
- `SecureFileManager`가 Tauri AppState로 등록되어, 모든 파일 관련 커맨드와 MCP 도구에서 안전하게 접근
- 경로 검증, 파일 크기 제한, 에러 처리 등 모든 보안 정책이 일관되게 적용

### ✅ **코드 중복 제거**
- 중복된 파일 접근 로직 제거
- 테스트 및 유지보수 용이성 향상

### ✅ **통일된 API**
- 프론트엔드에서 MCP와 커맨드 모두 동일한 파일 API 호출 방식 사용
- TypeScript 타입 안전성 유지

## 품질 검증 결과

- ✅ **코드 포매팅**: `cargo fmt` 통과
- ✅ **린팅**: `cargo clippy`, `pnpm lint` 통과  
- ✅ **빌드**: `pnpm build`, `cargo check` 성공
- ✅ **타입 안전성**: TypeScript 컴파일 오류 없음

## 실행된 파일 변경사항

1. **새로 생성된 파일**:
   - `src-tauri/src/services/secure_file_manager.rs`

2. **수정된 파일**:
   - `src-tauri/src/services/mod.rs` - SecureFileManager export 추가
   - `src-tauri/src/lib.rs` - AppState 등록 및 커맨드 업데이트
   - `src-tauri/src/mcp/builtin/filesystem.rs` - SecureFileManager 사용으로 전환
   - `src-tauri/src/mcp/builtin/mod.rs` - 공유 SecureFileManager 인스턴스 생성
   - `src/hooks/use-rust-backend.ts` - writeFile 함수 추가

**완료 일시**: 2025-08-23 22:15  
**검증자**: Claude Code Assistant  
**다음 작업**: browser_getPageContent 개선 (refactoring.md 기반)