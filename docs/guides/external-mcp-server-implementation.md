# LibrAgent External MCP 서버 구현 가이드

> **대상**: 서버 개발팀
> **최종 업데이트**: 2025-07-12
> **MCP Spec**: 2025-06-18 (Streamable HTTP Transport)

---

## 1. 개요

LibrAgent는 [MCP(Model Context Protocol) 2025-06-18](https://modelcontextprotocol.io/specification/2025-06-18) 스펙의 **Streamable HTTP Transport**를 통해 외부 MCP 서버와 통신합니다.

이 가이드는 LibrAgent 클라이언트와 호환되는 HTTP MCP 서버를 구현하는 데 필요한 모든 사항을 다룹니다.

### 1.1 핵심 요약

| 항목               | LibrAgent 동작                  | 서버 구현 요구사항                                 |
| ------------------ | ------------------------------- | -------------------------------------------------- |
| **Transport**      | Streamable HTTP (POST + SSE)    | POST 엔드포인트 필수, SSE(GET) 선택                |
| **Session**        | `Mcp-Session-Id` 헤더 사용      | 서버가 초기화 시 ID 발급, 이후 요청에서 유지       |
| **Stateless**      | `allow_stateless = true`        | 세션 ID 없이 동작하는 stateless 모드도 지원해야 함 |
| **Protocol**       | JSON-RPC 2.0                    | 표준 JSON-RPC 2.0 메시지 포맷                      |
| **Authentication** | `Authorization: Bearer <token>` | OAuth2/Bearer 토큰 검증                            |
| **CORS**           | 데스크톱 앱에서 호출            | CORS 설정 필요 (origin 제한 있음)                  |

---

## 2. 프로토콜 상세

### 2.1 Streamable HTTP Transport

LibrAgent는 MCP 2025-06-18 스펙의 Streamable HTTP Transport를 사용합니다. 두 가지 모드를 모두 지원합니다:

- **Stateful**: `Mcp-Session-Id` 헤더로 세션 상태 관리 (SSE 스트리밍 지원)
- **Stateless**: 헤더 없이 각 요청을 독립적으로 처리

> ⚠️ **중요**: LibrAgent 클라이언트는 `allow_stateless = true`로 설정됩니다. 서버가 `Mcp-Session-Id`를 반환하지 않아도 정상 연결됩니다.

### 2.2 엔드포인트

단일 엔드포인트를 POST와 GET으로 구분하여 사용합니다:

```
POST /mcp   ← JSON-RPC 메시지 전송 (tools/call, tools/list, initialize, ...)
GET  /mcp   ← SSE 스트림 수신 (stateful 모드일 때만)
```

### 2.3 POST 요청 (클라이언트 → 서버)

#### 2.3.1 초기화 (Initialize)

첫 POST 요청은 `initialize` 메서드여야 합니다:

```http
POST /mcp HTTP/1.1
Content-Type: application/json
Authorization: Bearer <token>  ← OAuth 토큰이 있는 경우

{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2025-06-18",
    "capabilities": {
      "roots": { "listChanged": true }
    },
    "clientInfo": {
      "name": "LibrAgent",
      "version": "0.8.x"
    }
  }
}
```

#### 2.3.2 서버 응답 (초기화 + 세션 ID 발급)

서버는 `initialize` 응답에 **`Mcp-Session-Id` 헤더**를 포함해야 합니다:

```http
HTTP/1.1 200 OK
Content-Type: application/json
Mcp-Session-Id: a1b2c3d4-e5f6-7890-abcd-ef1234567890  ← 서버가 생성한 세션 ID

{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "2025-06-18",
    "capabilities": {
      "tools": { "listChanged": true }
    },
    "serverInfo": {
      "name": "MyMCPService",
      "version": "1.0.0"
    }
  }
}
```

> ✅ **핵심**: `Mcp-Session-Id` 헤더는 응답 **헤더**에 포함되며, **응답 바디에는 포함되지 않습니다**.

#### 2.3.3 이후 요청 (세션 유지)

`initialize` 이후의 모든 요청에는 `Mcp-Session-Id` 헤더를 포함해야 합니다:

```http
POST /mcp HTTP/1.1
Content-Type: application/json
Mcp-Session-Id: a1b2c3d4-e5f6-7890-abcd-ef1234567890
Authorization: Bearer <token>

{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/list",
  "params": {}
}
```

#### 2.3.4 도구 실행 (Tool Call)

```http
POST /mcp HTTP/1.1
Content-Type: application/json
Mcp-Session-Id: a1b2c3d4-e5f6-7890-abcd-ef1234567890

{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "search_repos",
    "arguments": {
      "query": "rust mcp",
      "limit": 10
    }
  }
}
```

#### 2.3.5 도구 실행 응답

```http
HTTP/1.1 200 OK
Content-Type: application/json
Mcp-Session-Id: a1b2c3d4-e5f6-7890-abcd-ef1234567890

{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Found 5 repositories..."
      }
    ],
    "isError": false
  }
}
```

### 2.4 GET 요청 (SSE 스트림)

stateful 모드에서 서버는 SSE 스트림을 통해 비동기 결과를 클라이언트에게 보낼 수 있습니다:

```http
GET /mcp HTTP/1.1
Mcp-Session-Id: a1b2c3d4-e5f6-7890-abcd-ef1234567890
Accept: text/event-stream
```

**서버 응답:**

```http
HTTP/1.1 200 OK
Content-Type: text/event-stream
Cache-Control: no-cache
Connection: keep-alive
Mcp-Session-Id: a1b2c3d4-e5f6-7890-abcd-ef1234567890

event: message
data: {"jsonrpc":"2.0","result":{"content":[{"type":"text","text":"Progress..."}],"isError":false}}

event: message
data: {"jsonrpc":"2.0","method":"notifications/message","params":{"level":"info","data":"Done!"}}

event: end
```

> 💡 **선택사항**: SSE 스트리밍은 선택 사항입니다. 동기 응답만으로 충분히 동작합니다.

### 2.5 세션 종료

```http
DELETE /mcp HTTP/1.1
Mcp-Session-Id: a1b2c3d4-e5f6-7890-abcd-ef1234567890
```

응답:

```http
HTTP/1.1 204 No Content
```

---

## 3. 세션 관리

### 3.1 세션 ID 흐름

```
┌──────────────┐                          ┌──────────────┐
│  LibrAgent   │                          │  MCP Server  │
│  (Client)    │                          │              │
└──────┬───────┘                          └──────┬───────┘
       │                                         │
       │  POST /mcp                              │
       │  (초기화 요청, 세션 ID 없음)              │
       │────────────────────────────────────────>│
       │                                         │
       │                     HTTP/1.1 200 OK     │
       │                     Mcp-Session-Id: xxx │
       │<────────────────────────────────────────│
       │                                         │
       │  POST /mcp                              │
       │  Mcp-Session-Id: xxx                    │
       │  tools/list                             │
       │────────────────────────────────────────>│
       │                                         │
       │  HTTP/1.1 200 OK                        │
       │  Mcp-Session-Id: xxx                    │
       │<────────────────────────────────────────│
       │                                         │
       │  POST /mcp                              │
       │  Mcp-Session-Id: xxx                    │
       │  tools/call (search_repos)              │
       │────────────────────────────────────────>│
       │                                         │
       │  HTTP/1.1 200 OK                        │
       │  Mcp-Session-Id: xxx                    │
       │<────────────────────────────────────────│
       │                                         │
```

### 3.2 세션 만료 처리

LibrAgent 클라이언트는 **404 응답**을 세션 만료로 간주하고 **자동 재연결**합니다:

```
1. 클라이언트가 만료된 세션 ID로 요청
2. 서버가 404 Not Found 응답
3. 클라이언트가 연결을 해제하고 새 세션 초기화
4. 새 Mcp-Session-Id 발급 받아 계속 사용
```

**서버 구현 시 필수:**

- 만료된/잘못된 세션 ID에 대해 **404** 반환 (400 아님)
- 세션 만료 로직은 서버가 관리

```http
HTTP/1.1 404 Not Found
Content-Type: application/json

{
  "jsonrpc": "2.0",
  "error": {
    "code": -32000,
    "message": "Session expired"
  }
}
```

### 3.3 stateless 모드

서버가 `Mcp-Session-Id` 헤더를 반환하지 않으면 LibrAgent는 stateless 모드로 동작합니다:

- 각 요청이 독립적으로 처리됨
- SSE 스트리밍 불가
- 세션 상태 유지 불가

> ✅ stateless 모드도 완전히 지원되므로, 초기 구현에는 stateless로 시작해도 좋습니다.

---

## 4. 인증

### 4.1 OAuth2 / Bearer 토큰

LibrAgent에서 OAuth 인증이 구성된 MCP 서버에는 `Authorization` 헤더가 자동으로 붙습니다:

```http
Authorization: Bearer eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9...
```

### 4.2 토큰 검증

서버에서는 다음과 같이 토큰을 검증해야 합니다:

```python
from functools import wraps
from flask import request, jsonify
import jwt

def require_auth(f):
    @wraps(f)
    def decorated(*args, **kwargs):
        auth_header = request.headers.get('Authorization')
        if not auth_header:
            return jsonify({"error": "Authorization header required"}), 401

        parts = auth_header.split()
        if len(parts) != 2 or parts[0].lower() != 'bearer':
            return jsonify({"error": "Invalid authorization format"}), 401

        token = parts[1]
        try:
            # 토큰 검증 로직 (예: JWT decoding, OAuth introspection 등)
            payload = jwt.decode(token, secret_key, algorithms=["RS256"])
            request.user = payload
        except jwt.ExpiredSignatureError:
            return jsonify({"error": "Token expired"}), 401
        except jwt.InvalidTokenError:
            return jsonify({"error": "Invalid token"}), 401

        return f(*args, **kwargs)
    return decorated
```

---

## 5. JSON-RPC 2.0 메시지 포맷

### 5.1 필수 필드

모든 JSON-RPC 메시지는 다음 필드를 포함해야 합니다:

| 필드      | 타입                     | 설명                                                       |
| --------- | ------------------------ | ---------------------------------------------------------- |
| `jsonrpc` | string                   | 반드시 `"2.0"`                                             |
| `id`      | string \| number \| null | 요청-응답 매칭용 (notification은 null)                     |
| `method`  | string                   | 메서드 이름 (`initialize`, `tools/list`, `tools/call`, 등) |
| `params`  | object \| array \| null  | 메서드 파라미터                                            |

### 5.2 지원되는 메서드

| 메서드           | 설명                    | 요청 params                                     | 응답 result                                     |
| ---------------- | ----------------------- | ----------------------------------------------- | ----------------------------------------------- |
| `initialize`     | 연결 초기화             | `protocolVersion`, `capabilities`, `clientInfo` | `protocolVersion`, `capabilities`, `serverInfo` |
| `initialized`    | 초기화 완료 알림        | 없음                                            | 없음 (notification)                             |
| `tools/list`     | 사용 가능한 도구 목록   | `{}`                                            | `tools`: 도구 배열                              |
| `tools/call`     | 도구 실행               | `name`, `arguments`                             | `content`, `isError`                            |
| `resources/list` | 사용 가능한 리소스 목록 | `{}`                                            | `resources`: 리소스 배열                        |
| `resources/read` | 리소스 읽기             | `uri`                                           | `contents`: 리소스 배열                         |

### 5.3 도구 응답 형식

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "검색 결과:\n1. repo-a - A description\n2. repo-b - B description"
      },
      {
        "type": "image",
        "data": "<base64-encoded-image>",
        "mimeType": "image/png"
      }
    ],
    "isError": false
  }
}
```

**content 항목 타입:**

| type       | 필수 필드                            | 설명          |
| ---------- | ------------------------------------ | ------------- |
| `text`     | `text` (string)                      | 텍스트 콘텐츠 |
| `image`    | `data` (base64), `mimeType` (string) | 이미지 데이터 |
| `resource` | `resource` (object)                  | 리소스 참조   |

### 5.4 에러 응답

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "error": {
    "code": -32600,
    "message": "Invalid Request",
    "data": {
      "details": "Tool not found: search_users"
    }
  }
}
```

