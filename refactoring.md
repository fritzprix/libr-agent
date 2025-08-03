# 📚 SynapticFlow 문서화 전략 및 계획

## 🎯 문서화 목표

1. **개발자 온보딩 시간 단축**: 새로운 개발자가 30분 내에 개발 환경 구축 가능
2. **API 참조 표준화**: 모든 Tauri 커맨드에 대한 일관된 문서 제공
3. **확장성 보장**: 새로운 MCP 도구나 기능 추가 시 문서 업데이트 프로세스 체계화
4. **사용자 경험 향상**: 트러블슈팅 및 에러 해결을 위한 실용적 가이드 제공

## 📁 제안된 문서 구조

```
docs/
├── README.md                     # 프로젝트 개요 + 빠른 시작 가이드
├── api/
│   ├── tauri-commands.md         # 📍 메인 API 참조 문서 (통합형)
│   ├── types.md                  # 공통 타입 정의 (MCPTool, MCPServerConfig 등)
│   └── examples.md               # 실제 사용 시나리오 및 코드 예시
├── guides/
│   ├── getting-started.md        # 환경 설정 + 첫 MCP 서버 연결
│   ├── mcp-integration.md        # MCP 서버 개발/연동 가이드
│   ├── tool-development.md       # 커스텀 도구 개발 가이드
│   └── troubleshooting.md        # 자주 발생하는 문제 해결
├── architecture/
│   ├── overview.md               # 전체 시스템 아키텍처
│   ├── data-flow.md              # React ↔ Tauri ↔ MCP 데이터 흐름
│   ├── security.md               # 보안 고려사항 및 API 키 관리
│   └── error-handling.md         # 에러 처리 전략 및 복구 메커니즘
└── contributing/
    ├── coding-standards.md       # 코딩 컨벤션 및 스타일 가이드
    ├── testing.md               # 테스트 전략 및 가이드라인
    └── release-process.md        # 릴리스 프로세스 및 버전 관리
```

## 🔗 코드베이스와 문서 연결 매핑

### 1. **API 참조 문서 (`docs/api/tauri-commands.md`)**

**📍 소스 코드 위치**: [`src-tauri/src/lib.rs`](src-tauri/src/lib.rs) (줄 15-185)

#### 문서화 대상 Tauri 커맨드들:

| Command                    | 코드 위치      | 문서 섹션           | 우선순위  |
| -------------------------- | -------------- | ------------------- | --------- |
| `start_mcp_server`         | lib.rs:20-25   | Server Management   | 🔴 High   |
| `stop_mcp_server`          | lib.rs:27-32   | Server Management   | 🔴 High   |
| `call_mcp_tool`            | lib.rs:34-41   | Tool Operations     | 🔴 High   |
| `list_mcp_tools`           | lib.rs:43-48   | Tool Operations     | 🔴 High   |
| `list_tools_from_config`   | lib.rs:50-142  | Tool Operations     | 🟡 Medium |
| `get_connected_servers`    | lib.rs:144-146 | Status & Monitoring | 🟡 Medium |
| `check_server_status`      | lib.rs:148-150 | Status & Monitoring | 🟡 Medium |
| `check_all_servers_status` | lib.rs:152-154 | Status & Monitoring | 🟡 Medium |
| `list_all_tools`           | lib.rs:156-161 | Utility Functions   | 🟢 Low    |
| `get_validated_tools`      | lib.rs:163-168 | Utility Functions   | 🟢 Low    |
| `validate_tool_schema`     | lib.rs:170-173 | Utility Functions   | 🟢 Low    |

#### 각 커맨드별 문서 템플릿:

````markdown
### start_mcp_server

**Purpose**: MCP 서버를 시작하고 도구 목록을 초기화합니다.

