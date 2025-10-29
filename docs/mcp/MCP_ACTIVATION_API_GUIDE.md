# MCP Server Activation API - Implementation Guide

**작성일**: 2025-10-26  
**상태**: 구현 준비 완료

---

## 개요

사용자가 활성화한 MCP Server를 클라이언트에서 실제로 연결/구동하기 위한 API 응답 스키마 설계입니다.

**문제**: 기존 API는 단순 정보(name, description)만 반환하여 클라이언트가 실제 연결 불가

**해결책**: 토큰, 환경변수, 커맨드, URL 등 완전한 연결 정보를 포함한 응답 제공

---

## API 응답 구조

### 엔드포인트

```
GET /api/user/activated-mcps              // 전체 목록
GET /api/user/activated-mcps/{mcpId}      // 단일 상세
```

### Response Type: ActivatedMcpDetails

```typescript
{
  // 기본
  id: string
  name: string
  description: string
  logoUrl: string | null

  // 연결 방식
  type: 'remote' | 'local'
  transport: 'stdio' | 'http' | 'sse'

  // 연결 설정 (transport별로 다름)
  transportConfig: RemoteTransportConfig | StdioTransportConfig

  // 프로토콜
  protocolVersion: string
  capabilities?: MCPCapabilities

  // 필수 입력 상태
  inputVars?: InputVarWithValue[]
  allRequiredInputsProvided: boolean

  // 준비 상태
  activatedAt: number (timestamp ms)
  isReady: boolean
  readinessIssues?: string[]
}
```

### Remote Transport Config (OAuth MCP)

```typescript
{
  transport: 'http' | 'sse';
  url: string; // MCP 서버 URL
  authToken: string; // Bearer token (복호화됨)
  expiresAt: number | null; // 토큰 만료 시간 (ms)
  expiresIn: number | null; // 남은 시간 (초)
  requiresRefresh: boolean; // 5분 이내 만료 여부
  headers: Record<string, string>;
  customEnv: Record<string, string>;
}
```

### Local Transport Config (Stdio MCP)

```typescript
{
  transport: 'stdio'
  command: string                // "npx" | "python" | "node"
  args: string[]                 // 최종 인자 (값 이미 적용됨)
  cwd?: string                   // 작업 디렉토리
  env: Record<string, string>    // 환경변수 (값 포함)
}
```

---

## 실제 사용 예시

### List Response

```json
{
  "success": true,
  "data": [
    {
      "id": "mcp_github_001",
      "name": "GitHub MCP",
      "type": "remote",
      "transport": "http",
      "transportConfig": {
        "url": "https://mcp-hub.example.com/mcps/github/v1",
        "authToken": "gho_abc123...",
        "expiresAt": 1735689600000,
        "expiresIn": 2592000,
        "requiresRefresh": false,
        "headers": { "Authorization": "Bearer gho_abc123..." }
      },
      "isReady": true,
      "readinessIssues": []
    }
  ],
  "total": 1,
  "summary": {
    "totalActivated": 1,
    "requiresRefresh": 0,
    "notReady": 0,
    "lastSyncedAt": 1704067800000
  }
}
```

---

## 구현 체크리스트

### Backend (7일)

**Day 1: 스키마 정의**

- [ ] `src/types/api/mcps.ts` 에 스키마 추가
  - `RemoteTransportConfigSchema`
  - `StdioTransportConfigSchema`
  - `ActivatedMcpDetailsSchema`
  - `ActivatedMcpsListResponseSchema`

**Day 2-3: API Route Handler**

- [ ] `src/app/api/user/activated-mcps/route.ts` 구현
  - 인증 확인 (auth())
  - userActivatedMCPs + mcpServers 조인
  - 토큰 복호화 (decrypt)
  - transportConfig 생성
  - isReady 판단

**Day 4: React Query Hook**

- [ ] `src/hooks/useQueries/useActivatedMcps.ts` 구현
  - `useActivatedMcps()` - 전체 목록
  - `useActivatedMcpById(mcpId)` - 단일 상세
  - staleTime: 5분 설정

**Day 5-6: 클라이언트 통합**

- [ ] 컴포넌트에서 Hook 사용
- [ ] Ready MCPs 표시
- [ ] Not Ready MCPs 경고
- [ ] 토큰 갱신 필요 시 배지

**Day 7: 테스트**

- [ ] Remote MCP (GitHub) 테스트
- [ ] Local MCP (Stdio) 테스트
- [ ] 토큰 갱신 시나리오 테스트
- [ ] Not Ready 상태 테스트

### 타입 정의 예시

```typescript
// src/types/api/mcps.ts

export const RemoteTransportConfigSchema = z.object({
  transport: z.literal('http').or(z.literal('sse')),
  url: z.string().url(),
  authToken: z.string().optional(),
  expiresAt: z.number().int().nullable(),
  expiresIn: z.number().int().nullable(),
  requiresRefresh: z.boolean(),
  headers: z.record(z.string(), z.string()).optional(),
  customEnv: z.record(z.string(), z.string()).optional(),
});

export const StdioTransportConfigSchema = z.object({
  transport: z.literal('stdio'),
  command: z.string(),
  args: z.array(z.string()),
  cwd: z.string().optional(),
  env: z.record(z.string(), z.string()).optional(),
});

export const ActivatedMcpDetailsSchema = z.object({
  id: z.string().min(1),
  name: z.string().min(1).max(255),
  description: z.string(),
  logoUrl: z.union([z.url(), z.literal(''), z.null()]).nullable(),
  type: z.enum(['remote', 'local']),
  transport: z.enum(['stdio', 'http', 'sse']),
  transportConfig: z.union([
    RemoteTransportConfigSchema,
    StdioTransportConfigSchema,
  ]),
  protocolVersion: z.string(),
  capabilities: z.any().optional(),
  inputVars: z.array(z.any()).optional(),
  allRequiredInputsProvided: z.boolean(),
  activatedAt: z.number().int(),
  isReady: z.boolean(),
  readinessIssues: z.array(z.string()).optional(),
});

export type ActivatedMcpDetails = z.infer<typeof ActivatedMcpDetailsSchema>;
```

