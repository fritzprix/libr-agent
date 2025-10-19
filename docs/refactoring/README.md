# Switch Context Removal Refactoring Documentation

파라미터 기반 session/assistant/thread context 관리로 전환하기 위한 종합 가이드입니다.

## 📚 문서 구조

### 1. [`switch-context-removal-refactoring.md`](./switch-context-removal-refactoring.md)

**개요**: 리팩토링의 전략, 이유, 전체 로드맵

- **Executive Summary**: `switch_context` 문제점 및 해결 방안
- **Current Problem Analysis**: 각 도구의 현재 상태 분석
  - planning-server (Web MCP)
  - playbook-store (Web MCP)
  - content_store (Rust Backend)
  - workspace (Rust Backend)
- **Critical Design Principle**: Tool Definition 분리 (⭐ 가장 중요)
- **5 Phase Refactoring Outline**: 단계별 구현 계획
- **Migration Path & Compatibility**: 호환성 유지 전략
- **Risk Assessment**: 위험 평가

**읽기 순서**: 📌 첫 번째 (전체 그림 이해)

---

### 2. [`tool-implementation-guide.md`](./tool-implementation-guide.md)

**개요**: Backend type별 도구 구현 패턴 상세 가이드

- **Web MCP Servers** (TypeScript)
  - General Pattern & Checklist
  - Context 추출, State 저장, Args 정제
  - Memory Management (TTL/LRU)
  - playbook-store 실제 예제

- **Rust Built-in Servers** (Rust)
  - General Pattern & Checklist
  - ToolContext 구조, Session-scoped storage
  - workspace 실제 예제
  - Concurrent request handling

- **Comparison Table**: Web MCP vs Rust 비교
- **Common Patterns**: 공통 패턴들
- **Testing Strategy**: 테스트 전략
- **FAQ**: 자주 묻는 질문

**읽기 순서**: 📌 두 번째 (구현 방식 이해)

---

### 3. [`before-after-examples.md`](./before-after-examples.md)

**개요**: 실제 코드 변경사항을 Before/After로 비교

- **Web MCP: planning-server**
  - 현재 코드 (switch_context 기반)
  - 리팩토링된 코드 (파라미터 기반)
  - 개선 사항 명시

- **Rust: content_store**
  - 현재 코드 (SessionManager 의존)
  - 리팩토링된 코드 (session-scoped storage)
  - 개선 사항 명시

- **Frontend: use-tool-processor**
  - Context 주입 로직 추가
  - AI args 보존 원칙

- **핵심 차이점 요약**: Before/After 비교표
- **Testing 예제**: TypeScript & Rust 테스트
- **Migration Order**: 구현 순서

**읽기 순서**: 📌 세 번째 (실제 코드 학습)

---

## 🎯 Quick Start

### 당신의 역할에 따라:

#### Backend Engineer (Rust)

1. [`switch-context-removal-refactoring.md`](./switch-context-removal-refactoring.md) 읽기
   - 문제점 이해 (Phase 3 집중)
2. [`tool-implementation-guide.md`](./tool-implementation-guide.md) - "Rust Built-in Servers" 섹션
   - ToolContext, session-scoped storage 패턴
3. [`before-after-examples.md`](./before-after-examples.md) - "Rust: content_store"
   - 실제 코드 변경 예제

#### Frontend Engineer (TypeScript)

1. [`switch-context-removal-refactoring.md`](./switch-context-removal-refactoring.md) - "Critical Design Principle"
   - Tool Definition 분리 원칙
2. [`tool-implementation-guide.md`](./tool-implementation-guide.md) - "Web MCP Servers" 섹션
   - Context 추출, state isolation 패턴
3. [`before-after-examples.md`](./before-after-examples.md) - "Frontend: use-tool-processor"
   - Context 주입 로직

#### Full Stack / Architect

1. 모든 문서를 순서대로 읽기
2. Phase별 진행 계획 수립
3. Team sync 진행

---

## 🔑 핵심 개념 (5분 요약)

### 문제: Stateful Backend

```
Before (Current):
  Frontend → switchContext(sessionId=A) → Backend sets global state
           → callTool(args) → uses global state
           → switchContext(sessionId=B) → changes global state (⚠️ Race condition!)
           → callTool(args) → wrong session!
```

### 해결: Parameter-based Context

```
After (Refactored):
  Frontend → callTool(args + __sessionId=A) → Backend extracts session A
           → callTool(args + __sessionId=B) → Backend extracts session B
           → No global state! ✅ Safe concurrent requests
```

### Critical Constraint: Tool Definition 분리

```
Tool Definition (AI sees this):
  { "name": "create_goal", "inputSchema": { "goal": string } }
  ❌ NO __sessionId, __assistantId

Runtime Args (Backend uses this):
  { "goal": "Learn Rust", "__sessionId": "sess_1", "__assistantId": "asst_1" }
  ✅ Context injected by middleware, not visible to AI
```

---

## 📋 Implementation Checklist

### Phase 1: Type Changes ✅

- [ ] ServiceContextOptions interface 검토 (이미 존재)
- [ ] Tool definition 유지 (변경 없음)

### Phase 2: Frontend (use-tool-processor.ts)

- [ ] `__sessionId`, `__assistantId` 주입 로직 추가
- [ ] AI args 보존 (untouched)
- [ ] Tool definition 미변경 확인

### Phase 3: Web MCP Servers

- [ ] `planning-server.ts`: switchContext 제거, context 추출 추가
- [ ] `playbook-store.ts`: switchContext 제거, assistantId 권한 검사 추가
- [ ] Memory management (TTL/LRU) 구현
- [ ] 테스트

### Phase 4: Rust Builtin Servers

