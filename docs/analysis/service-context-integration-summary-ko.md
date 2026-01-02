# Service Context 통합 구조 분석 요약

## 📋 분석 개요

`AgentChatContext.tsx`, `service_proxy.rs`, 그리고 3개 builtin 툴(browser, playbook, content_store)의 service context 통합 구조를 분석하여 시스템 프롬프트에 동적 상태 정보가 어떻게 통합되는지 파악했습니다.

---

## 🔄 Service Context 통합 플로우

### 1단계: Context 수집 (`MCPServiceProxy`)

**위치**: `src-tauri/src/mcp/service_proxy.rs:147`

```rust
pub async fn get_service_contexts(&self) -> HashMap<String, ServiceContext>
```

- 각 에이전트 세션마다 독립적인 `MCPServiceProxy` 인스턴스 보유
- 등록된 모든 builtin 서버를 순회하며 `get_service_context()` 호출
- `tool_id -> ServiceContext` 맵 형태로 반환

### 2단계: System Prompt 구성 (Rust Backend)

**위치**: `src-tauri/src/agent/llm.rs:543`

```rust
pub async fn build_system_prompt(
    agent_config: &AgentConfig,
    proxy: Option<Arc<MCPServiceProxy>>,
) -> Result<String, String>
```

**시스템 프롬프트 구조**:

1. 시간/위치 컨텍스트 (날짜, 시간, 타임존)
2. 에이전트 커스텀 시스템 프롬프트
3. **Service Contexts** - 모든 툴의 현재 상태 정보

### 3단계: Frontend 통합 (선택사항)

**위치**: `src/context/AgentChatContext.tsx:134`

```typescript
const contexts = await invoke<Record<string, ServiceContext>>(
  'agent_get_service_contexts',
  { sessionId }
);
```

- UI 표시 목적으로만 사용
- 실제 시스템 프롬프트 구성은 Rust 백엔드에서 처리

---

## 📊 Builtin 툴 Service Context 구현 현황

### ✅ Content Store (완전 구현)

**파일**: `src-tauri/src/mcp/builtin/content_store/server.rs:153`

**제공 정보**:

- 세션별 저장된 파일 개수
- 최근 5개 파일 목록 (파일명, 크기, 미리보기)
- 구조화된 상태: `content_count`, `session_id`

**출력 예시**:

```text
## Content Store
**5 files stored**
  1. **project_requirements.txt** (12KB) - The project should implement...
  2. **api_documentation.md** (45KB) - # API Documentation...
  ...
```

### ❌ Browser (미구현)

**파일**: `src-tauri/src/mcp/builtin/browser/mod.rs:77`

**현재 상태**: 빈 문자열 반환

**권장 구현**:

- 활성 브라우저 세션 ID
- 현재 URL 및 페이지 제목
- 열린 탭/세션 수
- 마지막 탐색 작업

### ❌ Playbook (미구현)

**파일**: `src-tauri/src/mcp/builtin/playbook/mod.rs:153`

**현재 상태**: 기본 trait 구현 사용 (설명만 표시)

```text
## Playbook
**Description**: Playbook management for reusable workflows
```

**권장 구현**:

- DB에서 플레이북 개수 조회
- 최근 3개 플레이북 목록 (goal 포함)
- 현재 선택된 플레이북 강조 표시
- 구조화된 상태: `playbook_count`, `session_id`

---

## 🔑 핵심 설계 패턴

### 1. 세션 격리 (Session Isolation)

- 각 에이전트 세션이 독립적인 `MCPServiceProxy` 보유
- 툴 상태가 세션별로 격리됨 (예: todo 리스트, 콘텐츠 저장소)

### 2. 지연 컨텍스트 로딩 (Lazy Context Loading)

- 시스템 프롬프트 구성 시점에만 컨텍스트 조회
- 주기적 폴링이나 백그라운드 업데이트 없음
- 매 LLM 호출마다 최신 상태 반영

### 3. 구조화된 상태 (Structured State)

- `context_prompt`: LLM을 위한 마크다운 텍스트
- `structured_state`: 프론트엔드/디버깅용 JSON 데이터

### 4. 폴백 패턴 (Fallback Pattern)

- 기본 trait 구현이 기본 컨텍스트 제공
- 툴은 오버라이드하여 풍부한 상태 정보 추가
- 오류 시 우아한 성능 저하 (graceful degradation)