**표준 에러 코드:**

| 코드     | 의미                        |
| -------- | --------------------------- |
| `-32600` | Invalid Request             |
| `-32601` | Method Not Found            |
| `-32602` | Invalid Params              |
| `-32603` | Internal Error              |
| `-32000` | Server Error (세션 만료 등) |

---

## 6. 완전한 예제 구현

### 6.1 Python (FastAPI) - stateful 모드

```python
"""
LibrAgent 호환 MCP 서버 예제 (FastAPI + stateful 세션 관리)
"""
from fastapi import FastAPI, Request, HTTPException
from fastapi.responses import StreamingResponse, JSONResponse
import uuid
import json
import asyncio
from typing import Dict, Any
from datetime import datetime, timedelta

app = FastAPI(title="My MCP Server")

# 세션 저장소 (실제 운영에서는 Redis 등 사용)
sessions: Dict[str, dict] = {}
SESSION_TTL = timedelta(hours=1)


def cleanup_expired_sessions():
    """만료된 세션 정리"""
    now = datetime.utcnow()
    expired = [
        sid for sid, s in sessions.items()
        if now - s["created_at"] > SESSION_TTL
    ]
    for sid in expired:
        del sessions[sid]


@app.post("/mcp")
async def handle_mcp_request(request: Request):
    """JSON-RPC 메시지 처리 (POST)"""
    # 세션 ID 확인
    session_id = request.headers.get("Mcp-Session-Id")

    # 본문 읽기
    body = await request.body()
    message = json.loads(body.decode("utf-8"))

    # 초기화 요청
    if message.get("method") == "initialize":
        new_session_id = str(uuid.uuid4())
        sessions[new_session_id] = {
            "created_at": datetime.utcnow(),
            "last_active": datetime.utcnow(),
        }

        response = {
            "jsonrpc": "2.0",
            "id": message["id"],
            "result": {
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "tools": {"listChanged": True},
                },
                "serverInfo": {
                    "name": "MyMCPService",
                    "version": "1.0.0",
                },
            },
        }

        resp = JSONResponse(content=response)
        resp.headers["Mcp-Session-Id"] = new_session_id
        return resp

    # 인증 체크 (초기화 이후 요청)
    if not session_id or session_id not in sessions:
        return JSONResponse(
            status_code=404,
            content={
                "jsonrpc": "2.0",
                "error": {
                    "code": -32000,
                    "message": "Session expired",
                },
            },
        )

    # 세션 활성 시간 업데이트
    sessions[session_id]["last_active"] = datetime.utcnow()

    # 메서드 라우팅
    method = message.get("method")

    if method == "tools/list":
        return handle_tools_list(message, session_id)
    elif method == "tools/call":
        return await handle_tools_call(message, session_id)
    else:
        return JSONResponse(
            status_code=400,
            content={
                "jsonrpc": "2.0",
                "error": {
                    "code": -32601,
                    "message": f"Method not found: {method}",
                },
            },
        )


def handle_tools_list(message: dict, session_id: str):
    """도구 목록 반환"""
    return JSONResponse(content={
        "jsonrpc": "2.0",
        "id": message["id"],
        "result": {
            "tools": [
                {
                    "name": "search_repos",
                    "description": "GitHub 저장소 검색",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "검색어",
                            },
                            "limit": {
                                "type": "integer",
                                "description": "결과 제한",
                                "default": 10,
                            },
                        },
                        "required": ["query"],
                    },
                },
                {
                    "name": "get_user",
                    "description": "GitHub 사용자 정보 조회",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "username": {
                                "type": "string",
                                "description": "GitHub 사용자명",
                            },
                        },
                        "required": ["username"],
                    },
                },
            ]
        },
    })


async def handle_tools_call(message: dict, session_id: str):
    """도구 실행"""
    params = message.get("params", {})
    tool_name = params.get("name", "")
    arguments = params.get("arguments", {})

    try:
        if tool_name == "search_repos":
            result = await search_repos_impl(arguments)
        elif tool_name == "get_user":
            result = await get_user_impl(arguments)
        else:
            return JSONResponse(content={
                "jsonrpc": "2.0",
                "id": message["id"],
                "error": {
                    "code": -32602,
                    "message": f"Unknown tool: {tool_name}",
                },
            })

        return JSONResponse(content={
            "jsonrpc": "2.0",
            "id": message["id"],
            "result": {
                "content": [
                    {
                        "type": "text",
                        "text": result,
                    }
                ],
                "isError": False,
            },
        })

    except Exception as e:
        return JSONResponse(content={
            "jsonrpc": "2.0",
            "id": message["id"],
            "error": {
                "code": -32603,
                "message": f"Internal error: {str(e)}",
            },
        })


async def search_repos_impl(args: dict) -> str:
    """실제 검색 로직 (예제)"""
    query = args.get("query", "")
    limit = args.get("limit", 10)
    # 실제 API 호출 구현
    repos = [
        {"name": f"repo-{i}", "description": f"Description for repo-{i}"}
        for i in range(limit)
    ]
    return f"Found {len(repos)} repositories:\n" + "\n".join(
        f"- {r['name']}: {r['description']}" for r in repos
    )


async def get_user_impl(args: dict) -> str:
    """사용자 정보 조회 (예제)"""
    username = args.get("username", "")
    return f"User: {username}\nFollowers: 123\nRepos: 45"


@app.get("/mcp")
async def handle_sse_stream(request: Request):
    """SSE 스트림 (stateful 모드 선택사항)"""
    session_id = request.headers.get("Mcp-Session-Id")

    if not session_id or session_id not in sessions:
        return JSONResponse(
            status_code=404,
            content={"error": "Session not found"},
        )

    async def event_stream():
        """SSE 이벤트 스트림"""
        while session_id in sessions:
            yield f"event: message\ndata: {{\"jsonrpc\":\"2.0\",\"method\":\"notifications/message\",\"params\":{\"level\":\"info\",\"data\":\"Server is running\"}}}\n\n"
            await asyncio.sleep(30)

    return StreamingResponse(
        event_stream(),
        media_type="text/event-stream",
        headers={"Mcp-Session-Id": session_id},
    )


@app.delete("/mcp")
async def handle_session_delete(request: Request):
    """세션 종료"""
    session_id = request.headers.get("Mcp-Session-Id")
    if session_id and session_id in sessions:
        del sessions[session_id]
    return JSONResponse(status_code=204)
```

