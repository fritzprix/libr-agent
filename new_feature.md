# Built-in MCP Server Implementation Plan

## 🎯 목표

SynapticFlow에 **내장형 MCP 서버**를 구현하여 사용자가 별도의 외부 프로세스 설치 없이도 핵심 기능들을 MCP 프로토콜로 사용할 수 있도록 한다.

### 기존 문제점

- 외부 MCP 서버 추가는 일반 사용자에게 어려움
- NPM, Python, Docker 등 다양한 의존성 설치 필요
- 복잡한 환경 설정으로 인한 진입 장벽

### 해결 방안

- Rust 백엔드에 MCP 프로토콜과 동일한 인터페이스를 가진 내장 서버 구현
- 별도 설치 없이 즉시 사용 가능한 핵심 도구들 제공
- 시스템 리소스를 최대한 활용할 수 있는 네이티브 성능

## 🏗️ 아키텍처 설계

### 1. Built-in MCP Server Trait

```rust
pub trait BuiltinMCPServer: Send + Sync {
  fn name(&self) -> &str;
  fn description(&self) -> &str;
  fn version(&self) -> &str { "1.0.0" }
  fn tools(&self) -> Vec<MCPTool>;
  fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse;
}
```

### 2. 구현할 내장 서버들

#### 2.1 Filesystem Server (`builtin.filesystem`)

- **목적**: 파일 시스템 조작
- **주요 도구들**:
  - `read_file`: 파일 내용 읽기
  - `write_file`: 파일 내용 쓰기
  - `list_directory`: 디렉토리 목록 조회
  - `create_directory`: 디렉토리 생성 (향후 확장)
  - `delete_file`: 파일 삭제 (향후 확장)
- **접근 제약**: 현재 process의 실행 위치의 하위 디렉토리까지로 범위를 제한

#### 2.2 Sandbox Server (`builtin.sandbox`)

- **목적**: 코드 실행 (Python/TypeScript)
- **주요 도구들**:
  - `execute_python`: Python 코드 실행
  - `execute_typescript`: TypeScript 코드 실행 (ts-node 사용)
- **보안 기능**:
  - 임시 디렉토리에서 실행
  - 실행 시간 제한 (최대 30초)
  - 환경 변수 격리
  - 코드 크기 제한 (10KB)

### 3. 기존 시스템과의 통합

#### 3.1 MCPServerManager 확장

```rust
pub struct MCPServerManager {
  connections: Arc<Mutex<HashMap<String, MCPConnection>>>,  // 기존 외부 서버
  builtin.servers: HashMap<String, Box<dyn BuiltinMCPServer>>,  // 새로운 내장 서버
}
```

#### 3.2 통합 API 제공

- `list_all_tools_unified()`: 외부 + 내장 서버의 모든 도구 목록
- `call_builtin.tool()`: 내장 서버 도구 호출
- `list_builtin.servers()`: 사용 가능한 내장 서버 목록

## 📁 파일 구조

```
src-tauri/src/
├── mcp/
│   ├── mod.rs              # 기존 MCPServerManager + 통합 로직
│   ├── builtin/
│   │   ├── mod.rs          # BuiltinMCPServer trait 정의
│   │   ├── filesystem.rs   # 파일시스템 서버 구현
│   │   ├── sandbox.rs      # 샌드박스 서버 구현
│   │   └── utils.rs        # 공통 유틸리티
│   └── external.rs         # 기존 외부 MCP 서버 로직
└── lib.rs                  # 새로운 Tauri commands 추가
```

## 🔧 구현 세부사항

### 1. 보안 고려사항

#### Filesystem Server

- 경로 검증으로 directory traversal 공격 방지
- 허용된 디렉토리 외부 접근 제한
- 파일 크기 제한으로 디스크 공간 보호

#### Sandbox Server

- 임시 디렉토리에서 코드 실행
- 환경 변수 격리 (`PYTHONPATH`, `NODE_PATH` 초기화)
- 실행 시간 제한 (1-30초)
- 코드 크기 제한 (10KB)
- 네트워크 접근 제한 (향후 추가)

### 2. 에러 처리

- MCP 프로토콜 표준 에러 코드 사용
- 상세한 에러 메시지 제공
- 타임아웃 및 리소스 제한 처리

### 3. 성능 최적화

- Rust의 비동기 처리 활용
- 메모리 효율적인 스트림 처리
- 임시 파일 자동 정리

## 🚀 구현 단계

### Phase 1: 기본 구조 구축

