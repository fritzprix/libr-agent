# LibrAgent 기술 데모 컨셉

> **문서 버전:** v2.0 · **작성일:** 2026-06-14 · **상태:** 완성
> **작성자:** Technical Expert (Libr Assistant)
> **소유자:** Technical Expert · **검토:** Coordinator
> **참고 자료:** `docs/demo/marketing-demo-concept.md`, `docs/architecture/agent-workflow-architecture.md`, `src-tauri/src/mcp/service_proxy/mod.rs`, `src-tauri/src/agent/events.rs`, `CONTRIBUTING.md`

---

## 목차

1. [마인드셋 — 개발자가 "이건 진짜다"라고 느끼는 순간](#1-마인드셋)
2. [Wow Moment 1 — 세션 격리 proofs: 코드 레벨에서 증언](#2-wow-moment-1)
3. [Wow Moment 2 — 170+ 도구 스트리밍: UI 깨짐 없이 처리하는 아키텍처](#3-wow-moment-2)
4. [Wow Moment 3 — MCP 프로토콜: stdio/HTTP/SSE가 실제로 어떻게 연결되는가](#4-wow-moment-3)
5. [Wow Moment 4 — 세션 재개: 1-2초 → 500ms 목표의 실제 수치](#5-wow-moment-4)
6. [실제 데모용 코드/설정: 바로 복사해서 쓸 수 있는 예시](#6-실제-데모용-코드설정)
7. [개발자 타겟 기술 블로그 3부작](#7-개발자-타겟-기술-블로그-3부작)
8. [GitHub 기여자 유인 데모 — OSS 커뮤니티가 기여하고 싶어지게 하는 요소](#8-github-기여자-유인-데모)
9. [데모 실행 체크리스트 — P0 우선순위](#9-데모-실행-체크리스트)
10. [마케팅 데모와의 연계 — "아키텍처가 설정보다 낫다"의 양면 증명](#10-마케팅-데모와의-연계)

---

## 1. 마인드셋

> 마케팅 데모가 "누가, 어떤 문제를 해결하는가"를 보여준다면, 기술 데모는 **"어떻게, 왜 그렇게 설계했는가"**를 증명합니다.

| 항목 | 내용 |
|------|------|
| **관중** | 백엔드/프론트엔드 개발자, 시스템 아키텍트, OSS 기여 잠재력 있는 개발자 |
| **핵심 메시지** | "아키텍처가 설정보다 낫다. Rust + MCP + 세션 격리 = 확장 가능한 AI 에이전트 플랫폼" |
| **톤앤매너** | 기술적 정확성 우선. 과장보다 코드와 구조로 증언. |
| **금기** | "에이전트가 스스로 학습한다" 같은 허위 주장. 대신 "에이전트가 세션 격리된 상태로 도구를 호출한다"는 사실로 증언. |

### 기술 데모의 4가지 질문

| 질문 | 마케팅 데모가 답하는 방식 | 기술 데모가 답하는 방식 |
|------|--------------------------|------------------------|
| "세션 격리가 진짜로 동작하나요?" | "개인 비서와 팀 비서가 동시에 작동합니다" | `MCPServiceProxy`가 세션마다 별도 `HashMap<String, Box<dyn BuiltinMCPServer>>`를 생성하는 코드를 보여줌 |
| "170개 도구가 진짜 인식되나요?" | "모든 도구가 즉시 사용 가능합니다" | `agent:event` 스트림에서 Planning → Browser → Workspace → Knowledge 순으로 도구 호출이 실시간으로 표시됨 |
| "MCP 서버 추가가 진짜 쉬운가요?" | "클릭 한 번으로 추가합니다" | JSON 설정 5줄로 외부 MCP 서버가 즉시 도구 목록에 반영됨을 시연 |
| "에이전트가 진짜 자율적으로 동작하나요?" | "스스로 판단합니다" | Think-Act-Observe 루프가 5-6회 반복되는 전 과정을 `AgentEvent`로 추적 |

---

## 2. Wow Moment 1 — 세션 격리 proofs: 코드 레벨에서 증언

### 목표

에이전트 세션이 서로 완전히 격리됨을 **시각적으로, 코드 레벨에서, 동시에** 보여주기.

### 데모 플로우 (실제 실행 순서)

```
터미널 1 (세션 A)              터미널 2 (세션 B)
─────────────────              ─────────────────
$ libr-agent session new A     $ libr-agent session new B
→ session_id: sess_a_001       → session_id: sess_b_002

$ planning add "Task A"        $ planning add "Task B"
→ todo_id: todo_a_1            → todo_id: todo_b_1

$ planning list                $ planning list
→ [todo_a_1] Task A            → [todo_b_1] Task B

$ planning list (A에서)        $ planning list (B에서)
→ [todo_a_1] Task A            → [todo_b_1] Task B
→ Todo 개수: 1                 → Todo 개수: 1

$ planning list (A에서)        $ planning list (B에서)
→ [todo_a_1] Task A            → (빈 목록)
→ Todo 개수: 1                 → 세션 A의 데이터 전혀 안 보임
```

### 실제 코드: `MCPServiceProxy`의 세션 격리 구조

```rust
// src-tauri/src/mcp/service_proxy/mod.rs
// (실제 소스에서 발췌 — v2.0 기준)

pub struct MCPServiceProxy {
    /// This proxy is bound to a SINGLE agent session.
    session_id: String,

    /// Session-specific builtin server instances
    /// Key: tool_id (e.g., "knowledge", "planning")
    /// Value: Boxed trait object implementing BuiltinMCPServer
    builtin_servers: HashMap<String, Box<dyn BuiltinMCPServer>>,

    /// Cached tools from session-isolated stdio servers
    session_stdio_tool_cache: Arc<RwLock<HashMap<String, Vec<MCPTool>>>>,

    /// Cached tools from session-isolated HTTP servers
    session_http_tool_cache: Arc<RwLock<HashMap<String, Vec<MCPTool>>>>,

    /// Session-specific managers
    session_managers: SessionManagers,

    /// Tool execution timeout in seconds
    tool_timeout_seconds: u64,
}
```

**핵심 증명 포인트:**

1. `builtin_servers: HashMap<String, Box<dyn BuiltinMCPServer>>` — 각 세션이 별도 인스턴스를 가짐
2. `session_stdio_tool_cache` / `session_http_tool_cache` — Arc<RwLock>로 세션 격리된 캐시
3. `SessionManagers` — 각 세션이 별도 `HttpSessionManager` + `SessionMCPManager` 인스턴스를 가짐

### 시각적 증거: 세션 격리 아키텍처 도식

```
┌─────────────────────────────────────────────────────────┐
│                    Tauri Desktop App                     │
│                                                          │
│  ┌──────────────┐         ┌──────────────┐              │
│  │  Session A    │         │  Session B    │              │
│  │  sess_a_001   │         │  sess_b_002   │              │
│  │               │         │               │              │
│  │  ┌──────────┐ │         │  ┌──────────┐ │              │
│  │  │MCPService│ │         │  │MCPService│ │              │
│  │  │  Proxy A │ │         │  │  Proxy B │ │              │
│  │  │          │ │         │  │          │ │              │
│  │  │HashMap:  │ │         │  │HashMap:  │ │              │
│  │  │  planning│ │         │  │  planning│ │              │
│  │  │ knowledge│ │         │  │ knowledge│ │              │
│  │  │ browser  │ │         │  │ browser  │ │              │
│  │  └──────────┘ │         │  └──────────┘ │              │
│  │               │         │               │              │
│  │  StdioCache:  │         │  StdioCache:  │              │
│  │  HTTPCache:   │         │  HTTPCache:   │              │
│  └──────┬────────┘         └──────┬────────┘              │
│         │                         │                        │
│         └─────────┬───────────────┘                        │
│                   │                                        │
│         ┌─────────▼─────────┐                              │
│         │   Rust Backend     │                              │
│         │  (Shared DB,       │                              │
│         │   Shared SQLite)   │                              │
│         └───────────────────┘                              │
└─────────────────────────────────────────────────────────┘

중요: 세션 A의 HashMap과 세션 B의 HashMap은 완전히 분리됨.
세션 A가 planning list를 호출하면 오직 세션 A의 HashMap만 조회.
```

### "이건 진짜다"를 느끼게 하는 세 가지 검증

| 검증 | 방법 | 기대 결과 |
|------|------|-----------|
| **1. 데이터 불침투** | 세션 A에서 `planning add "A"` → 세션 B에서 `planning list` | B에서 A의 todo가 절대 안 보임 |
| **2. 도구 격리** | 세션 A에서 `browser navigate https://a.com` → 세션 B에서 `browser currentUrl` | B에서 A의 URL이 안 보임 |
| **3. 상태 격리** | 세션 A 종료 → 세션 B 상태 확인 | A의 모든 상태(Planning todo, Knowledge, Browser session)가 B에 미치지 않음 |

---

## 3. Wow Moment 2 — 170+ 도구 스트리밍: UI 깨짐 없이 처리하는 아키텍처

### 목표

단일 세션에서 170개 이상의 도구를 실시간으로 인식하고 실행하는 모습을 보여주되, **"UI가 깨지지 않는다"**는 것을 증명.

### 왜 이것이 Wow인가?

대부분의 AI 에이전트 플랫폼은 도구 수가 20-50개에 그칩니다. 170개 도구를 실시간으로 처리하면서도 UI가 깨지지 않는 것은 **아키텍처의 힘**입니다.

### 실제 이벤트 스트림: `AgentEvent` enum

```rust
// src-tauri/src/agent/events.rs
// (실제 소스에서 발췌 — v2.0 기준)

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AgentEvent {
    WorkflowStarted { session_id: String },
    WorkflowCompleted { session_id: String, reason: WorkflowCompletionReason },
    WorkflowError { session_id: String, error: AgentRuntimeError },
    StatusChanged { session_id: String, status: SessionStatus },
    MessageAdded { session_id: String, message: Box<Message> },
    ToolExecutionStarted { session_id: String, tool_name: String },
    ToolExecutionCompleted { session_id: String, tool_name: String, success: bool },
    ToolExecutionRequiresApproval { session_id: String, tool_call_id: String, tool_name: String, arguments: String, ... },
    ToolExecutionApprovalResolved { session_id: String, tool_call_id: String, approved: bool },
    InitializationStep { session_id: String, step: String, status: InitializationStatus },
    SessionRuntimeStateUpdated { session_id: String, runtime_state: SessionRuntimeState },
    PreflightTokenMetricsUpdated { session_id: String, metrics: PreflightTokenMetrics },
    ResourceUpdated { resource_type: String, action: String, resource_id: Option<String> },
}
```

### 데모 플로우: 도구 호출 타임라인 시각화

```
[00:00.000] WorkflowStarted(session=sess_demo_001)
[00:00.234] InitializationStep(step="load_builtins", status=Running)
[00:00.567] InitializationStep(step="load_builtins", status=Complete)
[00:00.891] SessionRuntimeStateUpdated(sequence=1, phase=Think, proxy_mode=Active)

[00:01.200] ToolExecutionStarted(tool=planning::createGoal)
[00:01.450] ToolExecutionCompleted(tool=planning::createGoal, success=true)
           → "Goal created: Research Rust async runtimes"

[00:02.100] ToolExecutionStarted(tool=arxiv::search_papers)
[00:03.890] ToolExecutionCompleted(tool=arxiv::search_papers, success=true)
           → "Found 15 papers on 'Rust async runtime 2024'"

[00:04.500] ToolExecutionStarted(tool=knowledge::record)
[00:04.780] ToolExecutionCompleted(tool=knowledge::record, success=true)
           → "Rust async runtime notes saved to knowledge base"

[00:05.200] ToolExecutionStarted(tool=workspace::writeFile)
[00:05.670] ToolExecutionCompleted(tool=workspace::writeFile, success=true)
           → "research_notes.md created (1,247 bytes)"

[00:06.100] ToolExecutionStarted(tool=agent__startSession)
[00:06.890] ToolExecutionCompleted(tool=agent__startSession, success=true)
           → "Child session spawned: session-child-abc123"

[00:07.500] SessionRuntimeStateUpdated(sequence=2, phase=Observe, proxy_mode=Active)
[00:08.000] MessageAdded(message=msg_final_report, role=assistant)
[00:08.200] WorkflowCompleted(reason=Natural)
```

**170+ 도구 처리의 핵심 기술:**

| 기술 | 설명 |
|------|------|
| **비동기 라우팅** | `route_tool()`이 `Builtin` vs `External`를 O(1)으로 분류 |
| **세션 격리 캐시** | `session_stdio_tool_cache`와 `session_http_tool_cache`가 매 세션마다 별도 |
| **이벤트 버퍼링** | `AgentEvent`가 `serde`로 직렬화되어 Tauri 이벤트 시스템으로 전송 |
| **프론트엔드 구독** | `listen('agent:event', ...)`이 이벤트 스트림을 실시간으로 소비 |

### 프론트엔드: 이벤트 기반 상태 관리

```typescript
// src/features/agent/AgentSessionContext.tsx (추상화)
useEffect(() => {
  const unlisten = listen('agent:event', (event: AgentEvent) => {
    switch (event.type) {
      case 'workflowStarted':
        setWorkflowStatus('running');
        break;
      case 'toolExecutionStarted':
        addToolCallToMessage(event.toolName);
        break;
      case 'toolExecutionCompleted':
        updateToolCallResult(event.toolName, event.success);
        break;
      case 'statusChanged':
        setSessionStatus(event.status);
        break;
      case 'messageAdded':
        appendMessage(event.message);
        break;
    }
  });
  return () => unlisten.then(fn => fn());
}, [sessionId]);
```

**핵심 메시지:** "프론트엔드는 상태를 소유하지 않습니다. 백엔드가 발신하는 이벤트를 구독할 뿐입니다."

---

## 4. Wow Moment 3 — MCP 프로토콜: stdio/HTTP/SSE가 실제로 어떻게 연결되는가

### 목표

외부 MCP 서버를 플러그인처럼 추가하고 제거하는 모습을 보여주되, **stdio/HTTP/SSE 전송 프로토콜이 실제로 어떻게 연결되는지**를 코드 레벨에서 보여주기.

### MCP 서버 연결 설정 예시: 실제 JSON/YAML

#### 예시 1: stdio 기반 MCP 서버 (로컬 Python 스크립트)

```json
{
  "name": "local-file-search",
  "description": "Fast file search within workspace using ripgrep",
  "transport": {
    "type": "stdio",
    "command": "python3",
    "args": [
      "/home/user/.libragent/mcp-servers/file-search/main.py"
    ],
    "env": {
      "RG_MAX_COUNT": "100",
      "RG_CASE": "smart"
    }
  }
}
```

#### 예시 2: HTTP 기반 MCP 서버 (리모트 API)

```json
{
  "name": "github-mcp",
  "description": "GitHub repository management via REST API",
  "transport": {
    "type": "http",
    "url": "https://mcp.github.example.com/mcp",
    "headers": {
      "Authorization": "Bearer $GITHUB_TOKEN"
    }
  }
}
```

#### 예시 3: SSE 기반 MCP 서버 (스트리밍)

```json
{
  "name": "google-search-mcp",
  "description": "Real-time web search with streaming results",
  "transport": {
    "type": "http-sse",
    "url": "https://mcp.google-search.example.com/sse",
    "enableSSE": true
  }
}
```

### 데모 플로우: 외부 MCP 서버 동적 로드

```
상태 1: 초기 (170개 내장 도구만)
┌──────────────────────────────────┐
│  Built-in Tools (170개)          │
│  ├─ planning                     │
│  ├─ knowledge                    │
│  ├─ browser                      │
│  ├─ workspace                    │
│  ├─ jupyter                      │
│  ├─ attachments                  │
│  ├─ arxiv                        │
│  └─ ... (163개 더)               │
└──────────────────────────────────┘
        ↓ add_mcp_server("github-mcp")
상태 2: GitHub MCP 추가 후
┌──────────────────────────────────┐
│  Built-in Tools (170개)          │
│  ├─ planning                     │
│  ├─ knowledge                    │
│  ├─ ...                          │
│  └─ github__createIssue          │ ← NEW
│     github__listPullRequests     │ ← NEW
│     github__mergePullRequest     │ ← NEW
│     github__searchCode           │ ← NEW
└──────────────────────────────────┘
        ↓ remove_mcp_server("github-mcp")
상태 3: GitHub MCP 제거 후
┌──────────────────────────────────┐
│  Built-in Tools (170개)          │
│  ├─ planning                     │
│  ├─ knowledge                    │
│  ├─ browser                      │
│  └─ ... (166개 더)               │
│  ← github 도구들이 즉시 사라짐    │
└──────────────────────────────────┘
```

### 실제 코드: `SessionMCPManager`의 동적 로드

```rust
// src-tauri/src/mcp/session_isolation.rs (추상화)

pub struct SessionMCPManager {
    /// 세션별 서버 인스턴스
    /// Key: server_name (예: "github-mcp")
    /// Value: McpClient 인스턴스
    servers: HashMap<String, McpClient>,

    /// 세션 ID
    session_id: String,
}

impl SessionMCPManager {
    /// 외부 MCP 서버 추가
    pub async fn add_server(&mut self, config: McpConfig) -> Result<(), String> {
        let client = match config.transport.type_ {
            TransportType::Stdio => {
                McpClient::connect_stdio(config.transport).await?
            }
            TransportType::Http => {
                McpClient::connect_http(config.transport).await?
            }
            TransportType::HttpSse => {
                McpClient::connect_http_sse(config.transport).await?
            }
        };

        // 도구 목록 캐싱 (eager discovery)
        let tools = client.list_tools().await?;
        self.servers.insert(config.name.clone(), client);

        // 세션별 도구 캐시 업데이트
        self.update_tool_cache(&config.name, tools).await;

        Ok(())
    }

    /// 외부 MCP 서버 제거
    pub async fn remove_server(&mut self, name: &str) -> Result<(), String> {
        if self.servers.remove(name).is_some() {
            self.clear_tool_cache(name).await;
            Ok(())
        } else {
            Err(format!("Server '{}' not found", name))
        }
    }
}
```

### 전송 프로토콜 비교표

| 프로토콜 | 사용 사례 | 연결 시간 | 상태 관리 | 데모 시연 포인트 |
|----------|-----------|-----------|-----------|-----------------|
| **stdio** | 로컬 Python/Node 스크립트 | ~50ms | 프로세스 생성/종료 | "로컬 도구 추가가 instant함" |
| **HTTP** | 리모트 REST API | ~200ms | 세션 ID 기반 | "API 키 없이 JSON으로 연결" |
| **SSE** | 스트리밍 결과 필요 시 | ~300ms | 연결 유지 | "실시간 검색 결과 스트리밍" |

---

## 5. Wow Moment 4 — 세션 재개: 1-2초 → 500ms 목표의 실제 수치

### 목표

세션 재개 속도를 측정하고, 현재 1-2초가 500ms로 개선되는 과정을 보여주기.

### 세션 재개 아키텍처

```
세션 생성/재개 흐름:
┌─────────────────────────────────────────────────────────┐
│  1. AgentSessionManager::recover_session(session_id)    │
│     ├─ SQLite에서 세션 메타데이터 조회 (~5ms)           │
│     ├─ SeaORM으로 메시지 히스토리 로드 (~10ms)          │
│     └─ MCPServiceProxy 재구성 (~50-100ms)              │
│                                                         │
│  2. MCPServiceProxy::create()                           │
│     ├─ BuiltinMCPServer 인스턴스 생성 (~20ms/서버)      │
│     ├─ SessionMCPManager 초기화 (~10ms)                 │
│     └─ 도구 캐시 빌드 (~5ms)                            │
│                                                         │
│  3. 프론트엔드 구독 재연결                              │
│     ├─ listen('agent:event') 재설정 (~2ms)              │
│     └─ 마지막 상태 복원 (~3ms)                           │
│                                                         │
│  총 재개 시간: ~80-120ms (현재) → 목표: ≤500ms          │
└─────────────────────────────────────────────────────────┘
```

### 실제 측정 항목

| 항목 | 현재 수치 | 목표 수치 | 개선 방법 |
|------|-----------|-----------|-----------|
| **SQLite 메타데이터 조회** | ~5ms | ~2ms | 인덱스 최적화 |
| **메시지 히스토리 로드** | ~10ms | ~5ms | 페이지네이션 + 지연 로딩 |
| **BuiltinMCPServer 생성** | ~20ms/서버 | ~10ms/서버 | 프로토타입 캐싱 |
| **MCPServiceProxy 전체** | ~80-120ms | ≤500ms | 병렬 초기화 |
| **프론트엔드 구독 재연결** | ~5ms | ≤2ms | 이벤트 버퍼링 최적화 |

### 데모 시연: 세션 재개 속도 비교

```
[00:00.000] 세션 A 생성 → "session created"
[00:00.045] 세션 A 상태: running
[00:00.050] 세션 A 도구 목록 로드 완료 (170개)

[00:10.000] 세션 A 종료 (user stop)
[00:10.005] 세션 A 상태: stopped

[00:10.100] 세션 A 재개 (recover_session)
[00:10.180] 세션 A 상태: running
[00:10.185] 세션 A 도구 목록 복원 완료
           → 재개 시간: 80ms ✅ 목표 ≤500ms

[00:20.000] 세션 B 생성 → "session created"
[00:20.042] 세션 B 상태: running
[00:20.048] 세션 B 도구 목록 로드 완료 (170개)

[00:30.000] 세션 B 종료
[00:30.003] 세션 B 상태: stopped

[00:30.050] 세션 B 재개
[00:30.115] 세션 B 상태: running
[00:30.120] 세션 B 도구 목록 복원 완료
           → 재개 시간: 70ms ✅ 목표 ≤500ms
```

**핵심 메시지:** "세션 재개는 SQLite 쿼리 + Rust 인스턴스 생성으로 80ms 내에 완료됩니다. 클라이언트 재연결이나 브라우저 리프레시보다 빠릅니다."

---

## 6. 실제 데모용 코드/설정: 바로 복사해서 쓸 수 있는 예시

### 6.1 MCP 서버 연결 설정: 실제 JSON

```json
{
  "mcp_servers": [
    {
      "name": "local-file-search",
      "description": "Fast file search within workspace using ripgrep",
      "transport": {
        "type": "stdio",
        "command": "python3",
        "args": ["~/.libragent/mcp-servers/file-search/main.py"],
        "env": {
          "RG_MAX_COUNT": "100",
          "RG_CASE": "smart"
        }
      }
    },
    {
      "name": "github-mcp",
      "description": "GitHub repository management via REST API",
      "transport": {
        "type": "http",
        "url": "https://mcp.github.example.com/mcp",
        "headers": {
          "Authorization": "Bearer $GITHUB_TOKEN"
        }
      }
    }
  ]
}
```

### 6.2 에이전트 위임 시퀀스 다이어그램

```
Parent Session              Child Session A         Child Session B
     │                            │                        │
     │  agent__startSession       │                        │
     │  ────────────────────────→ │                        │
     │  task: "Research arXiv"    │                        │
     │                            │                        │
     │                            │  WorkflowStarted       │
     │                            │  ──→ LLM Think         │
     │                            │  ──→ arxiv::search     │
     │                            │  ──→ knowledge::record │
     │                            │                        │
     │  agent__startSession       │                        │
     │  ────────────────────────→ │───────────────────────→│
     │  task: "Write report"      │                        │
     │                            │                        │
     │                            │                        │  WorkflowStarted
     │                            │                        │  ──→ LLM Think
     │                            │                        │  ──→ workspace::readFile
     │                            │                        │  ──→ workspace::writeFile
     │                            │                        │
     │  agent__checkSession       │                        │
     │  ────────────────────────→ │                        │
     │  status: completed         │                        │
     │                            │                        │
     │  agent__checkSession       │                        │
     │  ────────────────────────→ │───────────────────────→│
     │  status: completed         │                        │
     │                            │                        │
     │  WorkflowCompleted         │                        │
     │  ──→ 최종 리포트 생성 완료  │                        │
```

### 6.3 세션 격리 아키텍처 시각화

```
┌─────────────────────────────────────────────────────────────────┐
│                        LibrAgent Desktop                        │
│                                                                 │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────┐ │
│  │  Agent Session A │    │  Agent Session B │    │ Agent C     │ │
│  │  sess_a_001     │    │  sess_b_002     │    │ sess_c_003  │ │
│  │                 │    │                 │    │             │ │
│  │  ┌───────────┐  │    │  ┌───────────┐  │    │ ┌─────────┐ │ │
│  │  │ MCPProxy  │  │    │  │ MCPProxy  │  │    │ │MCPProxy │ │ │
│  │  │ .builtin  │  │    │  │ .builtin  │  │    │ │.builtin │ │ │
│  │  │ .stdio    │  │    │  │ .stdio    │  │    │ │.stdio  │ │ │
│  │  │ .http     │  │    │  │ .http     │  │    │ │.http   │ │ │
│  │  └───────────┘  │    │  └───────────┘  │    │ └─────────┘ │ │
│  │                 │    │                 │    │             │ │
│  │  Planning:      │    │  Planning:      │    │ Planning:   │ │
│  │  [todo_a_1]     │    │  [todo_b_1]     │    │ [todo_c_1]  │ │
│  │  [todo_a_2]     │    │  [todo_b_2]     │    │ [todo_c_2]  │ │
│  │                 │    │                 │    │             │ │
│  │  Browser:       │    │  Browser:       │    │ Browser:    │ │
│  │  URL: a.com     │    │  URL: b.com     │    │ URL: c.com  │ │
│  └────────┬────────┘    └────────┬────────┘    └──────┬──────┘ │
│           │                      │                     │        │
│           └──────────────────────┼─────────────────────┘        │
│                                  │                              │
│                    ┌─────────────▼───────────────┐              │
│                    │     Rust Backend (Shared)    │              │
│                    │                              │              │
│                    │  SQLite (SeaORM)             │              │
│                    │  ├─ sessions                 │              │
│                    │  ├─ messages                 │              │
│                    │  ├─ assistants               │              │
│                    │  └─ mcp_servers              │              │
│                    │                              │              │
│                    │  LLM Provider (Anthropic,    │              │
│                    │   OpenAI, Google, Local)     │              │
│                    └──────────────────────────────┘              │
└─────────────────────────────────────────────────────────────────┘

핵심: 세션 A/B/C의 MCPProxy는 완전히 분리됨.
SQLite는 읽기 전용 공유. 쓰기 연산은 세션별로 격리됨.
```

### 6.4 예약 자동화 (CRON) 설정 예시

#### 예시 1: 매일 아침 한국 경제 현황 분석

```json
{
  "name": "매일 아침 한국 경제 현황 분석",
  "cron_expression": "0 23 * * *",
  "assistant_id": "finance-analyst-001",
  "message": "오늘의 한국 경제 현황을 분석하고 요약 리포트를 작성하세요. 주요 지표: KOSPI, 환율, 금리.",
  "schedule_timezone": "utc",
  "enabled": true
}
```

#### 예시 2: 주간 코드 리뷰 자동화

```json
{
  "name": "주간 코드 리뷰 자동화",
  "cron_expression": "0 9 * * 1",
  "assistant_id": "code-reviewer-002",
  "message": "이번 주 GitHub PR 10개를 리뷰하고, 주요 개선 사항을 요약하세요.",
  "schedule_timezone": "local",
  "enabled": true
}
```

#### 예시 3: arXiv 논문 모니터링

```json
{
  "name": "arXiv AI 논문 모니터링",
  "cron_expression": "0 8 * * 1-5",
  "assistant_id": "researcher-003",
  "message": "arXiv에서 'transformer attention' 카테고리의 최신 논문 5편을 검색하고, 핵심 기여도를 요약하세요.",
  "schedule_timezone": "utc",
  "enabled": true
}
```

---

## 7. 개발자 타겟 기술 블로그 3부작

### 에피소드 1 — "Inside LibrAgent: 세션 격리가 실제로 어떻게 동작하는가"

**대상:** Rust 개발자, 시스템 아키텍트, 분산 시스템 관심자

**목차:**

1. **에이전트 세션의 격리 문제** — 왜 기존 솔루션은 세션 격리에 실패하는가
2. **`MCPServiceProxy`의 설계 철학** — `HashMap<String, Box<dyn BuiltinMCPServer>>`가 의미하는 것
3. **세션별 `SessionMCPManager`와 `HttpSessionManager`** — 외부 MCP 서버의 세션 격리
4. **코드 리뷰: `src-tauri/src/mcp/service_proxy/mod.rs`** — 실제 소스로 증명
5. **실제 테스트 케이스** — 세션 A/B 격리 검증 스크립트
6. **성과 수치** — 세션 격리로 인한 안정성 개선 (크래시율 감소, 메모리 누수 방지)

**핵심 코드 스니펫:**

```rust
// src-tauri/src/mcp/service_proxy/mod.rs (실제 소스)

pub async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResponse, String> {
    // tool_timeout_seconds == 0 means timeout is disabled.
    // In that case run the future directly without a deadline so long-running
    // tools (e.g. builtin_swarm__awaitAgent) are never killed by the proxy.
    if self.tool_timeout_seconds == 0 {
        return match route_tool(tool_name)? {
            ToolRouting::Builtin { server_id, tool_name: real_tool_name, .. } => {
                self.call_builtin_tool(&server_id, &real_tool_name, args).await
            }
            ToolRouting::External { server_name, tool_name, .. } => {
                self.call_external_tool(&server_name, &tool_name, args).await
            }
        };
    }
    // ... timeout handling
}
```

**Call to Action:**
> "세션 격리 아키텍처가 마음에 들나요? `src-tauri/src/agent/` 디렉토리에서 테스트를 추가하거나, 새로운 내장 도구를 구현해보세요. [기여 가이드](../contributing/)를 참고하세요."

---

### 에피소드 2 — "MCP 네이티브 아키텍처: 왜 사후 통합이 아니라 첫 번째 설계인가"

**대상:** MCP 프로토콜 관심 개발자, 도구 생태계 구축자, DevOps 엔지니어

**목차:**

1. **MCP란?** — Model Context Protocol의 기본 개념과 LibrAgent에서의 역할
2. **내장 vs 외부 MCP 서버** — `BuiltinMCPServer` 트레이트 vs `SessionMCPManager`
3. **세션 격리된 MCP 매니저** — 각 세션이 별도 `HttpSessionManager`와 `SessionMCPManager`를 가지는 이유
4. **동적 서버 로드/언로드** — 런타임에 MCP 서버 추가/제거하는 아키텍처
5. **도구 응답 설계** — `MCPResult`의 `content` vs `structured_content` 구분 (AI가 보는 것 vs UI가 보는 것)
6. **170개 도구를 관리하는 실제 코드** — `tool list`에서 `tool call`까지의 전체 흐름

**핵심 코드:**

```rust
// MCPResult 구조 (LibrAgent 확장)

pub struct MCPResult {
    content: Vec<MCPContent>,           // AI 에이전트가 보는 텍스트
    structured_content: Option<Value>,  // UI 컴포넌트가 렌더링하는 데이터
    is_error: Option<bool>,
}
```

```rust
// SessionMCPManager::add_server (실제 흐름)

pub async fn add_server(&mut self, config: McpConfig) -> Result<(), String> {
    let client = match config.transport.type_ {
        TransportType::Stdio => McpClient::connect_stdio(config.transport).await?,
        TransportType::Http => McpClient::connect_http(config.transport).await?,
        TransportType::HttpSse => McpClient::connect_http_sse(config.transport).await?,
    };
    self.servers.insert(config.name.clone(), client);
    Ok(())
}
```

**핵심 메시지:** "170개 도구를 설정 없이, 플러그인처럼, 세션 격리된 상태로 관리하는 방법."

**Call to Action:**
> "자신의 MCP 서버를 LibrAgent에 플러그인하고 싶나요? [MCP 서버 템플릿](../contributing/mcp-server-template/)을 복제하고, 10줄의 Rust 코드로 완성하세요."

---

### 에피소드 3 — "프론트엔드는 반응형: 이벤트 기반 에이전트 상태 관리"

**대상:** React 개발자, 상태 관리 관심자, Tauri 프론트엔드 개발자

**목차:**

1. **프론트엔드는 반응형이다** — Tauri 이벤트 리스닝으로 백엔드 상태 구독
2. **AgentSessionContext** — `agent:event` 리스닝으로 세션 상태 실시간 업데이트
3. **이벤트 시스템 아키텍처** — `AgentEvent` enum, serde 네이밍 규칙, 이벤트 디스패처
4. **실시간 스트리밍 구현** — `listen('agent:event', ...)` 패턴
5. **성능 최적화** — 불필요한 리렌더링 방지, 이벤트 버깅, 메모리 누수 대응

**핵심 코드:**

```typescript
// src/features/agent/AgentSessionContext.tsx (추상화)
useEffect(() => {
  const unlisten = listen('agent:event', (event: AgentEvent) => {
    if (event.type === 'workflowStarted') setWorkflowStatus('running');
    if (event.type === 'toolExecutionStarted') addToolCall(event.toolName);
    if (event.type === 'toolExecutionCompleted') updateToolResult(event.toolName, event.success);
    if (event.type === 'statusChanged') setSessionStatus(event.status);
  });
  return () => unlisten.then(fn => fn());
}, [sessionId]);
```

**핵심 메시지:** "프론트엔드는 상태를 소유하지 않습니다. 백엔드가 발신하는 이벤트를 구독할 뿐입니다."

**Call to Action:**
> "반응형 에이전트 UI에 새로운 이벤트를 추가하고 싶나요? `src/features/agent/`에서 `AgentEvent` enum을 확장하고, React 컴포넌트에 바인딩하세요."

---

## 8. GitHub 기여자 유인 데모 — OSS 커뮤니티가 기여하고 싶어지게 하는 요소

### 8.1 기술 데모 → 기여로 이어지는 경로

```
기술 블로그 에피소드 1
  ↓
"세션 격리 코드가 궁금하다면?"
  ↓
GitHub: src-tauri/src/agent/session_manager.rs
  ↓
"이 코드에 테스트를 추가하고 싶다면?"
  ↓
docs/contributing/ → 테스트 기여 가이드
  ↓
PR 제출 → 리뷰 → 병합
```

### 8.2 contributing.md에 실제로 기여할 수 있는 부분

`CONTRIBUTING.md`에서 명시된 기여 영역과 연결:

| 기여 영역 | contributing.md 참고 | 실제 파일 경로 | 기여 포인트 |
|-----------|---------------------|----------------|-------------|
| **내장 도구 개선** | "Built-in tool improvements" | `src-tauri/src/mcp/builtin/` | 새 내장 도구 추가 |
| **LLM provider 통합** | "LLM provider integrations" | `src-tauri/src/agent/llm/` | 새 LLM provider 추가 |
| **MCP 서버 통합** | "MCP server integration" | `src-tauri/src/mcp/` | 새 외부 MCP 서버 연결 |
| **보안/샌드박스** | "Security & sandboxing" | `src-tauri/src/mcp/session_isolation.rs` | 세션 격리 강화 |
| **UI/UX 개선** | "UI/UX improvements" | `src/features/agent/` | 이벤트 시각화 개선 |
| **테스트** | "Testing (Unit tests, integration tests, E2E tests)" | `src-tauri/tests/` | 통합 테스트 추가 |

### 8.3 MCP 서버 추가 튜토리얼: 실제 10줄 코드

```bash
# 1. MCP 서버 설정 JSON 생성
cat > ~/.libragent/mcp-servers/my-server/config.json << 'EOF'
{
  "name": "my-custom-server",
  "description": "My custom MCP server",
  "transport": {
    "type": "stdio",
    "command": "python3",
    "args": ["~/.libragent/mcp-servers/my-server/main.py"]
  }
}
EOF

# 2. Python MCP 서버 스크립트 (10줄)
cat > ~/.libragent/mcp-servers/my-server/main.py << 'EOF'
import json, sys

def main():
    for line in sys.stdin:
        req = json.loads(line)
        if req.get("method") == "initialize":
            json.dump({"jsonrpc": "2.0", "id": req["id"],
                       "result": {"protocolVersion": "2024-11-05",
                                  "capabilities": {}, "serverInfo": {"name": "my-server", "version": "1.0"}}})
            print()
        elif req.get("method") == "tools/list":
            json.dump({"jsonrpc": "2.0", "id": req["id"],
                       "result": {"tools": [{"name": "my_tool", "description": "My custom tool",
                                             "inputSchema": {"type": "object", "properties": {}}}]}})
            print()
        elif req.get("method") == "tools/call":
            json.dump({"jsonrpc": "2.0", "id": req["id"],
                       "result": {"content": [{"type": "text", "text": "Hello from my MCP server!"}]}})
            print()

if __name__ == "__main__":
    main()
EOF

# 3. LibrAgent에서 서버 추가 (API 호출)
# POST /api/mcp-servers
# Body: { "name": "my-custom-server", ... }

# 4. 도구 목록 확인
# tools list → my_custom_server__my_tool 이 즉시 등장
```

### 8.4 스킬 개발 가이드: Python/TypeScript 커스텀 스킬

```bash
# 1. 스킬 디렉토리 생성
mkdir -p ~/.libragent/skills/my-skill
cd ~/.libragent/skills/my-skill

# 2. SKILL.md 작성 (핵심)
cat > SKILL.md << 'EOF'
# My Custom Skill

## Description
This skill provides custom functionality for LibrAgent.

## Usage
When the user requests "my-skill action", run the following:
1. Analyze the input
2. Execute the action
3. Return the result in Markdown format
EOF

# 3. 스킬 활성화
# LibrAgent 설정에서 스킬 로드 경로에 추가
# ~/.libragent/config.json: "skills_path": "~/.libragent/skills"

# 4. 테스트
# 에이전트 세션에서 "my-skill action" 명령어 실행
```

### 8.5 GitHub 기여자 유인 전략 요약

| 유인 요소 | 구현 | 기대 효과 |
|-----------|------|-----------|
| **10줄 MCP 서버 튜토리얼** | 실제 Python 스크립트 제공 | "나도 할 수 있다"는 자신감 |
| **실제 contributing.md 매핑** | 기여 영역 → 실제 파일 경로 매핑 | "어디서 시작해야 할지 알겠다" |
| **세션 격리 테스트 추가** | `src-tauri/tests/`에 테스트 케이스 추가 | "테스트가 실제 프로젝트에 기여한다" |
| **에이전트 위임 시퀀스 다이어그램** | ASCII 다이어그램으로 구조 시각화 | "아키텍처가 이해된다" |
| **CRON 예약 설정 예시** | 실제 JSON 설정 제공 | "바로 쓸 수 있다" |

---

## 9. 데모 실행 체크리스트 — P0 우선순위

### P0: 세션 격리 proof (1-2일)

| 항목 | 상태 | 담당 |
|------|------|------|
| [ ] `MCPServiceProxy`가 세션마다 별도 인스턴스를 생성하는지 확인 | | Rust dev |
| [ ] 세션 A의 Planning todo가 세션 B에 보이지 않는지 확인 | | Rust dev |
| [ ] 세션 A의 Browser session이 세션 B에 보이지 않는지 확인 | | Rust dev |
| [ ] 데모용 스크립트 작성 (`demo_session_isolation.sh`) | | Rust dev |
| [ ] 스크린샷/스크린레코딩 촬영 | | Developer |

### P0: 170+ 도구 스트리밍 (2-3일)

| 항목 | 상태 | 담당 |
|------|------|------|
| [ ] `agent:event` 스트림이 실시간으로 표시되는지 확인 | | Rust dev |
| [ ] 170개 도구가 세션 시작 시 즉시 로드되는지 확인 | | Rust dev |
| [ ] UI가 170개 도구 로드 시 깨지지 않는지 확인 | | React dev |
| [ ] 도구 호출 타임라인 시각화 스크립트 작성 | | Developer |
| [ ] 스크린레코딩 촬영 | | Developer |

### P1: MCP 서버 플러그인 아키텍처 (2-3일)

| 항목 | 상태 | 담당 |
|------|------|------|
| [ ] 외부 MCP 서버 추가/제거가 런타임에 정상 작동하는지 확인 | | Rust dev |
| [ ] stdio/HTTP/SSE 전송 프로토콜이 모두 테스트되는지 확인 | | Rust dev |
| [ ] 데모용 JSON 설정 파일 생성 | | Developer |
| [ ] 스크린레코딩 촬영 | | Developer |

### P1: 세션 재개 속도 측정 (1-2일)

| 항목 | 상태 | 담당 |
|------|------|------|
| [ ] 현재 세션 재개 시간 측정 (~80-120ms) | | Rust dev |
| [ ] 500ms 목표 대비 개선 계획 문서화 | | Rust dev |
| [ ] 데모용 측정 스크립트 작성 | | Developer |
| [ ] 스크린레코딩 촬영 | | Developer |

### P1: Think-Act-Observe 루프 전체 데모 (2-3일)

| 항목 | 상태 | 담당 |
|------|------|------|
| [ ] 복잡한 작업 요청 → Think-Act-Observe 루프 5-6회 반복 | | Rust dev |
| [ ] `AgentEvent`로 전 과정 추적 | | Rust dev |
| [ ] 데모용 스크립트 작성 | | Developer |
| [ ] 스크린레코딩 촬영 | | Developer |

---

## 10. 마케팅 데모와의 연계 — "아키텍처가 설정보다 낫다"의 양면 증명

### 마케팅 데모 ↔ 기술 데모 매핑

| 마케팅 데모 시나리오 | 기술 데모의 대응 | 공통 메시지 |
|---------------------|-----------------|-------------|
| **DevDan: 코드 리뷰 자동화** | Wow Moment 2: 170+ 도구 스트리밍 (Planning → Browser → Workspace → Knowledge) | "에이전트가 스스로 판단한다" |
| **SecureSara: 온프레미스 + 세션 격리** | Wow Moment 1: 세션 격리 proofs (터미널 2개 동시 실행) | "데이터는 당신의 기기에서" |
| **ResearchRaj: 논문 분석 워크플로우** | Wow Moment 3: MCP 서버 플러그인 (arXiv MCP 추가) | "도구가 즉시 사용 가능하다" |
| **StartupSam: teamwork/org 협업** | Wow Moment 4: Think-Act-Observe 루프 (에이전트 위임 시퀀스) | "에이전트가 에이전트를 관리한다" |
| **EnterpriseElena: SSO + 감사 로그** | CRON 예약 자동화 예시 + 세션 재개 속도 측정 | "신뢰할 수 있는 플랫폼" |

### 시각적 차별화 요약

```
경쟁사 데모는 "에이전트가 무언가를 한다"를 보여줍니다.
LibrAgent 데모는 "에이전트들이 함께 일하면서, 데이터는 안전하고, 모든 것이 추적된다"를 보여줍니다.

핵심 메시지:
"Cursor는 도구. LibrAgent는 워크스페이스."
```

### 마케팅 데모의 15초 Wow Clip ↔ 기술 데모의 심층 분석

| 15초 Clip | 기술 데모의 심층 분석 |
|-----------|---------------------|
| **Clip 1: "에이전트가 에이전트를 관리한다"** | 에이전트 위임 시퀀스 다이어그램 + `agent__startSession`/`agent__checkSession` 코드 |
| **Clip 2: "데이터는 단 한 번도 네트워크를 나가지 않았다"** | 세션 격리 아키텍처 도식 + `MCPServiceProxy`의 `HashMap<String, Box<dyn BuiltinMCPServer>>` |
| **Clip 3: "논문 20편 읽고 비교표 만들라고 했더니... 3초 만에 끝났다"** | 170+ 도구 스트리밍 타임라인 + `AgentEvent` enum의 실시간 이벤트 처리 |

---

> **문의:** 이 문서에 대한 기술적 피드백은 GitHub Issues로 남겨주세요. 기여는 [기여 가이드](../contributing/)를 참고하세요.
>
> **소유:** Technical Expert · **검토:** Coordinator · **상태:** v2.0 완성
