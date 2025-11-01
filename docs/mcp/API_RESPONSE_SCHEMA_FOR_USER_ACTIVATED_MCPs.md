# MCP Server 활성화 상태 API 응답 스키마 설계

**작성일**: 2025-10-26  
**목적**: 사용자가 활성화한 MCP Server의 연결/구동에 필요한 모든 정보를 클라이언트에 제공하기 위한 API 응답 스키마 설계

---

## 📋 목차

1. [데이터베이스 스키마 분석](#1-데이터베이스-스키마-분석)
2. [현재 API 응답 구조 분석](#2-현재-api-응답-구조-분석)
3. [클라이언트 사용 시나리오](#3-클라이언트-사용-시나리오)
4. [포괄적 API 응답 스키마 설계](#4-포괄적-api-응답-스키마-설계)
5. [구현 가이드](#5-구현-가이드)

---

## 1. 데이터베이스 스키마 분석

### 1.1 MCP Server 엔티티 구조

```
mcpServers (MCP 서버 정의)
├── 기본정보: id, name, description, logoUrl
├── 타입 & Transport
│   ├── type: 'remote' (OAuth) | 'local' (서버리스 함수)
│   ├── transport: 'stdio' | 'http' | 'sse'
│
├── Transport별 설정
│   ├── [stdio - local 전용]
│   │   ├── command: "npx" | "python" | "node"
│   │   ├── commandArgs: string[]
│   │   ├── env: Record<string, string>
│   │   └── cwd: string (작업디렉토리)
│   │
│   └── [http/sse - remote 전용]
│       ├── url: "https://api.github.com/mcp"
│       └── headers: Record<string, string>
│
├── OAuth 2.1 / RFC 9728 구성
│   ├── resourceMetadataUrl (RFC 9728 자동발견)
│   ├── issuer, authorizationEndpoint, tokenEndpoint
│   ├── jwksUri, registrationEndpoint
│   ├── scopesSupported: string[]
│   └── grantTypesSupported: string[]
│
├── MCP 프로토콜
│   ├── protocolVersion: "2025-06-18"
│   ├── capabilities: { logging?, prompts?, resources?, tools? }
│   └── serverInfo: { name?, version? }
│
├── 사용자 입력 요구사항
│   └── inputVars: InputVar[]
│       ├── name: string (API_KEY, DATABASE_URL 등)
│       ├── required: boolean
│       ├── description: string
│       ├── type: 'env' | 'cmd' | 'header'
│       ├── argIndex?: number (cmd 타입일 때)
│       ├── headerKey?: string (header 타입일 때)
│       ├── placeholder: string
│       └── link: string (문서/발급 링크)
│
└── 메타데이터
    ├── isPublic, isVerified
    ├── publishStatus, publishRequestedAt, publishMessage
    ├── creatorId, createdAt, updatedAt
    └── avgCreditUsage (local 타입 크레딧 소모량)
```

### 1.2 사용자 활성화 MCP 엔티티 구조

```
userActivatedMCPs (사용자가 활성화한 MCP 상태)
├── id, userId, mcpId
├── 토큰 정보 (remote 타입)
│   ├── accessTokenEncrypted
│   ├── refreshTokenEncrypted
│   └── expiresAt
├── 환경변수 정보 (local/remote 타입)
│   └── userProvidedEnvEncrypted (사용자 입력값)
└── createdAt
```

### 1.3 핵심 관계도

```
Client Application
    ↓ (fetch activated MCPs)
    ↓
API Endpoint: GET /api/user/activated-mcps
    ↓
[userActivatedMCPs] ← (join) → [mcpServers]
    │                               │
    ├─ accessTokenEncrypted          ├─ transport info
    ├─ userProvidedEnvEncrypted      ├─ OAuth config
    └─ expiresAt                     ├─ capabilities
                                     ├─ required input vars
                                     ├─ command/url config
                                     └─ headers config
    ↓ (decrypt + merge)
    ↓
응답: ActivatedMcpDetails[]
```

---

## 2. 현재 API 응답 구조 분석

### 2.1 현재 응답 타입

**`src/types/api/featured.ts` - MCPServerCard**

```typescript
export const MCPServerCardSchema = z.object({
  id: z.string().min(1),
  name: z.string().min(1).max(255),
  description: z.string(),
  logoUrl: z.union([z.url(), z.literal(''), z.null()]),
  type: z.enum(['remote', 'local']),
  isActivated: z.boolean(),
  canActivate: z.boolean(),
  requiredScopes: z.array(z.string()).optional(),
});
```

**문제점**:

- ✗ 활성화된 MCP의 실제 연결/구동에 필요한 정보 부족
- ✗ Transport 설정 정보 없음
- ✗ 토큰/환경변수 정보 없음
- ✗ OAuth 엔드포인트 정보 없음
- ✗ 커맨드 실행 설정 없음
- ✗ 사용자 입력값 상태 불명확

### 2.2 현재 Activate/Deactivate API

**Request**: `POST /api/mcp-servers/{id}/activate`

```typescript
{ requiredInputs?: Record<string, string> }
```

**Response**:

```typescript
{ success: boolean; message?: string; error?: string; authUrl?: string }
```

**문제점**:

- ✗ 활성화 후 클라이언트가 MCP 연결에 필요한 정보를 별도로 조회해야 함
- ✗ 토큰 만료 시간, 갱신 전략 정보 없음
- ✗ MCP 구동 방식(stdio/http/sse)에 따른 클라이언트 동작 방식 불명확

---

## 3. 클라이언트 사용 시나리오

### Scenario 1: Remote OAuth MCP (GitHub 통합)

**문제**: Client가 GitHub MCP를 활성화하려고 함

**클라이언트 필요 정보**:

```
✓ OAuth flow 진행 여부 (이미 토큰 있는지)
✓ Authorization endpoint (OAuth 로그인 URL)
✓ 필요한 scopes (read:repo, admin:repo_hook 등)
✓ 토큰 상태 (유효한지, 만료 예정인지)
✓ MCP 서버 URL (토큰으로 authenticated request 전송)
✓ HTTP 헤더 설정 (Authorization: Bearer token)
```

### Scenario 2: Local Serverless MCP (AI 분석 함수)

**문제**: Client가 Local MCP를 활성화하고 구동하려고 함

**클라이언트 필요 정보**:

```
✓ 실행 커맨드 (npx, python, node 등)
✓ 커맨드 인자 (API_KEY 값으로 치환된 최종 args)
✓ 환경변수 (DB_URL, API_KEY 등 - 이미 사용자 입력값 적용됨)
✓ 작업 디렉토리
✓ MCP 프로토콜 버전
✓ MCP 서버 capabilities (tools, resources, prompts 지원 여부)
```

### Scenario 3: Token Refresh 예상

**문제**: Client가 토큰 만료 예상 시간을 알아야 함

**클라이언트 필요 정보**:

```
✓ 토큰 만료 시간 (Unix timestamp)
✓ 토큰이 곧 만료되는지 여부 (플래그)
✓ 갱신 필요 여부 (refresh token 있는지)
```

### Scenario 4: 다중 MCP 활성화 상태 조회

**문제**: Agent가 여러 MCP를 동시에 구동해야 함

**클라이언트 필요 정보**:

```
✓ 활성화된 모든 MCP 목록
✓ 각 MCP별 transport 방식 (stdio/http/sse)
✓ 각 MCP별 초기화 정보 (command, url, headers)
✓ 어느 MCP가 토큰 갱신 필요한지
```

---

## 4. 포괄적 API 응답 스키마 설계

### 4.1 개선된 MCP Server Card (기존 개선)

**용도**: Featured/Search 응답에서 사용자가 활성화 여부 확인

```typescript
// src/types/api/featured.ts 개선
export const EnhancedMCPServerCardSchema = z.object({
  id: z.string().min(1),
  name: z.string().min(1).max(255),
  description: z.string(),
  logoUrl: z.union([z.url(), z.literal(''), z.null()]),

  // Transport 정보 추가
  type: z.enum(['remote', 'local']),
  transport: z.enum(['stdio', 'http', 'sse']),

  // 활성화 상태
  isActivated: z.boolean(),
  canActivate: z.boolean(),

  // 필요한 입력 정보
  requiredInputs: z
    .array(
      z.object({
        name: z.string(),
        required: z.boolean(),
        description: z.string(),
        type: z.enum(['env', 'cmd', 'header']),
      }),
    )
    .optional(),

  // OAuth 정보 (remote 타입)
  requiredScopes: z.array(z.string()).optional(),
});

export type EnhancedMCPServerCard = z.infer<typeof EnhancedMCPServerCardSchema>;
```

### 4.2 활성화된 MCP 상세 정보 (핵심)

**용도**: `GET /api/user/activated-mcps/{mcpId}` 또는 `GET /api/user/activated-mcps`

```typescript
// src/types/api/mcps.ts 추가

// ============================================================================
// ACTIVATED MCP DETAILS - 클라이언트가 MCP 연결/구동에 필요한 모든 정보
// ============================================================================

/**
 * HTTP/SSE Transport 설정
 * remote 타입의 MCP에서 사용
 */
export const RemoteTransportConfigSchema = z.object({
  transport: z.literal('http') | z.literal('sse'),
  url: z.string().url(),

  // 인증 토큰 정보
  authToken: z.string().optional(), // Bearer token (access token)
  authTokenType: z.enum(['bearer', 'basic', 'custom']).optional(),

  // 토큰 만료 정보
  expiresAt: z.number().int().nullable(), // Unix timestamp (ms)
  isExpired: z.boolean(), // 현재 만료됨
  expiresIn: z.number().int().nullable(), // 남은 시간 (초)
  requiresRefresh: z.boolean(), // 곧 만료될 예정 (5분 이내)

  // HTTP 헤더
  headers: z.record(z.string(), z.string()).optional(),

  // 사용자가 입력한 커스텀 환경변수 (env 타입으로 지정된 것들)
  customEnv: z.record(z.string(), z.string()).optional(),
});

export type RemoteTransportConfig = z.infer<typeof RemoteTransportConfigSchema>;

/**
 * Stdio Transport 설정
 * local 타입의 MCP에서 사용
 */
export const StdioTransportConfigSchema = z.object({
  transport: z.literal('stdio'),

  // 실행 정보
  command: z.string(), // "npx", "python", "node" 등
  args: z.array(z.string()), // 최종 인자 (inputVars 값 이미 적용됨)
  cwd: z.string().optional(), // 작업 디렉토리

  // 환경변수 (inputVars의 env 타입 값들이 이미 적용됨)
  env: z.record(z.string(), z.string()).optional(),
});

export type StdioTransportConfig = z.infer<typeof StdioTransportConfigSchema>;

/**
 * MCP 프로토콜 Capabilities
 */
export const MCPCapabilitiesSchema = z.object({
  logging: z.object({}).optional(),
  prompts: z
    .object({
      listChanged: z.boolean().optional(),
    })
    .optional(),
  resources: z
    .object({
      subscribe: z.boolean().optional(),
      listChanged: z.boolean().optional(),
    })
    .optional(),
  tools: z
    .object({
      listChanged: z.boolean().optional(),
    })
    .optional(),
  experimental: z.record(z.string(), z.object({})).optional(),
});

export type MCPCapabilities = z.infer<typeof MCPCapabilitiesSchema>;

/**
 * 필수 입력 변수 (사용자가 입력한 값)
 */
export const InputVarWithValueSchema = z.object({
  name: z.string(), // API_KEY, DATABASE_URL 등
  type: z.enum(['env', 'cmd', 'header']),
  argIndex: z.number().int().optional(), // type='cmd'일 때
  headerKey: z.string().optional(), // type='header'일 때
  description: z.string(),
  required: z.boolean(),

  // 사용자 입력 상태
  isProvided: z.boolean(), // 값이 입력되었는지
  lastUpdatedAt: z.number().int().nullable(), // 마지막 수정 시간 (Unix timestamp ms)
});

export type InputVarWithValue = z.infer<typeof InputVarWithValueSchema>;

/**
 * 활성화된 MCP의 전체 정보
 * 클라이언트가 MCP 연결/구동에 필요한 모든 정보 포함
 */
export const ActivatedMcpDetailsSchema = z.object({
  // 기본정보
  id: z.string().min(1),
  name: z.string().min(1).max(255),
  description: z.string(),
  logoUrl: z.union([z.url(), z.literal(''), z.null()]).nullable(),

  // MCP 타입 & Transport
  type: z.enum(['remote', 'local']),
  transport: z.enum(['stdio', 'http', 'sse']),

  // Transport별 설정 (discriminated union)
  transportConfig: z.union([
    RemoteTransportConfigSchema,
    StdioTransportConfigSchema,
  ]),

  // MCP 프로토콜
  protocolVersion: z.string(),
  capabilities: MCPCapabilitiesSchema.optional(),
  serverInfo: z
    .object({
      name: z.string().optional(),
      version: z.string().optional(),
    })
    .optional(),

  // 필수 입력 변수 상태
  inputVars: z.array(InputVarWithValueSchema).optional(),
  allRequiredInputsProvided: z.boolean(), // 모든 필수 입력값이 제공되었는지

  // 활성화 메타데이터
  activatedAt: z.number().int(), // Unix timestamp (ms)
  activationExpiresAt: z.number().int().nullable(), // 활성화 만료 시간

  // 초기화 준비 상태
  isReady: z.boolean(), // 클라이언트가 즉시 사용 가능한지
  readinessIssues: z.array(z.string()).optional(), // 준비 불가 사유
});

export type ActivatedMcpDetails = z.infer<typeof ActivatedMcpDetailsSchema>;

export const ActivatedMcpDetailsResponseSchema = z.object({
  success: z.literal(true),
  data: ActivatedMcpDetailsSchema,
});

export type ActivatedMcpDetailsResponse = z.infer<
  typeof ActivatedMcpDetailsResponseSchema
>;

/**
 * 여러 활성화된 MCP 목록 응답
 * 용도: GET /api/user/activated-mcps
 */
export const ActivatedMcpsListResponseSchema = z.object({
  success: z.literal(true),
  data: z.array(ActivatedMcpDetailsSchema),
  total: z.number().int().nonnegative(),

  // 요약 정보
  summary: z
    .object({
      totalActivated: z.number().int().nonnegative(),
      requiresRefresh: z.number().int().nonnegative(), // 토큰 갱신 필요한 개수
      notReady: z.number().int().nonnegative(), // 준비 불완료 개수
      lastSyncedAt: z.number().int(), // 마지막 동기화 시간
    })
    .optional(),
});

export type ActivatedMcpsListResponse = z.infer<
  typeof ActivatedMcpsListResponseSchema
>;
```

### 4.3 예시: 실제 응답 데이터

#### Remote OAuth MCP (GitHub)

```json
{
  "success": true,
  "data": {
    "id": "mcp_github_001",
    "name": "GitHub MCP",
    "description": "GitHub API integration with MCP protocol",
    "logoUrl": "https://github.com/logo.png",
    "type": "remote",
    "transport": "http",

    "transportConfig": {
      "transport": "http",
      "url": "https://mcp-hub.example.com/mcps/github/v1",
      "authToken": "gho_abcdef1234567890",
      "authTokenType": "bearer",
      "expiresAt": 1735689600000,
      "isExpired": false,
      "expiresIn": 2592000,
      "requiresRefresh": false,

      "headers": {
        "Authorization": "Bearer gho_abcdef1234567890",
        "X-GitHub-Api-Version": "2022-11-28",
        "Accept": "application/vnd.github+json"
      },

      "customEnv": {}
    },

    "protocolVersion": "2025-06-18",
    "capabilities": {
      "tools": {
        "listChanged": true
      },
      "resources": {
        "subscribe": true,
        "listChanged": true
      }
    },
    "serverInfo": {
      "name": "github-mcp",
      "version": "1.0.0"
    },

    "inputVars": [
      {
        "name": "GITHUB_TOKEN",
        "type": "header",
        "headerKey": "Authorization",
        "description": "GitHub Personal Access Token",
        "required": true,
        "isProvided": true,
        "lastUpdatedAt": 1704067200000
      }
    ],
    "allRequiredInputsProvided": true,

    "activatedAt": 1704067200000,
    "activationExpiresAt": 1735689600000,

    "isReady": true,
    "readinessIssues": []
  }
}
```

#### Local Serverless MCP (AI 분석)

```json
{
  "success": true,
  "data": {
    "id": "mcp_ai_analysis_001",
    "name": "AI Analysis Server",
    "description": "Local AI-powered analysis tools",
    "logoUrl": null,
    "type": "local",
    "transport": "stdio",

    "transportConfig": {
      "transport": "stdio",
      "command": "npx",
      "args": [
        "-y",
        "@modelcontextprotocol/server-custom-analysis",
        "--api-key=sk_1234567890abcdef",
        "--db-url=postgresql://user:pass@localhost:5432/analysis_db"
      ],
      "cwd": "/usr/local/mcp-servers",

      "env": {
        "API_KEY": "sk_1234567890abcdef",
        "DATABASE_URL": "postgresql://user:pass@localhost:5432/analysis_db",
        "NODE_ENV": "production"
      }
    },

    "protocolVersion": "2025-06-18",
    "capabilities": {
      "tools": {
        "listChanged": true
      },
      "resources": {
        "subscribe": false,
        "listChanged": true
      }
    },
    "serverInfo": {
      "name": "ai-analysis-server",
      "version": "2.1.0"
    },

    "inputVars": [
      {
        "name": "API_KEY",
        "type": "cmd",
        "argIndex": 2,
        "description": "OpenAI API Key",
        "required": true,
        "isProvided": true,
        "lastUpdatedAt": 1704067200000
      },
      {
        "name": "DATABASE_URL",
        "type": "env",
        "description": "PostgreSQL connection string",
        "required": true,
        "isProvided": true,
        "lastUpdatedAt": 1704067200000
      }
    ],
    "allRequiredInputsProvided": true,

    "activatedAt": 1704067200000,
    "activationExpiresAt": null,

    "isReady": true,
    "readinessIssues": []
  }
}
```

#### Not Ready MCP (입력값 미설정)

```json
{
  "success": true,
  "data": {
    "id": "mcp_slack_001",
    "name": "Slack MCP",
    "description": "Slack API integration",
    "logoUrl": "https://slack.com/logo.png",
    "type": "remote",
    "transport": "http",

    "transportConfig": {
      "transport": "http",
      "url": "https://mcp-hub.example.com/mcps/slack/v1",
      "authToken": null,
      "authTokenType": null,
      "expiresAt": null,
      "isExpired": false,
      "expiresIn": null,
      "requiresRefresh": true,
      "headers": {}
    },

    "protocolVersion": "2025-06-18",
    "capabilities": {
      "tools": { "listChanged": true },
      "resources": { "subscribe": true }
    },

    "inputVars": [
      {
        "name": "SLACK_BOT_TOKEN",
        "type": "header",
        "headerKey": "Authorization",
        "description": "Slack Bot Token (xoxb-...)",
        "required": true,
        "isProvided": false,
        "lastUpdatedAt": null
      }
    ],
    "allRequiredInputsProvided": false,

    "activatedAt": 1704067200000,
    "activationExpiresAt": null,

    "isReady": false,
    "readinessIssues": [
      "Missing required input: SLACK_BOT_TOKEN",
      "Authorization token not provided"
    ]
  }
}
```

#### Multi MCP List Response

```json
{
  "success": true,
  "data": [
    {
      /* GitHub MCP - ActivatedMcpDetails */
    },
    {
      /* AI Analysis MCP - ActivatedMcpDetails */
    },
    {
      /* Slack MCP - ActivatedMcpDetails (not ready) */
    }
  ],
  "total": 3,
  "summary": {
    "totalActivated": 3,
    "requiresRefresh": 1,
    "notReady": 1,
    "lastSyncedAt": 1704067800000
  }
}
```

---

## 5. 구현 가이드

### 5.1 TypeScript 스키마 구현 위치

```
src/types/api/
├── mcps.ts (기존 - 개선)
│   ├── ActivateMcpRequest/Response (기존)
│   ├── DeactivateMcpRequest/Response (기존)
│   ├── RemoteTransportConfig (신규)
│   ├── StdioTransportConfig (신규)
│   ├── MCPCapabilities (신규)
│   ├── InputVarWithValue (신규)
│   ├── ActivatedMcpDetails (신규 - 핵심)
│   ├── ActivatedMcpDetailsResponse (신규)
│   ├── ActivatedMcpsListResponse (신규)
│   └── ...
```

### 5.2 API Route Handler 구현 위치

```
src/app/api/
├── user/
│   └── activated-mcps/
│       ├── route.ts (GET - 전체 목록)
│       └── [mcpId]/
│           └── route.ts (GET - 단일 상세)
```

### 5.3 Route Handler 구현 템플릿

#### `src/app/api/user/activated-mcps/route.ts`

```typescript
import { auth } from '@/lib/auth';
import { db } from '@/lib/db';
import { userActivatedMCPs, mcpServers, users } from '@/lib/schema';
import { eq } from 'drizzle-orm';
import { decrypt } from '@/lib/crypto';
import { NextResponse } from 'next/server';
import type { ActivatedMcpsListResponse } from '@/types/api/mcps';

export async function GET(request: Request): Promise<Response> {
  const session = await auth();
  if (!session?.user?.id) {
    return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
  }

  try {
    // 1. userActivatedMCPs + mcpServers 조인 조회
    const activatedMCPs = await db.query.userActivatedMCPs.findMany({
      where: eq(userActivatedMCPs.userId, session.user.id),
      with: {
        mcpServer: true,
      },
    });

    // 2. 각 MCP마다 ActivatedMcpDetails 생성
    const details = await Promise.all(
      activatedMCPs.map(async (activation) => {
        const mcp = activation.mcpServer;

        // 토큰/환경변수 복호화
        let decryptedEnv: Record<string, string> = {};
        if (activation.userProvidedEnvEncrypted) {
          const envStr = decrypt(activation.userProvidedEnvEncrypted);
          decryptedEnv = JSON.parse(envStr);
        }

        // transport별 config 생성
        let transportConfig: any;

        if (mcp.type === 'remote') {
          // Remote OAuth 설정
          let authToken = undefined;
          if (activation.accessTokenEncrypted) {
            authToken = decrypt(activation.accessTokenEncrypted);
          }

          const now = Date.now();
          const expiresAt = activation.expiresAt?.getTime() ?? null;
          const isExpired = expiresAt ? expiresAt < now : false;
          const expiresIn = expiresAt
            ? Math.round((expiresAt - now) / 1000)
            : null;
          const requiresRefresh = expiresIn ? expiresIn < 300 : false; // 5분 이내

          transportConfig = {
            transport: mcp.transport,
            url: mcp.url,
            authToken,
            authTokenType: 'bearer',
            expiresAt,
            isExpired,
            expiresIn,
            requiresRefresh,
            headers: mcp.headers || {},
            customEnv: decryptedEnv,
          };
        } else {
          // Local Stdio 설정
          const finalArgs = (mcp.commandArgs || []).map(
            (arg: string, idx: number) => {
              const inputVar = mcp.inputVars?.find(
                (v: any) => v.type === 'cmd' && v.argIndex === idx,
              );
              if (inputVar && decryptedEnv[inputVar.name]) {
                return decryptedEnv[inputVar.name];
              }
              return arg;
            },
          );

          transportConfig = {
            transport: 'stdio',
            command: mcp.command,
            args: finalArgs,
            cwd: mcp.cwd,
            env: { ...mcp.env, ...decryptedEnv },
          };
        }

        // 필수 입력값 상태 확인
        const inputVars = (mcp.inputVars || []).map((v: any) => ({
          name: v.name,
          type: v.type,
          argIndex: v.argIndex,
          headerKey: v.headerKey,
          description: v.description,
          required: v.required,
          isProvided: !!decryptedEnv[v.name],
          lastUpdatedAt: activation.createdAt?.getTime() ?? null,
        }));

        const allRequiredInputsProvided = inputVars
          .filter((v) => v.required)
          .every((v) => v.isProvided);

        const isReady =
          allRequiredInputsProvided && (mcp.type === 'local' || !!authToken);

        return {
          id: mcp.id,
          name: mcp.name,
          description: mcp.description,
          logoUrl: mcp.logoUrl,
          type: mcp.type,
          transport: mcp.transport,
          transportConfig,
          protocolVersion: mcp.protocolVersion,
          capabilities: mcp.capabilities,
          serverInfo: mcp.serverInfo,
          inputVars,
          allRequiredInputsProvided,
          activatedAt: activation.createdAt.getTime(),
          activationExpiresAt: activation.expiresAt?.getTime() ?? null,
          isReady,
          readinessIssues: !isReady
            ? [
                !allRequiredInputsProvided && 'Missing required inputs',
                mcp.type === 'remote' &&
                  !authToken &&
                  'Missing authentication token',
              ].filter(Boolean)
            : [],
        };
      }),
    );

    const response: ActivatedMcpsListResponse = {
      success: true,
      data: details,
      total: details.length,
      summary: {
        totalActivated: details.length,
        requiresRefresh: details.filter(
          (d) => d.transportConfig.requiresRefresh,
        ).length,
        notReady: details.filter((d) => !d.isReady).length,
        lastSyncedAt: Date.now(),
      },
    };

    return NextResponse.json(response);
  } catch (error) {
    console.error('Error fetching activated MCPs:', error);
    return NextResponse.json(
      { error: 'Internal server error' },
      { status: 500 },
    );
  }
}
```

### 5.4 Client Hook 구현

```typescript
// src/hooks/useQueries/useActivatedMcps.ts

import { useQuery } from '@tanstack/react-query';
import { ApiClient } from '@/services/apiClient';
import type {
  ActivatedMcpsListResponse,
  ActivatedMcpDetails,
} from '@/types/api/mcps';
import { queryKeys } from '@/lib/queryKeys';

export function useActivatedMcps() {
  return useQuery({
    queryKey: queryKeys.mcps.all,
    queryFn: async (): Promise<ActivatedMcpsListResponse> => {
      return ApiClient.get('/api/user/activated-mcps');
    },
  });
}

export function useActivatedMcpById(mcpId: string) {
  return useQuery({
    queryKey: queryKeys.mcps.detail(mcpId),
    queryFn: async () => {
      const response = await ApiClient.get<ActivatedMcpDetailsResponse>(
        `/api/user/activated-mcps/${mcpId}`,
      );
      return response.data;
    },
    enabled: !!mcpId,
  });
}
```

### 5.5 클라이언트 사용 예시

```typescript
// src/components/agent/AgentRuntime.tsx

import { useActivatedMcps } from '@/hooks/useQueries/useActivatedMcps';
import type { ActivatedMcpDetails } from '@/types/api/mcps';

export function AgentRuntime() {
  const { data: response, isLoading } = useActivatedMcps();

  if (isLoading) return <div>Loading MCP connections...</div>;

  const activeMcps = response?.data || [];
  const readyMcps = activeMcps.filter((mcp) => mcp.isReady);
  const notReadyMcps = activeMcps.filter((mcp) => !mcp.isReady);

  return (
    <div>
      <h2>Active MCPs: {readyMcps.length}/{activeMcps.length}</h2>

      {notReadyMcps.length > 0 && (
        <div className="text-amber-600">
          <p>Setup Required:</p>
          <ul>
            {notReadyMcps.map((mcp) => (
              <li key={mcp.id}>
                {mcp.name}: {mcp.readinessIssues?.join(', ')}
              </li>
            ))}
          </ul>
        </div>
      )}

      {readyMcps.map((mcp) => (
        <MCPConnectionCard key={mcp.id} mcp={mcp} />
      ))}
    </div>
  );
}

// 클라이언트: MCP 연결 초기화
function MCPConnectionCard({ mcp }: { mcp: ActivatedMcpDetails }) {
  const { transportConfig, type, transport } = mcp;

  const initializeConnection = async () => {
    if (type === 'remote') {
      // HTTP/SSE 연결
      const headers = {
        ...transportConfig.headers,
        'Authorization': `Bearer ${transportConfig.authToken}`,
      };

      const response = await fetch(transportConfig.url, {
        method: 'POST',
        headers,
        body: JSON.stringify({ method: 'initialize', params: {} }),
      });

      return response.json();
    } else {
      // Stdio 연결 (Node.js ChildProcess 또는 WASM 활용)
      const proc = spawn(
        transportConfig.command,
        transportConfig.args,
        {
          cwd: transportConfig.cwd,
          env: { ...process.env, ...transportConfig.env },
        }
      );

      return proc;
    }
  };

  return (
    <div>
      <h3>{mcp.name}</h3>
      <p>Transport: {transport}</p>
      <p>Token Expires In: {transportConfig.expiresIn}s</p>
      {transportConfig.requiresRefresh && (
        <button onClick={() => refreshToken(mcp.id)}>
          Refresh Token
        </button>
      )}
      <button onClick={initializeConnection}>
        Initialize Connection
      </button>
    </div>
  );
}
```

---

## 6. 체크리스트

### 필수 구현 항목

- [ ] `src/types/api/mcps.ts` 개선 및 신규 스키마 추가
  - [ ] `RemoteTransportConfigSchema`
  - [ ] `StdioTransportConfigSchema`
  - [ ] `ActivatedMcpDetailsSchema`
  - [ ] `ActivatedMcpsListResponseSchema`

- [ ] API Route Handler 구현
  - [ ] `GET /api/user/activated-mcps` (전체 목록)
  - [ ] `GET /api/user/activated-mcps/{mcpId}` (단일 상세)

- [ ] 토큰/환경변수 복호화 로직
  - [ ] 활성화 시 저장된 암호화된 값 복호화
  - [ ] final args/headers에 값 적용

- [ ] 클라이언트 Hook 구현
  - [ ] `useActivatedMcps()` (전체 목록)
  - [ ] `useActivatedMcpById()` (단일 상세)

- [ ] 클라이언트 통합 테스트
  - [ ] Remote MCP (OAuth) 연결 테스트
  - [ ] Local MCP (Stdio) 연결 테스트
  - [ ] 토큰 갱신 시나리오 테스트
  - [ ] 미입력 필드 에러 처리 테스트

### 선택적 개선 항목

- [ ] 토큰 만료 자동 갱신 로직
- [ ] 배치 갱신 (여러 토큰 동시 갱신)
- [ ] WebSocket 기반 실시간 MCP 상태 동기화
- [ ] MCP 연결 헬스 체크 (주기적 상태 확인)
- [ ] 에러 복구 자동화 (토큰 갱신 실패 시 사용자 알림)

---

## 7. 참고 사항

### Security Considerations

1. **토큰 암호화**:
   - 저장: `encrypt()` 함수로 암호화하여 DB에 저장
   - 응답: 클라이언트에는 암호화되지 않은 token 전송 (HTTPS만 사용)
   - 저장: 클라이언트는 토큰을 메모리에만 보관 (localStorage X)

2. **환경변수 보안**:
   - 모든 `inputVars` 값은 서버에서만 암호화/복호화
   - 클라이언트는 최종 command args만 전달받음

3. **OAuth 토큰 갱신**:
   - `refreshTokenEncrypted` 사용하여 자동 갱신
   - 갱신 실패 시 사용자에게 재인증 요청

### Performance Considerations

1. **조회 최적화**:
   - Relational query로 userActivatedMCPs + mcpServers 한 번에 조회
   - N+1 문제 방지 (`.with({ mcpServer: true })`)

2. **캐싱**:
   - React Query로 클라이언트 캐싱 (기본 5분)
   - 토큰 갱신 시 캐시 무효화

3. **복호화 비용**:
   - 활성화된 MCP가 많을 경우 복호화 연산량 증가
   - 필요시 Redis 캐싱 고려

---

## 8. 추가 리소스

- MCP 스펙: https://spec.modelcontextprotocol.io/
- OAuth 2.1: https://datatracker.ietf.org/doc/html/draft-ietf-oauth-v2-1
- RFC 9728 (Protected Resource Metadata): https://datatracker.ietf.org/doc/html/rfc9728
- Auth.js Adapter: https://authjs.dev/
