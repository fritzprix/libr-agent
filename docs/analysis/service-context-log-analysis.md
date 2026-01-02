# System Prompt Service Context 로그 분석 결과

## 📋 분석 대상 로그

```text
## Playbook
**Description**: Playbook management for reusable workflows

## Content Store
**Status**: No active session

## Planning
**Current Goal:** "AI 데이터센터 관련 주요 뉴스 (2026년 1월 2일 기준) 검색 및 심층 분석 리포트 작성"
*Goal is active. Track progress with todos below.*

**Todos:** 2 unchecked / 0 checked (2 total)
**Unchecked Items:**
  [0] ID:16 | 글로벌 뉴스 검색 | Priority:high
  [1] ID:17 | 뉴스 선정 | Priority:medium
...

## Workspace
**Directory**: C:\Users\SKTelecom\AppData\Roaming\...
**Running Processes**: 0
**Platform**: windows/x86_64
```

---

## 🔍 발견된 문제점

### 1. ❌ **Content Store Context 오류** (심각)

**문제**:
```text
## Content Store
**Status**: No active session
```

**원인 분석**:
- Content Store가 세션 ID를 인식하지 못함
- `session_manager.get_current_session()` 호출이 `None` 반환
- 실제로는 활성 세션이 존재함 (Planning, Workspace 정상 작동)

**예상 영향**:
- 에이전트가 Content Store 사용 불가능하다고 판단
- `addContent`, `searchKnowledge` 등 툴 사용 회피
- 파일/문서 저장 및 검색 기능 사용 불가

**근본 원인** (`content_store/server.rs:153-165`):
```rust
pub async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    let session_id = match self.session_manager.get_current_session() {
        Some(sid) => sid,
        None => {
            return ServiceContext {
                context_prompt: "## Content Store\n**Status**: No active session".to_string(),
                structured_state: None,
            };
        }
    };
    // ...
}
```

**문제점**:
1. `get_current_session()`이 `MCPServiceProxy`의 세션 컨텍스트와 동기화되지 않음
2. Content Store는 `session_id`를 생성자에서 받지만 context 조회 시 무시함
3. 다른 툴(Planning, Workspace)은 생성자의 `session_id`를 직접 사용

**비교 - Planning (정상 작동)**:
```rust
// Planning은 self.session_id를 직접 사용
impl PlanningServer {
    session_id: String,  // 생성자에서 받은 세션 ID 저장
    
    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        context::get_service_context(&self.db_pool, &self.session_id).await
        // ✅ self.session_id 직접 사용
    }
}
```

**비교 - Content Store (문제)**:
```rust
// Content Store는 session_manager에 의존
impl ContentStoreServer {
    session_id: String,  // 생성자에서 받지만 사용 안 함!
    session_manager: Arc<SessionManager>,
    
    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        let session_id = match self.session_manager.get_current_session() {
            Some(sid) => sid,  // ❌ session_manager 호출 (실패)
            None => return ServiceContext { ... },
        };
        // self.session_id를 사용하지 않음!
    }
}
```

---

### 2. ⚠️ **Playbook Context 미구현** (중간)

**문제**:
```text
## Playbook
**Description**: Playbook management for reusable workflows
```

**원인**:
- `get_service_context()` 오버라이드 없음
- 기본 trait 구현만 사용 (설명만 표시)

**예상 영향**:
- 에이전트가 사용 가능한 플레이북 인식 불가
- 재사용 가능한 워크플로우 활용도 저하
- 매번 새로운 계획 수립 (기존 플레이북 재사용 불가)

**현재 코드** (`playbook/mod.rs:153`):
```rust
#[async_trait]
impl BuiltinMCPServer for PlaybookServer {
    fn name(&self) -> &str { "playbook" }
    fn description(&self) -> &str { "Playbook management for reusable workflows" }
    fn tools(&self) -> Vec<MCPTool> { ... }
    async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResult, String> { ... }
    
    // ❌ get_service_context() 오버라이드 없음!
    // 기본 trait 구현 사용 (단순 설명만 반환)
}
```

---

### 3. ✅ **Planning Context 정상 작동** (참고)

**정상 출력**:
```text
## Planning
**Current Goal:** "AI 데이터센터 관련 주요 뉴스..."
**Todos:** 2 unchecked / 0 checked (2 total)
**Unchecked Items:**
  [0] ID:16 | 글로벌 뉴스 검색 | Priority:high
  [1] ID:17 | 뉴스 선정 | Priority:medium
```

**성공 요인**:
- `self.session_id` 직접 사용
- DB 조회로 실시간 todo 정보 표시
- 구조화된 상태 제공