### API Route Handler 예시

```typescript
// src/app/api/user/activated-mcps/route.ts

import { auth } from '@/lib/auth';
import { db } from '@/lib/db';
import { decrypt } from '@/lib/crypto';
import { NextResponse } from 'next/server';

export async function GET(request: Request): Promise<Response> {
  const session = await auth();
  if (!session?.user?.id) {
    return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
  }

  try {
    // 1. 활성화된 MCP 조회
    const activatedMCPs = await db.query.userActivatedMCPs.findMany({
      where: eq(userActivatedMCPs.userId, session.user.id),
      with: { mcpServer: true },
    });

    // 2. ActivatedMcpDetails 생성
    const details = await Promise.all(
      activatedMCPs.map(async (activation) => {
        let decryptedEnv = {};
        if (activation.userProvidedEnvEncrypted) {
          const envStr = decrypt(activation.userProvidedEnvEncrypted);
          decryptedEnv = JSON.parse(envStr);
        }

        let transportConfig;

        if (activation.mcpServer.type === 'remote') {
          let authToken;
          if (activation.accessTokenEncrypted) {
            authToken = decrypt(activation.accessTokenEncrypted);
          }

          const now = Date.now();
          const expiresAt = activation.expiresAt?.getTime() ?? null;
          const expiresIn = expiresAt
            ? Math.round((expiresAt - now) / 1000)
            : null;

          transportConfig = {
            transport: activation.mcpServer.transport,
            url: activation.mcpServer.url,
            authToken,
            expiresAt,
            expiresIn,
            requiresRefresh: expiresIn ? expiresIn < 300 : false,
            headers: activation.mcpServer.headers || {},
            customEnv: decryptedEnv,
          };
        } else {
          transportConfig = {
            transport: 'stdio',
            command: activation.mcpServer.command,
            args: (activation.mcpServer.commandArgs || []).map((arg, idx) => {
              const inputVar = activation.mcpServer.inputVars?.find(
                (v) => v.type === 'cmd' && v.argIndex === idx,
              );
              return inputVar && decryptedEnv[inputVar.name]
                ? decryptedEnv[inputVar.name]
                : arg;
            }),
            cwd: activation.mcpServer.cwd,
            env: { ...activation.mcpServer.env, ...decryptedEnv },
          };
        }

        const allRequiredInputsProvided = (activation.mcpServer.inputVars || [])
          .filter((v) => v.required)
          .every((v) => !!decryptedEnv[v.name]);

        const isReady =
          allRequiredInputsProvided &&
          (activation.mcpServer.type === 'local' ||
            !!transportConfig.authToken);

        return {
          id: activation.mcpServer.id,
          name: activation.mcpServer.name,
          description: activation.mcpServer.description,
          logoUrl: activation.mcpServer.logoUrl,
          type: activation.mcpServer.type,
          transport: activation.mcpServer.transport,
          transportConfig,
          protocolVersion: activation.mcpServer.protocolVersion,
          capabilities: activation.mcpServer.capabilities,
          allRequiredInputsProvided,
          activatedAt: activation.createdAt.getTime(),
          isReady,
          readinessIssues: !isReady
            ? [
                !allRequiredInputsProvided && 'Missing required inputs',
                activation.mcpServer.type === 'remote' &&
                  !transportConfig.authToken &&
                  'Missing authentication token',
              ].filter(Boolean)
            : [],
        };
      }),
    );

    return NextResponse.json({
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
    });
  } catch (error) {
    console.error('Error fetching activated MCPs:', error);
    return NextResponse.json(
      { error: 'Internal server error' },
      { status: 500 },
    );
  }
}
```

### React Query Hook 예시

```typescript
// src/hooks/useQueries/useActivatedMcps.ts

import { useQuery } from '@tanstack/react-query';
import { ApiClient } from '@/services/apiClient';
import type { ActivatedMcpsListResponse } from '@/types/api/mcps';
import { queryKeys } from '@/lib/queryKeys';

export function useActivatedMcps() {
  return useQuery({
    queryKey: queryKeys.mcps.activated,
    queryFn: async (): Promise<ActivatedMcpsListResponse> => {
      return ApiClient.get('/api/user/activated-mcps');
    },
    staleTime: 5 * 60 * 1000,
  });
}

export function useActivatedMcpById(mcpId: string) {
  return useQuery({
    queryKey: queryKeys.mcps.activatedDetail(mcpId),
    queryFn: async () => {
      return ApiClient.get(`/api/user/activated-mcps/${mcpId}`);
    },
    enabled: !!mcpId,
  });
}
```

---

## 보안

- DB: AES-256-GCM 암호화
- Server: 복호화 후 응답에 포함
- HTTP: HTTPS 전송 필수
- Client: 메모리 전용 (localStorage 금지)
- 세션 종료 시: 자동 삭제

---

## 성능

| 항목                 | 예상치    |
| -------------------- | --------- |
| 응답 시간 (1-5 MCPs) | 100-200ms |
| 응답 시간 (10+ MCPs) | 200-500ms |
| 응답 크기 (1 MCP)    | 2KB       |
| 캐시 TTL             | 5분       |

---

## 참고

**상세 설계**: `/docs/API_RESPONSE_SCHEMA_FOR_USER_ACTIVATED_MCPs.md`