**Source**: [`src-tauri/src/lib.rs:20-25`](../src-tauri/src/lib.rs#L20-L25)

**Parameters**:

- `config: MCPServerConfig` - 서버 설정 객체 ([타입 정의](./types.md#mcpserverconfig))

**Returns**:

- `Result<String, String>` - 성공 시 서버 이름, 실패 시 에러 메시지

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/tauri';

const config = {
  name: 'filesystem',
  command: 'npx',
  args: ['-y', '@modelcontextprotocol/server-filesystem', '/tmp'],
};

try {
  const serverName = await invoke<string>('start_mcp_server', { config });
  console.log(`Server started: ${serverName}`);
} catch (error) {
  console.error('Failed to start server:', error);
}
```
````

**Error Cases**:

- 서버 실행 파일을 찾을 수 없는 경우
- 포트가 이미 사용 중인 경우
- 잘못된 설정으로 인한 초기화 실패

**Related**:

- [stop_mcp_server](#stop_mcp_server)
- [check_server_status](#check_server_status)
- [MCPServerConfig 타입](./types.md#mcpserverconfig)

```

### 2. **타입 정의 문서 (`docs/api/types.md`)**

**📍 소스 코드 위치**:
- [`src-tauri/src/mcp.rs`](src-tauri/src/mcp.rs) (MCP 관련 타입들)
- [`src/models/`](src/models/) (React 프론트엔드 타입들)

#### 문서화 대상 주요 타입들:

| 타입 | 소스 위치 | 설명 | 사용 빈도 |
|------|-----------|------|----------|
| `MCPServerConfig` | mcp.rs:~50-70 | MCP 서버 설정 | 🔴 매우 높음 |
| `MCPTool` | mcp.rs:~80-100 | MCP 도구 정의 | 🔴 매우 높음 |
| `ToolCallResult` | mcp.rs:~120-140 | 도구 호출 결과 | 🔴 매우 높음 |
| `JSONSchema` | mcp.rs:~160-180 | JSON 스키마 정의 | 🟡 높음 |
| `Message` | src/models/chat.ts | 채팅 메시지 | 🟡 높음 |

### 3. **아키텍처 문서 (`docs/architecture/overview.md`)**

**📍 소스 코드 위치**: 전체 프로젝트 구조

#### 문서화 대상 주요 컴포넌트:

| 컴포넌트 | 소스 위치 | 책임 | 문서 섹션 |
|----------|-----------|------|----------|
| Tauri Backend | `src-tauri/src/` | MCP 서버 관리, 도구 실행 | Backend Layer |
| React Frontend | `src/` | UI, 사용자 상호작용 | Frontend Layer |
| ToolCaller | `src/features/chat/orchestrators/ToolCaller.tsx` | 도구 호출 오케스트레이션 | Integration Layer |
| MCP Client | `src/lib/tauri-mcp-client.ts` | MCP 프로토콜 구현 | Communication Layer |
| Context Providers | `src/context/` | 상태 관리 | State Management |

### 4. **트러블슈팅 가이드 (`docs/guides/troubleshooting.md`)**

**📍 에러 발생 위치 매핑**:

| 에러 유형 | 발생 위치 | 문서 섹션 | 해결 방법 |
|-----------|-----------|-----------|----------|
| WebKit 크래시 | `src-tauri/src/lib.rs:188-250` | Linux 환경 문제 | 환경 변수 설정, 패키지 설치 |
| MCP 서버 연결 실패 | `src-tauri/src/mcp.rs:200-250` | 서버 관리 문제 | 설정 검증, 포트 확인 |
| 도구 호출 실패 | `src/features/chat/orchestrators/ToolCaller.tsx:25-45` | 도구 실행 문제 | 스키마 검증, 에러 처리 |
| 타입 불일치 | `src/lib/tauri-mcp-client.ts` | 타입 시스템 문제 | 타입 정의 통합 |

## 📝 문서 작성 우선순위 및 계획

### Phase 1: 핵심 API 문서 (🔴 긴급, 1-2일)

1. **`docs/api/tauri-commands.md` 작성**
   - 모든 11개 Tauri 커맨드에 대한 통합 문서
   - 실제 사용 예시와 에러 처리 포함
   - 코드 위치 링크 및 관련 타입 참조

2. **`docs/api/types.md` 작성**
   - 핵심 타입 5개에 대한 상세 정의
   - JSON 스키마 예시 및 검증 규칙
   - TypeScript/Rust 간 타입 매핑

### Phase 2: 사용자 가이드 (🟡 중요, 3-5일)

3. **`docs/guides/getting-started.md` 작성**
   - 환경 설정부터 첫 MCP 서버 연결까지
   - 실제 동작하는 단계별 튜토리얼
   - 자주 발생하는 초기 설정 문제 해결

4. **`docs/guides/troubleshooting.md` 작성**
   - 현재 refactoring.md에서 식별된 문제들 기반
   - 각 에러의 발생 위치와 해결 방법
   - 실제 에러 메시지와 매칭되는 솔루션

### Phase 3: 아키텍처 및 확장성 (🟢 장기, 1주)

5. **`docs/architecture/overview.md` 작성**
   - 전체 시스템 구조 다이어그램
   - 각 레이어의 책임과 상호작용
   - 데이터 흐름 및 상태 관리

6. **`docs/guides/mcp-integration.md` 작성**
   - 새로운 MCP 서버 개발 가이드
   - 커스텀 도구 추가 방법
   - 스키마 정의 및 검증 규칙

## 🔄 문서 유지보수 전략

### 1. **코드-문서 동기화**

- **자동화 도구**: 코드 변경 시 문서 업데이트 알림
- **리뷰 프로세스**: PR에 문서 업데이트 체크리스트 포함
- **버전 태깅**: 문서 버전과 코드 버전 매핑

### 2. **문서 품질 관리**

- **링크 검증**: 코드 위치 링크 유효성 정기 확인
- **예시 코드 테스트**: 문서의 코드 예시 실제 동작 검증
- **사용자 피드백**: 문서 사용성에 대한 지속적 개선

### 3. **확장성 고려**

- **템플릿 표준화**: 새로운 API 추가 시 일관된 문서 형식
- **자동 생성**: TypeScript 타입에서 문서 자동 생성 도구 고려
- **다국어 지원**: 영어/한국어 병행 문서 작성 계획

## 📊 문서 성공 지표

### 1. **정량적 지표**

- **커버리지**: 전체 11개 Tauri 커맨드 100% 문서화
- **링크 유효성**: 코드 위치 링크 95% 이상 유효
- **업데이트 주기**: 코드 변경 후 3일 이내 문서 업데이트

### 2. **정성적 지표**

- **개발자 온보딩**: 새 개발자가 30분 내 개발 환경 구축 가능
- **문제 해결**: 트러블슈팅 가이드로 80% 이상 문제 자체 해결
- **API 이해도**: 문서만으로 API 사용법 완전 이해 가능

## 🎯 첫 번째 실행 계획

### 즉시 시작 (오늘)

1. **`docs/api/tauri-commands.md` 골격 작성**
   - 11개 커맨드의 기본 템플릿 생성
   - 각 커맨드의 코드 위치 링크 추가
   - 기본적인 파라미터/반환값 정보 입력

2. **실제 코드에서 타입 정보 추출**
   - `src-tauri/src/mcp.rs`에서 주요 타입 정의 분석
   - TypeScript 인터페이스와 Rust 구조체 매핑
   - 불일치하는 타입 정의 식별 및 문서화

### 1일 후

3. **사용 예시 코드 작성 및 검증**
   - 각 커맨드별 실제 동작하는 예시 코드
   - 에러 케이스 및 처리 방법
   - 관련 커맨드 간의 워크플로우 예시

이 계획을 통해 **SynapticFlow의 문서화 수준을 오픈소스 표준에 맞춰 향상**시키고, **개발자 경험과 프로젝트 확장성을 동시에 확보**할 수 있습니다.
```