- [ ] `BuiltinMCPServer` trait에서 switch_context 제거
- [ ] `content_store/server.rs`: Session-scoped storage로 변경
- [ ] `workspace/mod.rs`: Context 기반 process isolation 추가
- [ ] 테스트 (single/multi-session/concurrent)

### Phase 5: Testing & Deployment

- [ ] 단일 세션 테스트 통과
- [ ] 다중 세션 테스트 통과
- [ ] 동시 요청 테스트 통과
- [ ] 배포 전 complete validation (`pnpm refactor:validate`)

---

## 🧪 Testing Strategy

### Web MCP Tests

```typescript
// Tool state isolation by session
expect(state1.goals).not.toContain(state2Goal);

// Permission checks
expect(selectPlaybook(asst_2)).toReject();

// Memory cleanup (TTL)
// Verify old states removed after TTL
```

### Rust Tests

```rust
// Session-scoped storage
assert_eq!(storage["sess_1"].len(), 1);
assert_eq!(storage["sess_2"].len(), 1);

// Access control
assert!(is_same_session(process, context));

// Concurrent requests
tokio::join!(
  handle_add_content(sess_1),
  handle_add_content(sess_2)
);
```

---

## 🚨 Common Pitfalls

### ❌ Pitfall 1: Context fields in Tool Definition

```typescript
// WRONG!
{
  "inputSchema": {
    "properties": {
      "goal": { "type": "string" },
      "__sessionId": { "type": "string" }  // ❌ AI will see this!
    }
  }
}

// RIGHT!
{
  "inputSchema": {
    "properties": {
      "goal": { "type": "string" }
      // ✅ Only AI-visible fields
    }
  }
}
// Context injected by middleware (AI doesn't know)
```

### ❌ Pitfall 2: Modifying AI args

```typescript
// WRONG!
const cleanedArgs = {
  ...args,
  goal: args.goal?.trim(), // ❌ Modified!
};

// RIGHT!
const cleanedArgs = {
  ...args,
};
delete cleanedArgs.__sessionId; // Remove only infrastructure fields
```

### ❌ Pitfall 3: Memory leaks (Web MCP)

```typescript
// WRONG!
const stateMap = new Map(); // No cleanup!

// RIGHT!
setInterval(
  () => {
    // TTL-based cleanup
    for (const [key, entry] of stateMap.entries()) {
      if (now - entry.lastAccessedAt > TTL_MS) {
        stateMap.delete(key);
      }
    }
  },
  5 * 60 * 1000,
);
```

### ❌ Pitfall 4: Missing context extraction (Rust)

```rust
// WRONG!
let session_id = session_manager.get_current_session()?; // ❌ Global state!

// RIGHT!
let context = ToolContext::from_args(&args);
let session_id = &context.session_id; // ✅ From args
```

---

## 📞 Support & Questions

### FAQ 참고

[`tool-implementation-guide.md`](./tool-implementation-guide.md#7-faq) 참고

### 일반적인 질문들:

**Q: \_\_ prefix 필드가 Tool Definition에 나타나면?**
A: 안 됩니다! Tool schema에서 제거해야 합니다.

**Q: Session 없이 호출되면?**
A: `'default'` session 사용. 로깅으로 모니터링.

**Q: 같은 session에 대한 동시 요청?**
A: `Arc<Mutex<>>`/Map으로 thread-safe 보장.

**Q: assistantId 없는 요청?**
A: `context.assistantId.is_none()` 체크. Permission optional.

---

## 📈 Success Metrics

리팩토링 완료 후 확인할 사항:

- ✅ 단일 세션: 모든 도구 정상 작동
- ✅ 다중 세션: State 격리 확인
- ✅ 동시 요청: Race condition 없음
- ✅ 메모리: Leak 없음 (Web MCP TTL 동작)
- ✅ Tool Definition: Context fields 없음
- ✅ Permission: assistantId 기반 검사 작동
- ✅ Performance: 성능 저하 없음

---

## 🔗 Related Documentation

- [`docs/architecture/chat-feature-architecture.md`](../architecture/chat-feature-architecture.md) - Chat flow 전체
- [`docs/builtin-tools.md`](../builtin-tools.md) - Built-in tool 목록
- [`src-tauri/src/mcp/builtin/README.md`](../../../src-tauri/src/mcp/builtin/README.md) - Rust backend 구조
- [`src/lib/web-mcp/README.md`](../../../src/lib/web-mcp/README.md) - Web MCP 구조

---

## 📝 References

**파일 위치**:

- Frontend: `src/hooks/use-tool-processor.ts`
- Web MCP: `src/lib/web-mcp/modules/`
- Rust Backend: `src-tauri/src/mcp/builtin/`

**Key Types**:

- Frontend: `ServiceContextOptions`, `MCPResponse`
- Rust: `ToolContext`, `BuiltinMCPServer` trait

---

## 🎓 Learning Path

```
Week 1: Understanding
  ├─ Read switch-context-removal-refactoring.md (전략)
  ├─ Read tool-implementation-guide.md (패턴)
  └─ Review current code (planning-server.ts, content_store/server.rs)

Week 2: Implementation
  ├─ Frontend: use-tool-processor.ts 수정
  ├─ Web MCP: planning-server.ts, playbook-store.ts 수정
  ├─ Rust: content_store, workspace 수정
  └─ Unit tests 작성

Week 3: Testing & Validation
  ├─ Integration tests
  ├─ Concurrent request tests
  ├─ Performance tests
  └─ pnpm refactor:validate 통과

Week 4: Deployment
  ├─ Code review
  ├─ Staging test
  └─ Production deployment
```

---

Generated: 2025-10-19 | Architecture: Parameter-based Context Management