### 6.2 Node.js (Express) - stateless 모드

```typescript
/**
 * LibrAgent 호환 MCP 서버 예제 (Express + stateless)
 */
import express, { Request, Response, NextFunction } from 'express';
import { v4 as uuidv4 } from 'uuid';

const app = express();
app.use(express.json());

// CORS 설정 (LibrAgent 데스크톱 앱에서 호출)
app.use((req: Request, _res: Response, next: NextFunction) => {
  res.header('Access-Control-Allow-Origin', '*');
  res.header('Access-Control-Allow-Methods', 'GET, POST, DELETE, OPTIONS');
  res.header(
    'Access-Control-Allow-Headers',
    'Content-Type, Authorization, Mcp-Session-Id',
  );
  if (req.method === 'OPTIONS') {
    return res.sendStatus(204);
  }
  next();
});

// 토큰 검증 미들웨어
function requireAuth(req: Request, res: Response, next: NextFunction) {
  const auth = req.headers.authorization;
  if (!auth || !auth.startsWith('Bearer ')) {
    return res.status(401).json({
      jsonrpc: '2.0',
      error: { code: -32000, message: 'Authorization required' },
    });
  }
  next();
}

let messageId = 0;

app.post('/mcp', requireAuth, async (req: Request, res: Response) => {
  const message = req.body;

  // 초기화 요청
  if (message.method === 'initialize') {
    const response = {
      jsonrpc: '2.0' as const,
      id: message.id,
      result: {
        protocolVersion: '2025-06-18',
        capabilities: {
          tools: { listChanged: true },
        },
        serverInfo: {
          name: 'MyMCPService',
          version: '1.0.0',
        },
      },
    };

    // stateless 모드: Mcp-Session-Id 헤더 생략
    return res.json(response);
  }

  const method = message.method;

  if (method === 'tools/list') {
    return res.json({
      jsonrpc: '2.0',
      id: message.id,
      result: {
        tools: [
          {
            name: 'search_repos',
            description: 'GitHub 저장소 검색',
            inputSchema: {
              type: 'object' as const,
              properties: {
                query: { type: 'string', description: '검색어' },
                limit: {
                  type: 'integer',
                  description: '결과 제한',
                  default: 10,
                },
              },
              required: ['query'],
            },
          },
        ],
      },
    });
  }

  if (method === 'tools/call') {
    const { name, arguments: args } = message.params || {};

    if (name === 'search_repos') {
      const query = args?.query || '';
      const limit = args?.limit || 10;

      return res.json({
        jsonrpc: '2.0',
        id: message.id,
        result: {
          content: [
            {
              type: 'text' as const,
              text: `Found ${limit} repositories for "${query}"`,
            },
          ],
          isError: false,
        },
      });
    }

    return res.json({
      jsonrpc: '2.0',
      id: message.id,
      error: {
        code: -32602,
        message: `Unknown tool: ${name}`,
      },
    });
  }

  return res.json({
    jsonrpc: '2.0',
    error: {
      code: -32601,
      message: `Method not found: ${method}`,
    },
  });
});

app.listen(3000, () => {
  console.log('MCP Server running on http://localhost:3000');
});
```

