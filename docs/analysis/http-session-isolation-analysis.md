# Critical Analysis: HTTP MCP Session Management & Global Manager Removal

## 발견 사항 (Findings)

### 1. HTTP 연결의 세션 ID 관리 현황

**Global Manager의 HTTP 연결 생성 시:**

```rust
// src-tauri/src/mcp/server/lifecycle.rs (Line 118-121)
// Add session ID if provided
if let Some(sid) = session_id {
    if let Ok(v) = reqwest::header::ValueFrom::from_str(&sid) {
        if let Ok(k) = reqwest::header::HeaderName::from_bytes(b"Mcp-Session-Id") {
            header_map.insert(k, v);
        }
    }
}
```

**문제점:**

- Global Manager는 `start_http_server` 호출 시 **단일 session_id만 헤더에 설정**
- 이후 다른 세션에서 같은 HTTP 서버를 사용하려면 **헤더를 변경할 수 없음** (reqwest client는 생성 시점에 default_headers 고정)
- 결과적으로 **HTTP 서버도 세션별로 격리되어야 함** (현재 Global 방식으로는 다중 세션 지원 불가)

### 2. HttpSessionManager의 의도

**설계 의도 (src-tauri/src/mcp/session_isolation/http_manager.rs):**

```rust
/// Unlike stdio servers which need isolated processes, HTTP servers are shared
/// across sessions but inject the session ID via the Mcp-Session-Id header.
```

**실제 구현:**

- `HttpSessionManager`는 세션별로 생성되지만 **실제로 사용되지 않음**
- `create_proxy` 호출 시 HTTP 서버 연결을 세션별로 생성하는 로직 **없음**
- Global Manager의 HTTP 연결을 그대로 공유하는 구조

### 3. 상태 확인 기능의 문제점

**Frontend의 connectServers (MCPServerContext.tsx):**

```typescript
const rawToolsByServer = await listToolsFromConfig(mcpConfig);
// 모든 서버(HTTP + Stdio)를 Global Manager에 연결
// 이 연결들이 영구적으로 남아 세션 격리 방해
```

**결과:**

1. UI에서 "상태 확인" 시 Global Manager에 모든 서버 연결 생성
2. 첫 번째 세션의 session_id가 HTTP 헤더에 고정됨
3. 이후 세션들은 잘못된 session_id로 HTTP 서버와 통신

## 결론 (Conclusion)

### Global Manager는 완전히 제거되어야 함

**이유:**

1. **HTTP 서버도 세션별 격리 필요**: reqwest client의 default_headers는 생성 시점에 고정되므로, 여러 세션이 다른 session_id를 사용할 수 없음.
2. **현재 설계 불완전**: `HttpSessionManager`가 있지만 실제로는 사용되지 않고 Global Manager의 연결을 그대로 공유.
3. **상태 확인 기능의 부작용**: 첫 세션 또는 임의 세션의 ID가 Global 연결에 고정되어 다른 세션들의 도구 호출이 잘못된 컨텍스트로 실행됨.

### 올바른 아키텍처

**모든 MCP 연결은 세션별로 독립 관리:**

- **Stdio 서버**: `SessionMCPManager` (이미 구현됨)
- **HTTP 서버**: `HttpSessionManager` (선언만 되어있고 미사용)
  - 각 세션마다 독립적인 reqwest client 생성
  - 각 세션의 session_id를 Mcp-Session-Id 헤더에 설정
  - `MCPServiceProxy` 생성 시 HTTP 연결도 함께 생성

**제거 대상:**

1. `list_tools_from_config` 명령어 (Frontend 상태 확인 기능)
2. Global `MCPServerManager.connections` (HTTP/Stdio 통합 관리)
3. `MCPServiceProxyManager.list_all_external_tools()` (Global 도구 조회)

**유지/수정 대상:**

1. Builtin 서버 도구 조회 (세션별로 이미 격리됨)
2. 세션 생성 시 Eager Tool Discovery (Stdio는 이미 구현, HTTP 추가 필요)
3. `collect_available_tools` 로직 (Global 조회 제거, 세션 캐시만 사용)

## 다음 단계 (Next Steps)

1. **HttpSessionManager 활성화**: `create_proxy` 시 HTTP 서버도 세션별 연결 생성
2. **Frontend 상태 확인 제거**: `listToolsFromConfig` 호출 제거, 세션 생성 후 도구 자동 수집
3. **Global Manager 단계적 제거**:
   - 1단계: HTTP/Stdio 분리 (이미 필터링 적용)
   - 2단계: HTTP도 세션별 관리로 이관
   - 3단계: Global connections 완전 제거

## 위험 요소 (Risks)

- **Breaking Change**: Frontend가 의존하는 `listToolsFromConfig` API 제거
- **UI/UX 변경**: 사용자가 세션 시작 전에 도구 목록을 미리 볼 수 없음
- **Migration 필요**: 기존 Assistant 설정의 MCP 서버 연결 방식 변경
