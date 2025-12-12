# 🎯 LibrAgent 도구 응답 개선 체크리스트

> **작성일**: 2025-12-12  
> **기반**: `idea.md` 철학 - "도구 응답은 AI 에이전트의 자율적 작업 수행 효율을 극대화해야 한다"

---

## 📋 사용 방법

각 항목의 **답변란**에 다음과 같이 작성해주세요:

- ✅ **선택함** / ❌ **선택 안함** / ⏸️ **보류**
- 우선순위: `P0` (최우선), `P1` (높음), `P2` (중간), `P3` (낮음)
- 메모: 추가 의견이나 특별 요구사항

---

## 📦 카테고리 1: 인프라 레벨 개선 (Foundation)

### 1.1 가이드 기능 추가 (`mcp-response-utils.ts`)

**설명:**

- `createGuidedToolResult()` 함수 구현
- `nextSteps`, `tips`, `relatedTools` 옵션 지원
- 기존 함수와 호환성 유지

**예상 효과:**

- 모든 모듈에서 일관된 가이드 제공 가능
- 에이전트의 다음 행동 결정 시간 단축

**난이도:** ⭐ (낮음)

**답변란:**

```
선택: X
우선순위:
메모: overcomplication 하지말자
```

---

### 1.2 에러 응답 템플릿 시스템 구축

**설명:**

- 공통 에러 유형별 템플릿 정의
  - `NotFound`: "리소스를 찾을 수 없음" → "list_xxx 도구로 확인 권장"
  - `InvalidParam`: "파라미터 오류" → "올바른 형식 예시 제공"
  - `PermissionDenied`: "권한 부족" → "필요한 권한 획득 방법 안내"
- 각 템플릿에 자동 가이드 포함

**예상 효과:**

- 에러 메시지의 일관성과 유용성 향상
- 에러 복구율 증가

**난이도:** ⭐⭐ (중간)

**답변란:**

```
선택: X
우선순위:
메모: 이렇게 균일하게 유형화될 수 없어.
```

---

### 1.3 도구 연결성 메타데이터 추가

**설명:**

- 각 도구 스키마에 `relatedTools` 필드 추가
- 런타임에서 자동으로 관련 도구 제안
- 예: `write_file` 성공 → `read_file`로 검증 제안

**예상 효과:**

- AI 에이전트의 도구 조합 학습 향상
- 워크플로우 자동 최적화

**난이도:** ⭐⭐⭐ (높음)

**답변란:**

```
선택: X
우선순위:
메모: 이것은 MCP 비표준이야.
```

---

## 🔧 카테고리 2: 모듈별 응답 메시지 개선 (Content)

### 2.1 assistant-manager 응답 개선

**개선 항목:**

- `create_assistant` 성공 시: "어시스턴트가 생성되었습니다. `update_assistant`로 설정 변경 가능" 안내
- `get_assistant` 실패 시: "`list_assistants`로 가용 ID 확인" 제안
- `update_assistant` 성공 시: "변경사항 확인을 위해 `get_assistant` 호출" 권장
- `delete_assistant` 성공 시: "영구 삭제되었으며 복구 불가능" 경고

**예상 효과:**

- 어시스턴트 CRUD 작업 흐름 개선
- 도구셋 내 완결성 유지 (assistant 관리만 담당)

**난이도:** ⭐

**주의:**

- ⚠️ MCP 서버 연결은 `mcp-manager` 도구셋의 역할이므로 여기서 언급하지 않음 (도구셋 완결성 원칙)

**답변란:**

```
선택:
우선순위:
메모:
```

---

### 2.2 bootstrap-server 응답 개선

**개선 항목:**

- `check_tool_installed` 미설치 시: "`get_bootstrap_guide(tool='...')` 사용 권장"
- `get_bootstrap_guide` 성공 시: "설치 후 `check_tool_installed`로 검증" 안내
- `detect_platform` 성공 시: "이 정보를 바탕으로 적합한 가이드 선택 가능" 명시

**예상 효과:**

- 환경 설정 자동화 효율 향상
- 설치 검증 누락 방지

**난이도:** ⭐

**답변란:**

```
선택: O
우선순위: P0
메모: 특이 사항 없음
상태: 완료 (2025-12-12)
```

---

### 2.3 knowledge-server 응답 개선

**개선 항목:**