---

## 7. LibrAgent 클라이언트 동작 상세

### 7.1 연결 프로세스

```
1. client가 StreamableHttpClientTransport 생성
   - allow_stateless = true (항상 설정)
   - URL, custom headers 설정

2. ().serve(transport).await 호출
   → initialize 요청 전송
   → 서버가 Mcp-Session-Id 반환 시 세션 모드
   → 반환 안 하면 stateless 모드

3. 연결 성공 시 MCPConnection 저장
   → tools/list, tools/call 사용 가능
```

### 7.2 세션 재연결

```
1. 클라이언트가 Mcp-Session-Id와 함께 요청
2. 서버가 404 응답
3. 클라이언트:
   a. 기존 연결 해제
   b. 새 연결 시작 (새 initialize)
   c. 새 Mcp-Session-Id 받음
   d. 원래 tool call 재시도 (1회만)
```

### 7.3 도구 호출 네이밍

외부 MCP 서버의 도구는 LibrAgent 내에서 다음 패턴으로 호출됩니다:

```
{server_name}__{tool_name}
```

예:

- 서버 이름: `github-api`
- 도구 이름: `search_repos`
- LibrAgent 내 호출: `github-api__search_repos`

> ⚠️ 서버는 이 네이밍 규칙을 알고 있을 필요 없습니다. 클라이언트가 자동으로 변환합니다.