---

## 📈 구현 현황 요약 테이블

| 툴             | 상태        | 컨텍스트 정보                      | 구조화된 상태                 |
| -------------- | ----------- | ---------------------------------- | ----------------------------- |
| Content Store  | ✅ 구현됨   | 파일 개수, 파일 목록, 미리보기     | `content_count`, `session_id` |
| Playbook       | ❌ 미구현   | 기본 설명만 (default trait)        | 없음                          |
| Browser        | ❌ 미구현   | 빈 문자열                          | 없음                          |
| Planning       | ✅ 구현됨   | Todo 개수, 최근 todos, 완료 통계   | `todo_count`, `completed`     |
| Knowledge      | ✅ 구현됨   | 지식 항목 개수, 최근 항목          | `entry_count`, `session_id`   |
| Workspace      | ✅ 구현됨   | 현재 워크스페이스 경로, 파일 구조  | `workspace_path`, `file_count`|

---

## 💡 주요 발견사항

### 1. Service Context는 시스템 프롬프트에 자동 통합됨

- LLM에게 툴의 현재 상태를 동적으로 전달
- 에이전트가 컨텍스트 인식 결정을 내릴 수 있게 함
- 예: "Content Store에 5개 파일이 있음" → 에이전트가 기존 콘텐츠 활용 가능

### 2. Browser와 Playbook은 Service Context 구현 필요

**Browser 구현 시 이점**:

- 에이전트가 활성 브라우저 세션 인식
- 현재 페이지 정보 기반 작업 수행
- 불필요한 세션 재생성 방지

**Playbook 구현 시 이점**:

- 에이전트가 사용 가능한 플레이북 인식
- 유사 작업에 기존 플레이북 재사용
- 플레이북 활용도 향상

### 3. 토큰 효율성 고려 필요

- 모든 Service Context는 간결해야 함 (< 500자 권장)
- 마크다운 포맷 사용하여 가독성 확보
- 실행 가능한 힌트 포함 (예: "Use createSession to start")

---

## 🎯 권장사항

### Browser 툴 개선

```rust
async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    if let Some(session_id) = self.browser_session_id.read().ok().and_then(|g| g.clone()) {
        ServiceContext {
            context_prompt: format!(
                "## Browser\n**Session ID**: {}\n**Status**: Active\n\
                *Use navigation tools to interact with the browser.*",
                session_id
            ),
            structured_state: Some(json!({
                "session_id": session_id,
                "active": true
            })),
        }
    } else {
        ServiceContext {
            context_prompt: "## Browser\n**Status**: No active session\n\
                *Use createSession to start browsing.*".to_string(),
            structured_state: None,
        }
    }
}
```

### Playbook 툴 개선

```rust
async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM playbooks WHERE session_id = ?"
    )
    .bind(&self.session_id)
    .fetch_one(self.db_pool.as_ref())
    .await
    .unwrap_or(0);
    
    if count > 0 {
        // 최근 3개 플레이북 가져오기 및 표시
        // ...
    } else {
        ServiceContext {
            context_prompt: "## Playbook\n**No playbooks yet**\n\
                *Use createPlaybook to save reusable workflows.*".to_string(),
            structured_state: Some(json!({ "playbook_count": 0 })),
        }
    }
}
```

---

## 📚 참고 자료

- **상세 분석 문서**: `docs/analysis/service-context-integration-analysis.md`
- **Service Proxy**: `src-tauri/src/mcp/service_proxy.rs`
- **System Prompt Builder**: `src-tauri/src/agent/llm.rs`
- **Frontend Context**: `src/context/AgentChatContext.tsx`
- **Type Definitions**: `src-tauri/src/mcp/types.rs`

---

## 🧪 테스트 방법

### Frontend에서 테스트

```typescript
const contexts = await invoke<Record<string, ServiceContext>>(
  'agent_get_service_contexts',
  { sessionId: 'your-session-id' }
);
console.log(contexts);
```

### Rust Backend에서 직접 테스트

```rust
let proxy = proxy_manager.get_proxy(session_id).await.unwrap();
let contexts = proxy.get_service_contexts().await;
for (tool_id, context) in contexts {
    println!("{}: {}", tool_id, context.context_prompt);
}
```

---

**분석 완료일**: 2025-01-02  
**분석 대상**: LibrAgent Agent V2 Architecture
