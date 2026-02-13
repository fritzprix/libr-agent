# 오류 처리 통합 리팩토링 계획 (Built-in + External MCP)

**작성일:** 2026-02-12  
**브랜치:** `resolve-assistant-id-14444470464130912136`  
**범위:** Rust 백엔드 오류 처리 개선

- Built-in MCP 툴 서비스 프레임워크 (`src-tauri/src/mcp/builtin/**`)
- External MCP 서버 연동(stdio + HTTP) (`src-tauri/src/mcp/**`)

본 문서는 저장소 아키텍처 문서와 실제 코드 엔트리포인트 조사 결과(특히 `MCPServiceProxy`, external manager, builtin error helper 사용 패턴)를 바탕으로 작성되었습니다.

---

## 0) 목표와 비목표

### 목표

1. **에이전트가 복구 가능한 오류**: 모든 툴 실패는 “무슨 일이 있었는지”와 “다음에 무엇을 해야 하는지”를 함께 제공해야 합니다.
2. **일관성**: Built-in / External 툴 실패가 **에이전트가 읽는 텍스트 출력**에서 동일한 오류 형태(계약)를 갖도록 합니다.
3. **정규화(Normalization)**: External 오류(stdio/HTTP/protocol/remote-tool)를 소수의 안정적인 분류 체계로 매핑합니다.
4. **가드레일(Guardrails)**: 회귀를 방지합니다.
   - Recovery 힌트는 동일 ToolGroup 내 도구만 제안
   - 중요한 식별자(ID/handle)는 JSON이 아니라 텍스트(`content`)에 반드시 포함
5. **침묵(deprecated) 방지**: Deprecated 경로가 “빈 리스트 반환” 등으로 조용히 성공처럼 보이지 않도록 합니다.

### 비목표

- rmcp 자체에 대한 대규모 프로토콜 변경은 하지 않습니다.
- “Did you mean …” 같은 별칭/자동 추천(툴 aliasing)은 도입하지 않습니다. (Canonical naming 유지)
- 모든 툴을 한 번에 갈아엎지 않습니다. 단계별로 테스트/호환성을 유지하며 진행합니다.

---

## 1) 현 상태 요약 (근거)

### Built-in 툴

- 다수의 built-in 서비스가 이미 공유 오류 helper를 사용합니다:
  - `missing_param_error(...)`
  - `operation_failed_error(...)`
  - `not_found_error(...)`
  - `ToolGroup::*` 기반 스코핑

발견 위치 예:

- `src-tauri/src/mcp/builtin/ui/mod.rs`
- `src-tauri/src/mcp/builtin/browser/*`
- `src-tauri/src/mcp/builtin/playbook/*`
- `src-tauri/src/mcp/builtin/workspace/*`

**강점:** Workspace 등은 다층 검증 + 명확한 복구 가이드를 잘 구현.

**리스크:** 일관성이 “규칙/관습”에 의존(드리프트 위험). 성공/실패 응답 패턴도 도구마다 조금씩 차이가 남.

### External MCP

핵심 라우팅/엔트리포인트:

- `src-tauri/src/mcp/service_proxy.rs` (`MCPServiceProxy::call_tool`)
- `src-tauri/src/mcp/server/mod.rs`, `src-tauri/src/mcp/server/tools.rs`
- 세션 격리 매니저:
  - `src-tauri/src/mcp/session_isolation/http_manager.rs`
  - `src-tauri/src/mcp/session_isolation/stdio_manager.rs`

**리스크:** External 오류는 유형이 매우 다양(transport/protocol/remote tool error/HTTP 404 등). 명시적 정규화가 없으면 에이전트 복구가 불안정.

### Deprecated 글로벌 MCP 매니저 커맨드 경로

`src-tauri/src/commands/mcp_commands.rs`에서:

- deprecated global manager 경고 로그
- `println!` 사용
- `list_tools_from_config` deprecated + 빈 리스트 반환

**리스크:** 동일 기능이 여러 표면에서 다르게 동작 → 오류 처리/UX 분산. 특히 “빈 리스트”는 실패를 성공처럼 보이게 만들어 치명적.

---

## 2) 목표 계약: 에이전트가 보는 오류 텍스트 포맷

에이전트는 기본적으로 `content` 텍스트를 통해서만 상황을 이해합니다. `structured_content`는 UI 렌더링용이며 에이전트는 보지 못합니다.

### 필수 섹션

모든 툴 실패 메시지는 아래를 포함해야 합니다:

- **Operation**: 수행하려던 작업의 짧은 라벨
- **Source**: `Builtin(<service>)` 또는 `External(<server>)`
- **Category**: 안정적인 분류(아래 taxonomy)
- **Cause**: 한 줄 요약
- **Recovery**: 1..N개의 다음 행동(가능하면 복사-붙여넣기 가능한 호출 형태)

### 예시

```text
Operation: Call External Tool
Source: External(filesystem)
Category: Transport
Cause: failed to spawn stdio process (ENOENT)

Recovery:
- Verify the server command exists and is executable
- Restart the MCP server: startServer("filesystem")
- List tools to confirm availability: listTools("filesystem")
```

텍스트 우선(text-first) 설계이며, structured JSON은 필요 시 부가적으로 포함합니다.

### UI 계약(오류 그룹핑을 위한 필수 조건)

텍스트 포맷 표준화 외에도, UI가 아래 동작을 안정적으로 수행하려면 **기계적으로 판별 가능한 신호**가 필요합니다.

1. 실패한 tool result가 나타나면 새로운 그룹을 시작하고,
2. 연속으로 발생하는 실패 tool result들을 하나의 "오류 그룹"으로 묶어 경고/빨간(semantic) 스타일로 표시

이를 위해 채팅 메시지의 metadata 필드를 사용합니다.

- `message.role === 'tool'`
- 실패한 tool result 메시지에 대해 `message.metadata.toolError === true`

이 방식은 tool output 텍스트를 파싱하는(취약한) 휴리스틱을 피합니다.

**Backend-first 요구사항:** 백엔드가 실패한 tool result에 `metadata.toolError`를 설정하기 전까지, UI의 오류 그룹핑 동작은 활성화되지 않습니다.

#### 오류 2분류(#1/#2) 매핑 규칙

- **#1 에이전트/툴 복구 가능 오류** (잘못된 tool call, validation 실패, tool-side 로직 오류)

- 정상적인 tool result 메시지로 표현 (`role: 'tool'`)
- `message.metadata.toolError = true` 설정
- 상세한 복구 가이드는 tool result 텍스트에 포함(사용자가 tool call/result 맥락을 같이 볼 수 있음)
- 글로벌 "서비스 오류" UI로 승격하지 않음

- **#2 사용자만 복구 가능한 provider/system 오류** (LLM provider 통합 문제: tool-use JSON 깨짐, 빈 응답, 인증, rate limit 등)

- `Message.error` (또는 글로벌 error state)로 표현 + 간단한 taxonomy + Retry 제공
- `displayMessage`는 비기술적/간결한 설명 유지
- 항상 Retry 버튼 제공

---

## 3) 통합 분류 체계(Taxonomy)

### 제안 카테고리

#### 공통 / 일반

- `InvalidInput`
- `NotFound`
- `PermissionDenied`
- `InvalidState`
- `Internal`

#### External 전용

- `Transport` (spawn/connect/broken pipe)
- `Protocol` (예상치 못한 JSON-RPC / schema / decode)
- `RemoteToolError` (서버가 `isError=true`로 반환)
- `SessionExpired` (HTTP 404 / 세션 무효)
- `Timeout`

### 매핑

- Built-in의 `ErrorCategory::*`는 공통 카테고리로 매핑합니다.
- External 오류는 위 external 전용 카테고리로 정규화합니다.

---

## 4) 단계별 리팩토링 계획

### Phase 0 — 계약 + 포맷 유틸리티 추가 (동작 변경 없음)

**목표:** 공통 포맷터와 taxonomy 타입을 도입합니다.

#### Phase 0 산출물

- 오류 포맷 및 taxonomy 전용 모듈 추가 (후보: `src-tauri/src/mcp/errors/` 또는 `src-tauri/src/mcp/error_normalization.rs`).
- 함수 예:
  - `format_tool_error_text(...)`
  - `format_external_error_text(...)`

#### Phase 0 완료 기준

- 툴 로직 변경 없이 유틸만 추가.
- 입력만으로 목표 포맷의 오류 텍스트를 생성 가능.

---

### Phase 1 — Built-in 프레임워크 강화

**목표:** Built-in 툴이 계약을 안정적으로 준수하도록 일관성과 가드레일을 추가합니다.

#### Phase 1 작업 항목

1. **Built-in 오류 생성 경로 중앙화**
   - 기존 helper를 감싸는 “canonical” 오류 helper 도입 (텍스트 계약 표준화).
   - 기존 helper 함수는 호환성 유지, 호출부는 점진적 마이그레이션.

2. **가드레일: ToolGroup 힌트 격리**
   - 테스트/디버그에서 Recovery 힌트가 동일 ToolGroup 도구만 언급하는지 검증.
   - 단순 문자열 체크로도 충분.