### 7.4 도구 목록 캐싱

LibrAgent는 서버 연결 시 `tools/list`를 호출하여 도구 목록을 캐시합니다. 이후 세션 동안 캐시된 목록을 사용합니다.

---

## 8. 테스트

### 8.1 curl 테스트

#### 초기화 테스트

```bash
curl -X POST http://localhost:3000/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
      "protocolVersion": "2025-06-18",
      "clientInfo": { "name": "test-client", "version": "1.0" }
    }
  }' -v
```

**예상 응답:**

- 상태 코드: `200 OK`
- 헤더에 `Mcp-Session-Id` 포함 (stateful 모드)
- 바디에 `initialize` 결과 포함

#### 도구 목록 테스트

```bash
# 초기화 응답에서 Mcp-Session-Id 추출
curl -X POST http://localhost:3000/mcp \
  -H "Content-Type: application/json" \
  -H "Mcp-Session-Id: <extracted-session-id>" \
  -d '{
    "jsonrpc": "2.0",
    "id": 2,
    "method": "tools/list",
    "params": {}
  }'
```

#### 도구 실행 테스트

```bash
curl -X POST http://localhost:3000/mcp \
  -H "Content-Type: application/json" \
  -H "Mcp-Session-Id: <extracted-session-id>" \
  -d '{
    "jsonrpc": "2.0",
    "id": 3,
    "method": "tools/call",
    "params": {
      "name": "search_repos",
      "arguments": {
        "query": "rust",
        "limit": 5
      }
    }
  }'
```