---

### 4. ✅ **Workspace Context 정상 작동** (참고)

**정상 출력**:
```text
## Workspace
**Directory**: C:\Users\SKTelecom\AppData\Roaming\...
**Running Processes**: 0
**Platform**: windows/x86_64
```

**성공 요인**:
- `self.session_id` 직접 사용
- 세션별 워크스페이스 경로 표시
- 실행 중인 프로세스 개수 추적

---

## 🚨 심각도 평가

### Critical (즉시 수정 필요)

**Content Store 세션 인식 실패**:
- **영향도**: 높음 (핵심 기능 사용 불가)
- **빈도**: 모든 세션에서 발생
- **사용자 경험**: 파일 저장/검색 불가능
- **우선순위**: P0 (즉시 수정)

### Medium (개선 권장)

**Playbook Context 미구현**:
- **영향도**: 중간 (기능은 작동하나 효율성 저하)
- **빈도**: 플레이북 사용 시나리오
- **사용자 경험**: 재사용성 저하
- **우선순위**: P1 (다음 스프린트)

---

## 🔧 해결 방안

### 1. Content Store 긴급 수정 (P0)

**방법 A: 생성자 session_id 사용** (권장):

```rust
// src-tauri/src/mcp/builtin/content_store/server.rs:153
pub async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    // ✅ session_manager 대신 self.session_id 직접 사용
    let session_id = &self.session_id;
    
    // Get content information for this session
    let (count, summaries) = match self.storage.try_lock() {
        Ok(storage) => {
            let count = storage.get_content_count(session_id);
            let summaries = storage.get_content_summary(session_id, 5);
            (count, summaries)
        }
        Err(e) => {
            log::warn!("Failed to lock content storage: {}", e);
            return ServiceContext {
                context_prompt: "## Content Store\n**Status**: Error loading state".to_string(),
                structured_state: None,
            };
        }
    };
    
    // ... (나머지 로직 동일)
}
```

**변경 사항**:
- `session_manager.get_current_session()` 제거
- `self.session_id` 직접 사용 (생성자에서 이미 받음)
- Planning/Workspace와 동일한 패턴

**테스트**:
```bash
# Content Store가 정상적으로 세션 인식하는지 확인
# 예상 출력: "## Content Store\n**0 files stored**" (또는 실제 파일 개수)
```

---

**방법 B: switch_context 호출 보장** (대안):

```rust
// src-tauri/src/mcp/service_proxy.rs
impl MCPServiceProxy {
    pub async fn new(...) -> Result<Self, String> {
        // ...
        
        // 각 builtin 서버 초기화 후 switch_context 호출
        for (tool_id, server) in &builtin_servers {
            server.switch_context(ServiceContextOptions {
                session_id: Some(session_id.clone()),
                assistant_id: None,
            }).await?;
        }
        
        Ok(Self { ... })
    }
}
```

**장단점**:
- ✅ session_manager 동기화 보장
- ❌ 추가 복잡도 증가
- ❌ 모든 서버에 switch_context 구현 필요

---

### 2. Playbook Context 구현 (P1)

```rust
// src-tauri/src/mcp/builtin/playbook/mod.rs:153
#[async_trait]
impl BuiltinMCPServer for PlaybookServer {
    // ... (기존 메서드들)
    
    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        // DB에서 플레이북 개수 조회
        let count_result = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM playbooks WHERE session_id = ?"
        )
        .bind(&self.session_id)
        .fetch_one(self.db_pool.as_ref())
        .await;
        
        match count_result {
            Ok(count) if count > 0 => {
                // 최근 3개 플레이북 가져오기
                let recent = sqlx::query_as::<_, (String, String)>(
                    "SELECT id, goal FROM playbooks WHERE session_id = ? 
                     ORDER BY updated_at DESC LIMIT 3"
                )
                .bind(&self.session_id)
                .fetch_all(self.db_pool.as_ref())
                .await
                .unwrap_or_default();
                
                let mut parts = vec![
                    format!("## Playbook\n**{} playbooks available**\n", count),
                ];
                
                if !recent.is_empty() {
                    parts.push("**Recent:**".to_string());
                    for (id, goal) in recent {
                        let goal_short = if goal.len() > 50 {
                            format!("{}...", &goal[..50])
                        } else {
                            goal.clone()
                        };
                        parts.push(format!("  - {} (ID: {})", goal_short, &id[..8]));
                    }
                }
                
                ServiceContext {
                    context_prompt: parts.join("\n"),
                    structured_state: Some(serde_json::json!({
                        "playbook_count": count,
                        "session_id": self.session_id
                    })),
                }
            }
            Ok(_) => {
                ServiceContext {
                    context_prompt: "## Playbook\n**No playbooks yet**\n*Use createPlaybook to save reusable workflows.*".to_string(),
                    structured_state: Some(serde_json::json!({
                        "playbook_count": 0
                    })),
                }
            }
            Err(e) => {
                log::error!("Failed to get playbook context: {}", e);
                ServiceContext {
                    context_prompt: "## Playbook\n**Status**: Error loading state".to_string(),
                    structured_state: None,
                }
            }
        }
    }
}
```