- `save_knowledge` 성공 시: "저장한 태그로 검색 가능: [태그 목록]" 표시
- `search_knowledge` 결과 없음 시: "키워드 확장 또는 `list_knowledge`로 최근 항목 확인" 제안
- `delete_knowledge` 성공 시: "영구 삭제되었으며 복구 불가능" 경고

**예상 효과:**

- 지식 저장/검색 효율성 증가
- 데이터 손실 방지

**난이도:** ⭐

**답변란:**

```
선택: X
우선순위:
메모:
```

---

### 2.4 mcp-manager 응답 개선 ⚠️ 최우선 + 아키텍처 이슈

**현재 문제점:**

1. AI 에이전트가 MCP 서버를 "설치/실행"하려고 시도하는 오류 빈번
2. 실제로는 **MCP config 정보를 registry에 추가**하는 것이 목적
3. **`connect_server`의 `scope` 개념이 무의미함**
   - 현재: Assistant scope vs Global scope로 나뉨
   - 문제: 모든 서버는 기본적으로 전체 접근 가능해야 하며, 특정 assistant 제한은 UI 설정에서 관리되어야 함
   - 결과: AI 에이전트가 프로그래밍 방식으로 매핑을 관리하는 것은 불필요한 복잡성

**개선 항목:**

- `create_server` 성공 시:

  ```
  ✅ MCP 서버 등록 완료 (Registry에 저장됨)

  [중요] 이 도구는 서버 설치/실행 도구가 아닙니다.
  - 역할: 이미 설치된 MCP 서버의 연결 정보(stdio command, HTTP endpoint)를 registry에 등록
  - 등록된 서버는 기본적으로 모든 어시스턴트에서 접근 가능
  - 특정 어시스턴트 제한은 UI 설정에서 관리

  [주의] MCP 서버 자체 설치는 별도로 필요 (npm install, pip install 등)
  ```

- `list_servers` 개선:

  ```
  📋 등록된 MCP 서버 목록

  [설명] 이 목록은 registry에 등록된 서버들입니다.
  - 모든 서버는 기본적으로 사용 가능
  - UI 설정에서 특정 어시스턴트의 접근을 제한할 수 있음
  ```

- 도구 설명(description) 개선:

  ```
  create_server: "Register MCP server connection config to registry (NOT for installation/execution)"
  list_servers: "List registered MCP servers (all are accessible by default)"
  ```

- **아키텍처 개선 제안** (선택 사항):
  - `connect_server`/`disconnect_server` 도구 제거 또는 deprecated 처리
  - Assistant-Server 매핑은 UI 설정에서만 관리
  - AI 에이전트는 registry 관리(CRUD)만 담당

**예상 효과:**

- "MCP 서버 설치/실행" 오해 제거 (가장 빈번한 에러)
- 불필요한 scope/매핑 관리 복잡성 제거
- Registry 개념 명확화
- UI 설정과 프로그래밍 API 역할 분리

**난이도:** ⭐⭐ (메시지 개선) / ⭐⭐⭐ (아키텍처 개선)

**답변란:**

```
선택: ✅
우선순위: P0 (최우선)
메모: scope 개념 제거 필요. 모든 서버는 기본 전체 접근 가능하고, UI에서 제한 설정하는 구조로 변경 검토 필요
상태: 완료 (2025-12-12)
```

---

### 2.5 planning-server 응답 개선

**개선 항목:**

- `create_goal` 성공 시: "`add_todo`로 구체적인 단계로 세분화 권장"
- `add_todo` 성공 시: "의존성 관리: `dependsOn` 활용 팁 제공"
- `mark_todo` 완료 시: "다음 할 일: 미완료 todo 중 우선순위 높은 것 확인"

**예상 효과:**

- 계획 수립 품질 향상
- 태스크 관리 효율성 증가

**난이도:** ⭐

**답변란:**

```
선택: O
우선순위: P0
메모:` mark_todo` 시 해당 단계의 결과물을 scratchpad 등에 저장해 놓으면 나중에 기억할 수 있어 좋다는 팁을 명시
상태: 완료 (2025-12-12)
```

---

### 2.6 playbook-store 응답 개선

**현재 문제:**

- `show_playbooks` 도구 설명이 불충분하여 AI Agent가 자동화 시나리오에서 오용 가능

**개선 항목:**

