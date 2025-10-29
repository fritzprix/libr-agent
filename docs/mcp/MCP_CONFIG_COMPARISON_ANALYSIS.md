# MCP Configuration: Current vs Proposed API Response Format - Comparison Analysis

**작성일**: 2025-10-29  
**목적**: 현재 LibrAgent의 MCPConfig 구조와 제안된 API 응답 스키마 간의 차이점 분석

---

## 📋 목차

1. [현재 구현 분석 (LibrAgent)](#1-현재-구현-분석-libragent)
2. [제안된 API 응답 스키마 분석](#2-제안된-api-응답-스키마-분석)
3. [핵심 차이점 비교](#3-핵심-차이점-비교)
4. [Gap Analysis](#4-gap-analysis)
5. [통합 전략 제안](#5-통합-전략-제안)

---

## 1. 현재 구현 분석 (LibrAgent)

### 1.1 Current MCPConfig Structure

**파일 위치**: `src/models/chat.ts`

```typescript
export interface MCPConfig {
  mcpServers?: Record<
    string,
    {
      command: string;
      args?: string[];
      env?: Record<string, string>;
    }
  >;
}
```

**특징**:

- **Stdio Transport 전용**: `command` + `args` 구조는 stdio 방식만 지원
- **간단한 구조**: 기본적인 실행 정보만 포함
- **로컬 전용**: Remote MCP (OAuth, HTTP/SSE) 지원 없음
- **런타임 설정**: 사용자 활성화 개념 없이 Assistant마다 직접 설정

### 1.2 MCPServerContext 동작 방식

**파일 위치**: `src/context/MCPServerContext.tsx`

```typescript
const connectServers = async (mcpConfig: MCPConfig) => {
  // 1. Rust Backend에 MCPConfig 전달
  const rawToolsByServer = await listToolsFromConfig(mcpConfig);

  // 2. Tool name aliasing: serverName__toolName
  const availableTools: MCPTool[] = Object.entries(rawToolsByServer).flatMap(
    ([s, tools]) => {
      return tools.map((t) => ({
        ...t,
        name: `${toValidJsName(s)}__${t.name}`,
      }));
    },
  );

  // 3. Connected servers 확인
  const connectedServers = await getConnectedServers();
  setServerStatus({ ...serverStatus });
};
```

**동작 흐름**:

1. **Input**: `MCPConfig` (Assistant 설정에서 가져옴)
2. **Rust Backend**: `listToolsFromConfig(mcpConfig)` 호출
3. **Output**: `availableTools[]` (도구 이름은 `prefix__toolname` 형식)
4. **Storage**: Context state only (no persistent activation)

### 1.3 Tool Execution Pattern

```typescript
const executeToolCall = async (toolCall: ToolCall) => {
  // Tool name parsing: "serverAlias__toolName"
  const delimiter = '__';
  const parts = aiProvidedToolName.split(delimiter);
  const alias = parts[0];
  const toolName = parts.slice(1).join(delimiter);

  // Server name lookup
  const serverName = aliasToIdTableRef.current.get(alias);

  // Execute via Rust backend
  const rawResponse = await callMCPTool(serverName, toolName, toolArguments);

  return rawResponse; // MCPResponse format
};
```

### 1.4 Current Data Flow

```text
┌─────────────────────────────────────────────────────────────┐
│                    Frontend (React)                         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Assistant Definition                                       │
│  ┌────────────────────────────────────┐                    │
│  │ interface Assistant {               │                    │
│  │   mcpConfig: MCPConfig {           │                    │
│  │     mcpServers?: {                 │                    │
│  │       "server-name": {             │                    │
│  │         command: "npx"             │                    │
│  │         args: ["-y", "@scope/pkg"] │                    │
│  │         env: { API_KEY: "..." }    │                    │
│  │       }                            │                    │
│  │     }                              │                    │
│  │   }                                │                    │
│  │ }                                  │                    │
│  └────────────────────────────────────┘                    │
│                    ↓                                        │
│  MCPServerContext.connectServers()                         │
│                    ↓                                        │
├─────────────────────────────────────────────────────────────┤
│                Tauri Backend (Rust)                         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  listToolsFromConfig(mcpConfig)                            │
│  ┌────────────────────────────────────┐                    │
│  │ MCPServerManager::spawn()          │                    │
│  │   - stdio process 실행             │                    │
│  │   - MCP protocol handshake         │                    │
│  │   - tools/list 요청                │                    │
│  └────────────────────────────────────┘                    │
│                    ↓                                        │
│  Return: Record<ServerName, MCPTool[]>                     │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**핵심 특징**:

- ✓ **간단한 구조**: Stdio만 지원하면 충분한 경우 효율적
- ✓ **즉시 실행**: Assistant 선택 시 바로 MCP 연결
- ✗ **영구 활성화 없음**: 세션마다 재연결 필요
- ✗ **사용자별 설정 없음**: API 키 등을 Assistant 정의에 하드코딩
- ✗ **OAuth 미지원**: Remote MCP 통합 불가능
- ✗ **토큰 관리 없음**: 인증 토큰 저장/갱신 로직 없음

---

## 2. 제안된 API 응답 스키마 분석

### 2.1 ActivatedMcpDetails Structure

**파일 위치**: `docs/mcp/API_RESPONSE_SCHEMA_FOR_USER_ACTIVATED_MCPs.md`

```typescript
export interface ActivatedMcpDetails {
  // 기본 정보
  id: string;
  name: string;
  description: string;
  logoUrl: string | null;

  // Transport 방식
  type: 'remote' | 'local';
  transport: 'stdio' | 'http' | 'sse';

  // Transport별 설정 (Discriminated Union)
  transportConfig: RemoteTransportConfig | StdioTransportConfig;

  // MCP 프로토콜
  protocolVersion: string;
  capabilities?: MCPCapabilities;
  serverInfo?: { name?: string; version?: string };

  // 사용자 입력 변수 상태
  inputVars?: InputVarWithValue[];
  allRequiredInputsProvided: boolean;

  // 활성화 메타데이터
  activatedAt: number;
  activationExpiresAt: number | null;

  // 준비 상태
  isReady: boolean;
  readinessIssues?: string[];
}
```

### 2.2 Transport Config Types

#### Remote Transport (OAuth MCP)

```typescript
interface RemoteTransportConfig {
  transport: 'http' | 'sse';
  url: string;

  // 인증
  authToken?: string;
  authTokenType?: 'bearer' | 'basic' | 'custom';

  // 토큰 만료 관리
  expiresAt: number | null;
  isExpired: boolean;
  expiresIn: number | null;
  requiresRefresh: boolean;

  // HTTP 설정
  headers?: Record<string, string>;
  customEnv?: Record<string, string>;
}
```

#### Local Transport (Stdio MCP)

```typescript
interface StdioTransportConfig {
  transport: 'stdio';
  command: string;
  args: string[];
  cwd?: string;
  env?: Record<string, string>;
}
```

### 2.3 Proposed Data Flow

```text
┌──────────────────────────────────────────────────────────────┐
│                     Frontend (React)                         │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  1. MCP Hub에서 활성화                                       │
│     POST /api/mcp-servers/{id}/activate                     │
│     → OAuth flow or input variables                         │
│                                                              │
│  2. 활성화된 MCP 목록 조회                                   │
│     GET /api/user/activated-mcps                            │
│     ┌──────────────────────────────────────┐                │
│     │ Response: {                          │                │
│     │   success: true,                     │                │
│     │   data: ActivatedMcpDetails[],       │                │
│     │   summary: {                         │                │
│     │     totalActivated: 3,               │                │
│     │     requiresRefresh: 1,              │                │
│     │     notReady: 1                      │                │
│     │   }                                  │                │
│     │ }                                    │                │
│     └──────────────────────────────────────┘                │
│                                                              │
│  3. Assistant 선택 시 MCP 연결                               │
│     - Ready MCPs만 connectServers() 호출                    │
│     - Not Ready MCPs는 경고 표시                            │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│                  Backend API (Next.js)                       │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Database:                                                   │
│  ┌─────────────────────────────────────────┐                │
│  │ mcpServers (MCP 정의)                   │                │
│  │   - id, name, type, transport           │                │
│  │   - command/args (stdio)                │                │
│  │   - url/headers (http/sse)              │                │
│  │   - OAuth config                        │                │
│  │   - inputVars[]                         │                │
│  └─────────────────────────────────────────┘                │
│                    ⬇                                         │
│  ┌─────────────────────────────────────────┐                │
│  │ userActivatedMCPs (사용자별 활성화)     │                │
│  │   - userId, mcpId                       │                │
│  │   - accessTokenEncrypted (OAuth)        │                │
│  │   - refreshTokenEncrypted               │                │
│  │   - userProvidedEnvEncrypted            │                │
│  │   - expiresAt                           │                │
│  └─────────────────────────────────────────┘                │
│                                                              │
│  API Route: /api/user/activated-mcps                        │
│    1. userActivatedMCPs + mcpServers JOIN                   │
│    2. decrypt(accessToken, userEnv)                         │
│    3. Build transportConfig                                 │
│    4. Check isReady status                                  │
│    5. Return ActivatedMcpDetails[]                          │
│                                                              │
└──────────────────────────────────────────────────────────────┘
                           ⬇
┌──────────────────────────────────────────────────────────────┐
│                 Tauri Backend (Rust) - 변경 최소화           │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  MCPServerManager - 기존 로직 유지                          │
│  - stdio transport: 기존과 동일                             │
│  - http/sse transport: 신규 추가 필요                       │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

---

## 3. 핵심 차이점 비교

### 3.1 Architecture Level

| 관점               | 현재 (LibrAgent)                       | 제안 (API Schema)                    |
| ------------------ | -------------------------------------- | ------------------------------------ |
| **저장 위치**      | Assistant 정의 내 (`mcpConfig`)        | 별도 DB 테이블 (`userActivatedMCPs`) |
| **활성화 개념**    | 없음 (Assistant마다 즉시 실행)         | 사용자별 영구 활성화                 |
| **Transport 지원** | Stdio만                                | Stdio + HTTP + SSE                   |
| **OAuth 지원**     | 없음                                   | 완전 지원 (RFC 9728)                 |
| **토큰 관리**      | 없음                                   | 암호화 저장 + 자동 갱신              |
| **사용자 입력**    | 환경변수만 (Assistant 정의에 하드코딩) | `inputVars` 스키마로 유연하게 관리   |
| **준비 상태**      | 없음 (항상 실행 시도)                  | `isReady` + `readinessIssues`        |

### 3.2 Type Structure Comparison

#### Current: Simple Stdio-Only

```typescript
// 현재: 간단하지만 제한적
interface MCPConfig {
  mcpServers?: Record<
    string,
    {
      command: string;
      args?: string[];
      env?: Record<string, string>;
    }
  >;
}

// 사용 예시
const config: MCPConfig = {
  mcpServers: {
    'my-server': {
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-filesystem'],
      env: { ROOT_PATH: '/home/user/docs' },
    },
  },
};
```

#### Proposed: Comprehensive Multi-Transport

```typescript
// 제안: 포괄적이고 확장 가능
interface ActivatedMcpDetails {
  type: 'remote' | 'local';
  transport: 'stdio' | 'http' | 'sse';
  transportConfig: RemoteTransportConfig | StdioTransportConfig;
  // ... 기타 메타데이터
}

// Remote OAuth MCP 예시
const githubMcp: ActivatedMcpDetails = {
  id: 'mcp_github_001',
  name: 'GitHub MCP',
  type: 'remote',
  transport: 'http',
  transportConfig: {
    transport: 'http',
    url: 'https://api.github.com/mcp',
    authToken: 'gho_abc123...',
    expiresAt: 1735689600000,
    expiresIn: 2592000,
    requiresRefresh: false,
    headers: { Authorization: 'Bearer ...' },
  },
  isReady: true,
};

// Local Stdio MCP 예시 (현재와 유사)
const localMcp: ActivatedMcpDetails = {
  id: 'mcp_local_001',
  name: 'Filesystem Server',
  type: 'local',
  transport: 'stdio',
  transportConfig: {
    transport: 'stdio',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-filesystem', '/home/user/docs'],
    env: { ROOT_PATH: '/home/user/docs' },
  },
  isReady: true,
};
```

### 3.3 Connection Lifecycle

#### Current Flow

```text
Assistant 선택
    ↓
MCPServerContext.connectServers(assistant.mcpConfig)
    ↓
Rust: listToolsFromConfig() - 모든 서버 spawn 시도
    ↓
성공 → availableTools 설정
실패 → 에러 로그, 계속 진행
```

**특징**:

- ✓ 간단함
- ✗ 매번 프로세스 spawn (오버헤드)
- ✗ 실패 시 복구 방법 없음
- ✗ OAuth 토큰 갱신 불가능

#### Proposed Flow

```text
[Initial Setup]
사용자가 MCP Hub에서 활성화
    ↓
POST /api/mcp-servers/{id}/activate
    ↓ (OAuth flow or input variables)
Database: userActivatedMCPs 저장
    ↓ (accessToken 암호화 저장)


[Runtime]
Assistant 선택
    ↓
GET /api/user/activated-mcps (캐시 5분)
    ↓
ActivatedMcpDetails[] 조회
    ↓
Ready MCPs만 필터링
    ↓
MCPServerContext.connectServers(readyMcps)
    ↓
Rust: 연결 (토큰이 이미 준비됨)
    ↓
성공 → availableTools 설정
토큰 만료 → 자동 갱신 or 사용자 알림
```

**특징**:

- ✓ 한 번 활성화하면 영구 사용
- ✓ 토큰 갱신 자동화
- ✓ 준비 상태 사전 확인
- ✓ 오류 상황 명확히 처리
- ✗ 초기 설정 복잡도 증가

### 3.4 Security Comparison

| 항목              | 현재                                   | 제안                                          |
| ----------------- | -------------------------------------- | --------------------------------------------- |
| **API Key 저장**  | Assistant 정의 (평문 or 암호화 불명확) | DB 암호화 저장 (`AES-256-GCM`)                |
| **토큰 관리**     | 없음                                   | 암호화 + 자동 갱신                            |
| **사용자별 격리** | Assistant 공유 시 API Key 공유됨       | `userId` 기반 격리                            |
| **환경변수 노출** | Frontend state에 그대로 저장           | 복호화는 서버에서만, 클라이언트는 최소 정보만 |
| **토큰 전송**     | N/A                                    | HTTPS only, 메모리 전용                       |

---

## 4. Gap Analysis

### 4.1 현재 구현에서 부족한 점

#### ❌ Remote MCP 지원 불가능

```typescript
// 현재: Stdio만 지원
interface MCPConfig {
  mcpServers?: Record<
    string,
    {
      command: string; // ← HTTP/SSE 방식 표현 불가능
      args?: string[];
      env?: Record<string, string>;
    }
  >;
}

// GitHub MCP 같은 Remote MCP 연결 불가능
// OAuth 토큰 저장/관리 불가능
```

#### ❌ 영구 활성화 개념 없음

```typescript
// 현재: 매번 Assistant 선택 시 재연결
// - 프로세스 spawn 오버헤드
// - 토큰 만료 대응 불가능
// - 사용자 입력값 영구 저장 없음
```

#### ❌ 사용자별 설정 격리 없음

```typescript
// 현재: Assistant 정의에 API Key 하드코딩
interface Assistant {
  mcpConfig: {
    mcpServers: {
      'my-mcp': {
        env: { API_KEY: 'sk-...' }; // ← 모든 사용자가 공유
      };
    };
  };
}

// 문제:
// 1. Multi-user 환경에서 API Key 공유됨
// 2. 사용자별 다른 credentials 사용 불가능
// 3. 보안 위험 (Assistant 내보내기 시 키 노출)
```

#### ❌ 토큰 갱신 메커니즘 없음

```typescript
// OAuth 토큰이 만료되면?
// → 현재: 실패하고 끝
// → 제안: refreshToken으로 자동 갱신 or 사용자 재인증 요청
```

#### ❌ 준비 상태 확인 없음

```typescript
// 현재: connectServers() 호출 시 무조건 시도
// 제안: isReady=false인 MCP는 사전에 필터링
```

### 4.2 제안된 스키마의 복잡도

#### 장점

- ✓ Remote/Local 모두 지원
- ✓ OAuth 완전 지원
- ✓ 사용자별 격리
- ✓ 토큰 자동 갱신
- ✓ 준비 상태 관리

#### 단점

- ✗ 구현 복잡도 높음 (Backend API 필요)
- ✗ DB 마이그레이션 필요
- ✗ 기존 Assistant mcpConfig 마이그레이션
- ✗ Rust backend에 HTTP/SSE transport 추가 필요

---

## 5. 통합 전략 제안

### 5.1 Migration Path

#### Phase 1: Backward Compatible Extension

**목표**: 기존 Stdio 방식 유지하면서 Remote MCP 지원 추가

```typescript
// src/models/chat.ts - 확장
export interface MCPConfig {
  // 기존 Stdio 방식 (하위 호환)
  mcpServers?: Record<
    string,
    {
      command: string;
      args?: string[];
      env?: Record<string, string>;
    }
  >;

  // 신규: 활성화된 Remote MCP ID 참조
  activatedMcpIds?: string[];
}

export interface Assistant {
  // ... 기존 필드
  mcpConfig: MCPConfig;
}
```

**사용 예시**:

```typescript
const assistant: Assistant = {
  name: 'Multi-Agent',
  mcpConfig: {
    // 기존 방식: Local Stdio MCP
    mcpServers: {
      'local-fs': {
        command: 'npx',
        args: ['-y', '@modelcontextprotocol/server-filesystem'],
        env: { ROOT_PATH: '/home/user' },
      },
    },

    // 신규: Remote MCP 참조
    activatedMcpIds: ['mcp_github_001', 'mcp_slack_002'],
  },
};
```

#### Phase 2: MCPServerContext 통합

```typescript
// src/context/MCPServerContext.tsx - 개선

const connectServers = async (mcpConfig: MCPConfig) => {
  try {
    // 1. Local Stdio MCPs (기존 방식)
    const stdioTools = mcpConfig.mcpServers
      ? await listToolsFromConfig({ mcpServers: mcpConfig.mcpServers })
      : {};

    // 2. Remote Activated MCPs (신규)
    let remoteTools = {};
    if (mcpConfig.activatedMcpIds?.length) {
      const activatedMcps = await fetchActivatedMcps(mcpConfig.activatedMcpIds);

      // Ready MCPs만 필터링
      const readyMcps = activatedMcps.filter((mcp) => mcp.isReady);

      // Not ready MCPs 경고
      const notReadyMcps = activatedMcps.filter((mcp) => !mcp.isReady);
      if (notReadyMcps.length > 0) {
        logger.warn(
          'Some MCPs are not ready:',
          notReadyMcps.map((m) => ({
            name: m.name,
            issues: m.readinessIssues,
          })),
        );
      }

      // Remote MCPs 연결
      remoteTools = await connectRemoteMcps(readyMcps);
    }

    // 3. 통합
    const allTools = { ...stdioTools, ...remoteTools };

    // ... 기존 로직
  } catch (error) {
    // ...
  }
};

// 신규 함수
async function fetchActivatedMcps(
  mcpIds: string[],
): Promise<ActivatedMcpDetails[]> {
  const response = await fetch('/api/user/activated-mcps', {
    method: 'POST',
    body: JSON.stringify({ mcpIds }),
  });
  return response.json();
}

async function connectRemoteMcps(
  mcps: ActivatedMcpDetails[],
): Promise<Record<string, MCPTool[]>> {
  const toolsByServer: Record<string, MCPTool[]> = {};

  for (const mcp of mcps) {
    if (mcp.transport === 'stdio') {
      // Stdio는 기존 방식 사용
      const config = {
        mcpServers: {
          [mcp.name]: {
            command: (mcp.transportConfig as StdioTransportConfig).command,
            args: (mcp.transportConfig as StdioTransportConfig).args,
            env: (mcp.transportConfig as StdioTransportConfig).env,
          },
        },
      };
      const tools = await listToolsFromConfig(config);
      toolsByServer[mcp.name] = tools[mcp.name];
    } else {
      // HTTP/SSE는 Rust backend 확장 필요
      const tools = await listToolsFromRemoteMcp(mcp);
      toolsByServer[mcp.name] = tools;
    }
  }

  return toolsByServer;
}
```

#### Phase 3: Rust Backend Extension

```rust
// src-tauri/src/mcp/manager.rs - HTTP/SSE 지원 추가

pub enum MCPTransport {
    Stdio {
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
    },
    Http {
        url: String,
        headers: HashMap<String, String>,
    },
    Sse {
        url: String,
        headers: HashMap<String, String>,
    },
}

impl MCPServerManager {
    pub async fn connect_mcp(
        &self,
        name: String,
        transport: MCPTransport,
    ) -> Result<Vec<MCPTool>, String> {
        match transport {
            MCPTransport::Stdio { command, args, env } => {
                // 기존 로직
                self.spawn_stdio_server(name, command, args, env).await
            }
            MCPTransport::Http { url, headers } => {
                // 신규: HTTP client 구현
                self.connect_http_server(name, url, headers).await
            }
            MCPTransport::Sse { url, headers } => {
                // 신규: SSE client 구현
                self.connect_sse_server(name, url, headers).await
            }
        }
    }
}
```

### 5.2 Implementation Checklist

#### Backend (Next.js API) - 7일

- [ ] **Day 1-2**: DB Schema & Migration
  - [ ] `mcpServers` 테이블 생성 (RFC 9728 포함)
  - [ ] `userActivatedMCPs` 테이블 생성
  - [ ] 암호화 함수 (`encrypt/decrypt`)

- [ ] **Day 3-4**: API Routes
  - [ ] `GET /api/featured-mcps` (MCP Hub)
  - [ ] `POST /api/mcp-servers/{id}/activate`
  - [ ] `GET /api/user/activated-mcps`
  - [ ] `GET /api/user/activated-mcps/{mcpId}`
  - [ ] OAuth callback handler

- [ ] **Day 5-6**: React Query Hooks
  - [ ] `useFeaturedMcps()`
  - [ ] `useActivatedMcps()`
  - [ ] `useActivateMcp()`
  - [ ] `useDeactivateMcp()`

- [ ] **Day 7**: Integration Testing
  - [ ] Remote OAuth MCP 테스트
  - [ ] Local Stdio MCP 테스트 (기존 방식)
  - [ ] 토큰 갱신 시나리오

#### Frontend (React) - 5일

- [ ] **Day 1-2**: MCPServerContext 확장
  - [ ] `activatedMcpIds` 지원 추가
  - [ ] `fetchActivatedMcps()` 함수
  - [ ] `connectRemoteMcps()` 함수
  - [ ] 준비 상태 확인 로직

- [ ] **Day 3**: MCP Hub UI
  - [ ] Featured MCPs 목록
  - [ ] Activate/Deactivate 버튼
  - [ ] OAuth flow redirect 처리
  - [ ] Input variables form

- [ ] **Day 4**: Settings UI
  - [ ] Activated MCPs 관리 페이지
  - [ ] 토큰 상태 표시
  - [ ] 갱신 필요 배지
  - [ ] Deactivate 기능

- [ ] **Day 5**: Testing & Polish
  - [ ] E2E 테스트
  - [ ] 에러 처리 개선
  - [ ] 로딩 상태 UX

#### Rust Backend (Tauri) - 10일

- [ ] **Day 1-3**: HTTP/SSE Transport
  - [ ] `reqwest` HTTP client 통합
  - [ ] SSE event stream 처리
  - [ ] MCP protocol over HTTP

- [ ] **Day 4-6**: Authentication
  - [ ] Bearer token 헤더 추가
  - [ ] 토큰 갱신 에러 처리
  - [ ] Credential 암호화 검증

- [ ] **Day 7-8**: MCPServerManager 확장
  - [ ] Transport enum (`Stdio | Http | Sse`)
  - [ ] `connect_http_server()`
  - [ ] `connect_sse_server()`

- [ ] **Day 9-10**: Testing
  - [ ] Unit tests (HTTP/SSE transport)
  - [ ] Integration tests (end-to-end)
  - [ ] Performance benchmarks

### 5.3 Rollout Strategy

#### Option A: Big Bang (Risk: High)

- 모든 변경사항 한 번에 배포
- 기존 Assistant mcpConfig 일괄 마이그레이션
- 빠르지만 위험함

#### Option B: Feature Flag (추천)

```typescript
// src/config/feature-flags.ts
export const FEATURE_FLAGS = {
  REMOTE_MCP_SUPPORT: process.env.ENABLE_REMOTE_MCP === 'true',
};

// MCPServerContext.tsx
if (FEATURE_FLAGS.REMOTE_MCP_SUPPORT && mcpConfig.activatedMcpIds) {
  // 신규 로직
} else {
  // 기존 로직 (fallback)
}
```

**장점**:

- ✓ 단계적 배포
- ✓ 문제 발생 시 즉시 롤백
- ✓ A/B 테스트 가능

#### Option C: Parallel Systems (가장 안전)

- 기존 Stdio 방식 그대로 유지
- Remote MCP는 별도 Context (`RemoteMCPContext`)
- 점진적으로 통합

---

## 6. 결론 및 권장사항

### 6.1 핵심 차이점 요약

| 항목          | 현재 LibrAgent     | 제안 API Schema               |
| ------------- | ------------------ | ----------------------------- |
| **Transport** | Stdio만            | Stdio + HTTP + SSE            |
| **OAuth**     | 미지원             | 완전 지원 (RFC 9728)          |
| **활성화**    | Assistant마다 설정 | 사용자별 영구 활성화          |
| **토큰 관리** | 없음               | 암호화 + 자동 갱신            |
| **보안**      | API Key 공유 가능  | 사용자별 격리                 |
| **준비 상태** | 없음               | `isReady` + `readinessIssues` |
| **복잡도**    | 낮음               | 높음 (DB + API 필요)          |

### 6.2 권장사항

#### 단기 (1-2주) - Phase 1

1. **Backend API 구축**
   - `/api/user/activated-mcps` 엔드포인트
   - DB schema 생성
   - 암호화 구현

2. **MCPConfig 확장**
   - `activatedMcpIds` 필드 추가
   - 하위 호환성 유지

3. **Feature Flag 도입**
   - 안전한 롤아웃 준비

#### 중기 (3-4주) - Phase 2

1. **Frontend 통합**
   - MCPServerContext 확장
   - MCP Hub UI 구축
   - OAuth flow 구현

2. **Rust Backend 확장**
   - HTTP/SSE transport 추가
   - 토큰 인증 로직

3. **Testing & Documentation**
   - E2E 테스트
   - 사용자 가이드

#### 장기 (2-3개월) - Phase 3

1. **기존 Assistant 마이그레이션**
   - Stdio MCPs를 activated MCPs로 전환
   - 사용자별 API Key 입력 유도

2. **고급 기능**
   - 토큰 자동 갱신
   - 헬스 체크
   - 배치 활성화

3. **Legacy 제거**
   - 기존 `mcpServers` 필드 deprecate
   - `activatedMcpIds`만 사용

### 6.3 최종 평가

**현재 구현의 강점**:

- ✓ 간단하고 이해하기 쉬움
- ✓ Stdio MCP에는 충분함
- ✓ 빠른 프로토타이핑에 적합

**제안 스키마의 필요성**:

- ✓ Remote MCP 생태계 통합 필수
- ✓ Multi-user 환경 보안 필수
- ✓ 장기적으로 확장 가능한 아키텍처

**결론**:

- Stdio 전용으로 충분하다면 현재 구조 유지
- Remote MCP 지원이 필요하다면 **Phase 1부터 시작하여 점진적 통합 추천**
- Feature Flag로 위험 관리하면서 병행 개발 가능

---

## 7. 참고 자료

- [MCP Protocol Specification](https://spec.modelcontextprotocol.io/)
- [RFC 9728 - OAuth 2.1 Protected Resource Metadata](https://datatracker.ietf.org/doc/html/rfc9728)
- [API Response Schema 상세 문서](./API_RESPONSE_SCHEMA_FOR_USER_ACTIVATED_MCPs.md)
- [MCP Activation API Guide](./MCP_ACTIVATION_API_GUIDE.md)
- LibrAgent 현재 구현:
  - `src/models/chat.ts` - MCPConfig 정의
  - `src/context/MCPServerContext.tsx` - 연결 로직
  - `src-tauri/src/mcp/manager.rs` - Rust backend