**예상 출력** (플레이북 있을 때):
```text
## Playbook
**3 playbooks available**

**Recent:**
  - AI 데이터센터 뉴스 분석 워크플로우 (ID: a3f2b8c1)
  - 주간 시장 리포트 생성 프로세스 (ID: d4e9f1a2)
  - 기술 문서 요약 및 정리 (ID: b7c3e5d6)
```

---

## 📊 수정 우선순위

### Phase 1 (즉시 - 이번 주)

1. **Content Store 세션 인식 수정** (P0)
   - 파일: `src-tauri/src/mcp/builtin/content_store/server.rs:153`
   - 변경: `session_manager.get_current_session()` → `self.session_id`
   - 테스트: 세션 생성 후 Content Store context 확인

### Phase 2 (다음 스프린트)

2. **Playbook Context 구현** (P1)
   - 파일: `src-tauri/src/mcp/builtin/playbook/mod.rs:153`
   - 추가: `get_service_context()` 오버라이드
   - 테스트: 플레이북 생성 후 context에 표시되는지 확인

3. **Browser Context 구현** (P1)
   - 파일: `src-tauri/src/mcp/builtin/browser/mod.rs:77`
   - 추가: 활성 세션 정보 표시
   - 테스트: 브라우저 세션 생성 후 context 확인

---

## 🧪 테스트 케이스

### Content Store 수정 후 검증

**Test Case 1: 빈 Content Store**
```typescript
// 1. 새 세션 생성
const session = await createSession();

// 2. Service Context 확인
const contexts = await invoke('agent_get_service_contexts', { 
  sessionId: session.id 
});

// 3. 검증
expect(contexts.contentstore.context_prompt).toContain('No content stored yet');
expect(contexts.contentstore.structured_state.content_count).toBe(0);
```

**Test Case 2: 파일 추가 후**
```typescript
// 1. 파일 추가
await invoke('builtin_contentstore__saveKnowledge', {
  content: 'Test content',
  title: 'Test file'
});

// 2. Service Context 재확인
const contexts = await invoke('agent_get_service_contexts', { 
  sessionId: session.id 
});

// 3. 검증
expect(contexts.contentstore.context_prompt).toContain('1 file stored');
expect(contexts.contentstore.structured_state.content_count).toBe(1);
```

---

## 📈 예상 효과

### Content Store 수정 후

**Before**:
```text
## Content Store
**Status**: No active session
```
→ 에이전트가 Content Store 사용 불가능하다고 판단

**After**:
```text
## Content Store
**0 files stored**
*Use addContent to store files, documents, or text for later retrieval.*
```
→ 에이전트가 Content Store 사용 가능함을 인식하고 적극 활용

### Playbook 구현 후

**Before**:
```text
## Playbook
**Description**: Playbook management for reusable workflows
```
→ 플레이북 존재 여부 불명

**After**:
```text
## Playbook
**3 playbooks available**
**Recent:**
  - AI 데이터센터 뉴스 분석 워크플로우 (ID: a3f2b8c1)
  - 주간 시장 리포트 생성 프로세스 (ID: d4e9f1a2)
```
→ 에이전트가 기존 플레이북 재사용 가능

---

## 🎯 결론

**즉시 수정 필요 (Critical)**:
- Content Store의 세션 인식 실패는 **설계 불일치** 문제
- Planning/Workspace는 `self.session_id` 직접 사용하여 정상 작동
- Content Store만 `session_manager` 의존으로 실패
- **1줄 수정**으로 해결 가능: `self.session_id` 사용

**개선 권장 (Medium)**:
- Playbook Context 미구현은 **기능성 저하** 문제
- 에이전트의 학습/재사용 능력 제한
- 약 50줄 추가로 구현 가능

---

**분석일**: 2026-01-02  
**분석자**: GitHub Copilot  
**관련 문서**: `docs/analysis/service-context-integration-analysis.md`