- `show_playbooks` 도구 설명(description) 개선:

  ```
  현재: "Display playbooks with interactive UI (includes HTML UI resource for frontend, pauses agent)"

  개선안: "Display playbooks in interactive UI for USER selection. This tool pauses the agent and waits for user to click 'Select' button. DO NOT use this for autonomous execution - use 'list_playbooks' + 'get_playbook' instead."
  ```

**예상 효과:**

- AI Agent의 자동화 시나리오에서 `show_playbooks` 오용 방지

**난이도:** ⭐

**답변란:**

```
선택: ✅
우선순위: P1
메모: show_playbooks 도구 설명만 개선. 나머지는 이미 잘 작동 중
상태: 완료 (2025-12-12)
```

---

### 2.7 ui-tools 응답 개선

**개선 항목:**

- `visualize_data` 성공 시: "차트 해석 팁: 최대값, 평균값 자동 계산" 제공
- `prompt_user` 사용 시: "응답 대기 중 타임아웃 없음. 사용자 응답 대기" 명시
- `wait_for_user_resume` 성공 시: "사용자가 재개할 때까지 대기" 상태 표시

**예상 효과:**

- UI 인터랙션 신뢰도 향상
- 사용자 대기 상태 명확화

**난이도:** ⭐

**답변란:**

```
선택: X
우선순위:
메모: 해당 사항 없음
```

---

### 2.8 workspace 파일 도구 응답 개선 (Rust)

**개선 항목:**

- `read_file` 실패 시: "`list_directory`로 파일 구조 확인" 제안

**예상 효과:**

- 파일 작업 오류 복구율 향상
- 파일 시스템 탐색 효율 증가

**난이도:** ⭐⭐

**답변란:**

```
선택: O
우선순위:  P1
메모:  특이 사항 없음
상태: 완료 (2025-12-12)
```

---

### 2.9 workspace 터미널 도구 응답 개선 (Rust)

**개선 항목:**

- `execute_shell` 비동기 실행 시: "`poll_process(process_id='...')`로 상태 확인" 안내
- `poll_process` 실패 시: "프로세스 종료됨. `list_processes`로 활성 프로세스 확인" 제안
- `read_process_output` 성공 시: "전체 출력이 필요하면 `lines` 파라미터 증가" 팁

**예상 효과:**

- 백그라운드 작업 모니터링 개선
- 비동기 실행 신뢰성 향상

**난이도:** ⭐⭐

**답변란:**

```
선택: O
우선순위:  P0
메모:  특이 사항 없음
상태: 완료 (2025-12-12)
```

---

### 2.10 브라우저 도구 (✅ 이미 완료됨 - 참고용)

**현재 상태:**

- ✅ `extractWebContent` → `readWebContent` 의존성 명시됨
  - 성공 응답: "Use readWebContent(sessionId, page) to read more." (ExtractContentTool.ts#L203-L207)
  - 에러 응답: "No content found for this session. Please call extractWebContent first." (ReadContentTool.ts#L58-L61)
- ✅ 모든 에러에 actionable guidance 포함 (`error-utils.ts`)
  - SESSION_NOT_FOUND → "use `listSessions` or `createSession`"
  - ELEMENT_NOT_FOUND → "use `listInteractable` to see available elements"
  - NAVIGATION_FAILED → "check URL format, try `getPageTitle`"

**모범 사례 요소:**

1. **도구 간 의존성 관리**: ContentStore 기반 extract → read 파이프라인
2. **구조화된 에러 처리**: browser-error.ts로 표준화된 에러 타입 (BrowserErrorCode enum)
3. **에러별 복구 가이드**: getGuidanceForError() 함수로 각 에러 유형별 구체적 next-step 제시
4. **빈 페이지 감지**: 콘텐츠 추출 실패 시 자동으로 saveRawHtml 옵션 안내

**참고 코드:**

- `src/features/tools/browser-tools/error-utils.ts`: 에러 가이드 템플릿 구현
- `src/features/tools/browser-tools/content-store.ts`: 세션 기반 상태 관리
- `src/features/tools/browser-tools/browser-error.ts`: 구조화된 에러 타입

**답변란:**

```text
선택: N/A (이미 idea.md 철학 구현 완료)
우선순위: N/A
메모: 다른 도구 개선 시 브라우저 도구를 참조 모델로 사용할 것
```