#### 세션 만료 테스트

```bash
# 유효하지 않은 세션 ID로 요청
curl -X POST http://localhost:3000/mcp \
  -H "Content-Type: application/json" \
  -H "Mcp-Session-Id: invalid-session-id" \
  -d '{
    "jsonrpc": "2.0",
    "id": 4,
    "method": "tools/list",
    "params": {}
  }'
```

**예상 응답:**

- 상태 코드: `404 Not Found`
- 바디에 에러 메시지 포함

#### 인증 테스트

```bash
curl -X POST http://localhost:3000/mcp \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer invalid-token" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {}
  }'
```

#### SSE 스트림 테스트 (선택사항)

```bash
curl -N -H "Mcp-Session-Id: <session-id>" \
     http://localhost:3000/mcp
```

### 8.2 LibrAgent에서 직접 테스트

1. LibrAgent 설정에서 MCP 서버 등록
2. `transport.type: "http"` 설정
3. `transport.url: "http://localhost:3000/mcp"` 설정
4. 에이전트 세션 시작 → 도구 목록이 보이는지 확인
5. 도구 호출이 정상 동작하는지 확인

---

## 9. 체크리스트

서버 구현 완료 후 다음 항목을 확인하세요:

### 필수 항목

- [ ] POST `/mcp` 엔드포인트 구현됨
- [ ] `initialize` 메서드 처리
- [ ] `tools/list` 메서드 처리
- [ ] `tools/call` 메서드 처리
- [ ] JSON-RPC 2.0 포맷 준수
- [ ] `Mcp-Session-Id` 헤더 반환 (stateful 모드)
- [ ] 만료된 세션에 404 반환
- [ ] content 항목에 `type: "text"` 포함
- [ ] 도구 응답에 `isError: false` 포함