1. `BuiltinMCPServer` trait 정의
2. `MCPServerManager`에 내장 서버 통합 로직 추가
3. 기본 Tauri commands 구현

### Phase 2: Filesystem Server 구현

1. 기본 파일 읽기/쓰기 기능
2. 디렉토리 목록 조회
3. 보안 검증 로직 추가

### Phase 3: Sandbox Server 구현

1. Python 코드 실행 기능
2. TypeScript 코드 실행 기능 (ts-node)
3. 보안 및 제한 사항 적용

### Phase 4: 프론트엔드 통합

1. 내장 서버 도구들을 UI에 표시
2. 기존 MCP 도구와 동일한 방식으로 호출
3. 에러 처리 및 사용자 피드백

## 📋 새로운 Tauri Commands

```rust
// 내장 서버 관련
#[tauri::command] async fn list_builtin.tools() -> Vec<MCPTool>
#[tauri::command] async fn call_builtin.tool(server_name: String, tool_name: String, args: Value) -> MCPResponse
#[tauri::command] async fn list_builtin.servers() -> Vec<String>

// 통합 API
#[tauri::command] async fn list_all_tools_unified() -> Result<Vec<MCPTool>, String>
```

## 💡 사용 예시

### 프론트엔드에서의 사용법

```typescript
// 모든 도구 목록 가져오기 (외부 + 내장)
const allTools = await invoke('list_all_tools_unified');

// 파일 읽기
const fileContent = await invoke('call_builtin.tool', {
  serverName: 'builtin.filesystem',
  toolName: 'read_file',
  args: { path: '/path/to/file.txt' },
});

// Python 코드 실행
const pythonResult = await invoke('call_builtin.tool', {
  serverName: 'builtin.sandbox',
  toolName: 'execute_python',
  args: {
    code: 'print("Hello from Python!")',
    timeout: 5,
  },
});

// TypeScript 코드 실행
const tsResult = await invoke('call_builtin.tool', {
  serverName: 'builtin.sandbox',
  toolName: 'execute_typescript',
  args: {
    code: 'console.log("Hello from TypeScript!");',
    timeout: 10,
  },
});
```

## 🔄 향후 확장 계획

### 추가 내장 서버 아이디어

1. **HTTP Client Server**: REST API 호출 기능
2. **Database Server**: SQLite 등 경량 DB 조작
3. **Image Processing Server**: 기본적인 이미지 처리
4. **Text Processing Server**: 정규식, 텍스트 변환 등
5. **System Info Server**: 시스템 정보 조회

### 고급 기능

1. **권한 관리**: 사용자별 도구 접근 제한
2. **사용량 모니터링**: 도구 사용 통계 및 제한
3. **플러그인 시스템**: 사용자 정의 내장 서버 추가
4. **캐싱**: 자주 사용되는 결과 캐시

## 🎯 기대 효과

1. **사용자 편의성 향상**: 복잡한 설치 과정 없이 즉시 사용 가능
2. **성능 향상**: 네이티브 Rust 성능으로 빠른 실행
3. **보안 강화**: 샌드박스 환경에서 안전한 코드 실행
4. **확장성**: 새로운 내장 서버를 쉽게 추가할 수 있는 구조
5. **일관성**: 외부 MCP와 동일한 인터페이스로 학습 비용 최소화

---

## 📝 구현 체크리스트

### Phase 1: 기본 구조

- [ ] `src-tauri/src/mcp/builtin/mod.rs` - BuiltinMCPServer trait 정의
- [ ] `src-tauri/src/mcp/mod.rs` - MCPServerManager에 내장 서버 통합
- [ ] `src-tauri/src/lib.rs` - 새로운 Tauri commands 추가

### Phase 2: Filesystem Server

- [ ] `src-tauri/src/mcp/builtin/filesystem.rs` - FilesystemServer 구현
- [ ] 파일 읽기/쓰기 기능 구현
- [ ] 디렉토리 목록 조회 기능 구현
- [ ] 보안 검증 로직 추가

### Phase 3: Sandbox Server

- [ ] `src-tauri/src/mcp/builtin/sandbox.rs` - SandboxServer 구현
- [ ] Python 코드 실행 기능 구현
- [ ] TypeScript 코드 실행 기능 구현
- [ ] 보안 제한 사항 적용

### Phase 4: 테스트 및 통합

- [ ] 단위 테스트 작성
- [ ] 프론트엔드 통합 테스트
- [ ] 문서 업데이트
- [ ] 성능 테스트 및 최적화