3. **가드레일: ID는 텍스트에 포함**
   - structured JSON에만 존재하는 중요한 ID는 반드시 `content` 텍스트에도 포함.
   - 회귀 방지용 단위 테스트 추가.

4. **Service context 캐시 무효화 일관화**
   - 상태 변경이 발생하는 모든 함수에서 캐시 무효화 호출을 표준화.
   - Workspace의 좋은 패턴을 다른 영역에도 확장.

#### Phase 1 완료 기준

- Built-in 오류는 항상 Recovery 섹션을 포함.
- 테스트에서 cross-tool-group 힌트가 잡힘.

---

### Phase 2 — External MCP 오류 정규화

**목표:** stdio/HTTP/protocol/remote-tool 오류를 안정적인 taxonomy로 매핑하고, 계약에 맞는 텍스트를 생성합니다.

#### Phase 2 작업 항목

1. **정규화 레이어 추가**
   - (server, tool, error)를 받아 다음을 산출:
     - category
     - 표준 agent-visible 텍스트
     - optional structured JSON

2. **HTTP SessionExpired 정책(404/세션 무효)**
   - 중앙화된 bounded retry 정책:
     1. 세션 만료 감지
     2. 세션 무효 처리(mark invalid)
     3. reconnect/re-init 1회
     4. 원 작업 1회 재시도
     5. 실패 시 `SessionExpired`로 오류 반환 + Recovery 제공

3. **tool-not-found 복구 UX**
   - external tool-not-found 시:
     - 시도한 server/tool 명시
     - Recovery에 “해당 서버 tool list 조회” 포함

#### Phase 2 완료 기준

- 모든 external 오류 텍스트에 `(server, tool)`이 포함.
- SessionExpired는 bounded retry + 명확한 Recovery 제공.

---

### Phase 3 — Deprecated 커맨드 표면 정리(제거/격리)

**목표:** deprecated global MCP manager 경로가 조용히 성공처럼 보이지 않게 하고, 오류 처리를 일원화합니다.

#### Phase 3 선택지(하나 선택 필요)

- **A(브레이킹):** deprecated 커맨드 제거
- **B(호환):** deprecated 커맨드를 session-isolated 경로 + 정규화 레이어로 라우팅
- **C(안전):** deprecated 커맨드는 명확한 마이그레이션 오류를 반환(빈 리스트 금지)

#### Phase 3 공통 필수 수정

- `println!` 제거 → 로거로 통일
- deprecated placeholder로 “빈 리스트 반환” 금지

#### Phase 3 완료 기준

- deprecated MCP 커맨드는 성공처럼 보이는 빈 결과를 반환하지 않음.

---

### Phase 4 — 테스트

**목표:** 오류 텍스트를 에이전트용 “공개 API”로 취급하고 회귀를 방지합니다.

#### Phase 4 Built-in 테스트

- missing param → `Category:` + `Recovery:` 포함
- invalid ID → 동일 ToolGroup의 올바른 “list…” 힌트 포함
- 중요한 ID가 텍스트에 포함

#### Phase 4 External 테스트

- transport failure → `Transport`로 정규화
- tool-not-found → list-tools 복구 힌트 포함
- SessionExpired → 분류 + bounded retry 정책 검증

#### Phase 4 완료 기준

- 계약 위반 시 테스트 실패.

---

### Phase 5 — 검증 게이트

- `pnpm refactor:validate`
- mcp 관련 `cargo test`

#### Phase 5 완료 기준

- lint/build/tests 통과.

---

## 5) 구현에 영향을 주는 확인 질문

1. External 오류를 가능한 한 **tool-result 형태(MCPResult 에러)**로 변환해서 에이전트가 복구 행동을 할 수 있게 할까요, 아니면 **Rust `Err(String)`로 즉시 중단**할까요?

2. HTTP 서버의 re-init/reconnect는 rmcp 수준에서 공식적으로 지원되는 initialize/reinitialize 호출이 있나요? 없다면 “transport/client 재생성 + 1회 재시도” 방식으로 정책을 구현해도 될까요?

3. `src-tauri/src/commands/mcp_commands.rs` deprecated 경로는 **제거(A)**, **세션 격리 경로로 라우팅(B)**, **명시적 오류 반환(C)** 중 어느 쪽이 맞나요?

---

## 6) 추천 PR 단위(작게 쪼개기)

1. Phase 0: taxonomy + formatter 유틸 추가
2. Built-in 서버 1개(예: Workspace or UI)를 exemplar로 마이그레이션 + 테스트
3. External 정규화 레이어 + 테스트
4. 나머지 built-in 점진적 마이그레이션
5. deprecated 커맨드 정리