### 선택 항목

- [ ] GET `/mcp` SSE 스트림 구현
- [ ] DELETE `/mcp` 세션 종료
- [ ] OAuth2/Bearer 토큰 검증
- [ ] CORS 설정
- [ ] `resources/list` 및 `resources/read` 구현
- [ ] 에러 코드 표준 준수

### LibrAgent 특화

- [ ] `allow_stateless` 호환 (Mcp-Session-Id 없어도 동작)
- [ ] `Authorization` 헤더 수신
- [ ] 도구 이름에 `__` 구분자 사용 (클라이언트 측 자동 변환)

---

## 10.常见问题 (FAQ)

### Q1. `Mcp-Session-Id` 헤더를 반환해야 하나요?

**A**: stateful 모드를 원한다면 필수입니다. stateless 모드도 지원되므로, 초기 구현에서는 생략해도 LibrAgent와 호환됩니다.

### Q2. 400 Bad Request 대신 404를 반환해야 하나요?

**A**: 세션 관련 오류(만료, 없음)에는 **404**를 반환해야 합니다. LibrAgent는 404를 세션 만료로 간주하고 자동 재연결합니다. 400은 다른 에러(잘못된 파라미터 등)에 사용하세요.

### Q3. SSE 없이 동작할 수 있나요?

**A**: 네, 완전히 가능합니다. 동기 POST 응답만으로 모든 도구 호출이 동작합니다. SSE는 비동기/스트리밍 결과에OPTIONAL입니다.

### Q4. 여러 에이전트 세션이 동시에 연결할 수 있나요?

**A**: 네. 각 LibrAgent 에이전트 세션은 독립적인 `Mcp-Session-Id`를 사용하므로 서버는 세션별로 상태를 분리해야 합니다.

### Q5. 토큰 없이 동작할 수 있나요?

**A**: 네. 인증은 선택 사항입니다. `Authorization` 헤더가 없으면 토큰 검증을 건너뜁니다.

---

## 11. 참고 자료

- [MCP Specification 2025-06-18](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports)
- [RMCP Library (Rust)](https://docs.rs/rmcp/latest/rmcp/)
- [LibrAgent External MCP Architecture](./external-mcp-integration.md)

---

**문의**: 서버 구현 중 질문이 있다면 이슈 트래커 또는 개발 채널에서 문의하세요.
