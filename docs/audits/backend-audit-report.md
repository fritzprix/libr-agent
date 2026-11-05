# 백엔드 문서 감사 보고서 (Backend Docs Audit)

**감사일**: 2026-08-08  
**감사 대상**: docs/architecture/, docs/specs/, docs/tauri/, docs/fixes/, docs/improvements/, docs/mcp/  
**총 파일 수**: 49개  
**감사 관점**: USER(비개발자) vs DEVELOPER(개발자)

---

## 1. Executive Summary

### 전체 점수

| 지표                   |    점수     | 설명                                                         |
| ---------------------- | :---------: | ------------------------------------------------------------ |
| **사용자 접근성 평균** | **0.8 / 5** | 문서의 98% 이상이 개발자 전용                                |
| **실행 가능성 평균**   | **1.1 / 5** | 비개발자가 따라할 수 있는 단계가 없음                        |
| **충분함 평균**        | **2.3 / 5** | 개발자에게는 대체로 충분하나, 사용자 관점에서 핵심 정보 누락 |

### 핵심 발견

1. **사용자 문서의 전무**: 49개 중 비개발자가 이해할 수 있는 문서는 단 2개 (`agent-vibe-charter.md`, `open-source-launch-manifesto.md`)
2. **미정형 기록의 누적**: migration_status_update_v2/v3/v4 — 같은 내용의 3번 반복. V4에서 "Fully Compliant"라고 결론났지만 V2/V3는 삭제되지 않음
3. **개발자 전용 복사-붙여넣기**: `docs/tauri/dragdrop.md`는 Tauri 공식 문서의 복사본
4. **사용자 가이드 부재**: MCP 서버 연결, 스킬 사용, 에이전트 생성 등 사용자가 실제로 필요로 할 정보가 전무

---

## 2. 디렉토리별 상세 분석

### 2.1 docs/architecture/ (24개 파일)

**전체 사용자 접근성: 0.5 / 5 — 개발자 전용 100%**

| 파일명                                        | 사용자 접근성 | 실행 가능성 | 충분함 | 분류        | 비고                                                  |
| --------------------------------------------- | :-----------: | :---------: | :----: | ----------- | ----------------------------------------------------- |
| `agent-workflow-architecture.md` (28KB)       |       0       |      0      |   4    | 🛠 Dev      | Rust/React 분할, PlantUML 다이어그램, Tokio primitive |
| `codebase-feature-map.md` (32KB)              |       0       |      0      |   5    | 🛠 Dev      | 파일 경로 매핑. 개발자 탐색용                         |
| `external-mcp-integration.md` (34KB)          |       0       |      1      |   5    | 🛠 Dev      | Transport 레이어, Session Isolation, 코드 예제        |
| `builtin-tools-design-principles.md` (4KB)    |       1       |      0      |   3    | 🛠 Dev      | 도구 설계 원칙. 개발자 가이드라인                     |
| `builtin-tools-improvement-summary.md` (22KB) |       0       |      0      |   4    | 🛠 Dev      | Phase 1 구현 요약, 테스트 결과                        |
| `builtin-tools-evaluation.md` (21KB)          |       0       |      0      |   5    | 🛠 Dev      | 70+ 도구 평가, 준수도 점수                            |
| `ai-soul-manifesto.md` (3KB)                  |       3       |      1      |   3    | 📖 User+Dev | 에이전트 행동 원칙. **가장 사용자 친화적**            |
| `frontend-architecture.md` (3KB)              |       0       |      1      |   4    | 🛠 Dev      | React 컴포넌트 패턴, TypeScript 타입                  |
| `mcp-tool-response-design.md` (4KB)           |       0       |      1      |   4    | 🛠 Dev      | MCPResult 구조, structured_content 확장               |
| `message-context-management.md` (6KB)         |       0       |      0      |   4    | 🛠 Dev      | Rust 프론트엔드 책임 분할                             |
| `browser-error-structured-types.md` (4KB)     |       0       |      0      |   3    | 🛠 Dev      | BrowserError enum 설계. TS 미구현                     |
| `soul-lounge-recovery-loop.md` (7KB)          |       1       |      1      |   3    | 📖 User+Dev | 에이전트 회복 루프. 개념적 이해 가능                  |
| `gemini-caching-implementation.md` (8KB)      |       0       |      0      |   4    | 🛠 Dev      | 캐시 라이프타임, stable/volatile 프롬프트             |
| `session-lineage-and-tree-ui.md` (10KB)       |       0       |      0      |   4    | 🛠 Dev      | HTTP API, DB 엔티티, MCP client                       |
| `scheduled-group-work-plan.md` (5KB)          |       0       |      0      |   2    | 🗑️ Obsolete | **구현 안됨**. superseded 표시 있음                   |
| `rust-conventions.md` (4KB)                   |       0       |      1      |   3    | 🛠 Dev      | snake_case, Method vs Associated Function             |
| `push-notify-concurrency-impl.md` (11KB)      |       0       |      0      |   5    | 🛠 Dev      | SessionBus, Notify, Semaphore 구현                    |
| `session-cancel-isolation.md` (6KB)           |       0       |      0      |   4    | 🛠 Dev      | SP6, dual-notifier, AtomicBool                        |
| `workflow-settlement.md` (6KB)                |       0       |      0      |   4    | 🛠 Dev      | finish.rs, settle_before_terminal_transition          |
| `agent-hub-refactoring.md` (6KB)              |       0       |      0      |   3    | 🛠 Dev      | Tool deduplication, AgentHubServer                    |
| `agent-vibe-charter.md` (2KB)                 |       4       |      2      |   3    | 📖 User+Dev | **사용자 접근성 가장 높음**. 제품 성격 정의           |
| `open-source-launch-manifesto.md` (2KB)       |       4       |      2      |   3    | 📖 User+Dev | 프로젝트 비전, 커뮤니티 약속                          |
| `type-safety-guide.md` (5KB)                  |       0       |      1      |   4    | 🛠 Dev      | TypeScript 타입 안전 가이드. `any` 금지               |
| `text-loop-recovery.md` (6KB)                 |       0       |      0      |   3    | 🛠 Dev      | 텍스트 반복 감지, 재시도 카운터                       |

**사용자 문서로 전환 가능한 파일**: 3개

- `agent-vibe-charter.md` → "에이전트의 성격과 행동 원칙"으로 일반화 가능
- `ai-soul-manifesto.md` → "에이전트 리커버리 시스템"으로 사용자 설명 가능
- `soul-lounge-recovery-loop.md` → "에이전트가 헤맬 때 자동으로 회복하는 방법"으로 단순화 가능

**삭제 권장**: 1개

- `scheduled-group-work-plan.md` — 구현 안됨, superseded 표시 있음. 아카이브로 이동해야 함

---

### 2.2 docs/specs/ (7개 파일)

**전체 사용자 접근성: 0.7 / 5 — 개발자 전용 95%**

| 파일명                                 | 사용자 접근성 | 실행 가능성 | 충분함 | 분류        | 비고                                         |
| -------------------------------------- | :-----------: | :---------: | :----: | ----------- | -------------------------------------------- |
| `knowledge-server-v2-design.md` (6KB)  |       0       |      0      |   4    | 🛠 Dev      | sqlite-vec, fastembed-rs, RRF 알고리즘       |
| `message-compaction.md` (26KB)         |       0       |      0      |   5    | 🛠 Dev      | 토큰 체크포인트, split_idx, retry ladder     |
| `mcp-channel-support.md` (10KB)        |       0       |      1      |   4    | 🛠 Dev      | Claude Channels 프로토콜, JSON-RPC           |
| `sp1-sp2-concurrency-design.md` (16KB) |       0       |      0      |   5    | 🛠 Dev      | SessionBus, Notify, Semaphore — SP1/SP2      |
| `skill-mention-system.md` (6KB)        |       2       |      1      |   3    | 📖 User+Dev | @reference, /command 트리거. **사용자 관련** |
| `workspace.md` (3KB)                   |       1       |      1      |   3    | 🛠 Dev      | writeFile 스키마, Dual Channel 응답          |
| `bubbles.md` (1KB)                     |       2       |      1      |   2    | 📖 User+Dev | Think Bubble 스펙. UI 관련                   |

**사용자 문서로 전환 가능한 파일**: 2개

- `skill-mention-system.md` → `@파일참조`와 `/명령어` 사용법으로 일반 사용자 가이드 가능
- `bubbles.md` → "Think 표시 방식" 설명으로 UI 이해도 향상

---

### 2.3 docs/tauri/ (1개 파일)

**전체 사용자 접근성: 0 / 5 — 개발자 전용 100%**

| 파일명              | 사용자 접근성 | 실행 가능성 | 충분함 | 분류         | 비고                                                         |
| ------------------- | :-----------: | :---------: | :----: | ------------ | ------------------------------------------------------------ |
| `dragdrop.md` (1KB) |       0       |      1      |   1    | 🗑️ Duplicate | **Tauri 공식 문서 복사본**. 프로젝트 문서로 유지할 가치 없음 |

**권장**: 삭제 또는 Tauri 공식 문서 링크로 교체

---

### 2.4 docs/fixes/ (2개 파일)

**전체 사용자 접근성: 0.5 / 5 — 개발자 중심**

| 파일명                           | 사용자 접근성 | 실행 가능성 | 충분함 | 분류        | 비고                                               |
| -------------------------------- | :-----------: | :---------: | :----: | ----------- | -------------------------------------------------- |
| `windows-ps1-execution.md` (5KB) |       0       |      0      |   3    | 🛠 Dev      | PowerShell 명령 실행 버그 수정. AV 우회            |
| `thinking-display-fix.md` (4KB)  |       1       |      0      |   3    | 📖 User+Dev | Think 콘텐츠 표시 문제. **사용자가 궁금해할 내용** |

**개선 제안**: `thinking-display-fix.md`는 "에이전트의 사고 과정이 화면에 표시되지 않는 문제"로 일반화 가능. 버그 리포트에 포함 가능.

---

### 2.5 docs/improvements/ (3개 파일)

**전체 사용자 접근성: 0.3 / 5 — 개발자 중심**

| 파일명                                              | 사용자 접근성 | 실행 가능성 | 충분함 | 분류        | 비고                                              |
| --------------------------------------------------- | :-----------: | :---------: | :----: | ----------- | ------------------------------------------------- |
| `per-assistant-bundled-skills.md` (9KB)             |       0       |      0      |   4    | 🛠 Dev      | assistant별 스킬 할당. 파일 기반 선언형           |
| `workspace_output_visibility_fix.md` (2KB)          |       1       |      0      |   2    | 📖 User+Dev | 프로세스 출력 가시성 수정. **사용자가 겪을 문제** |
| `streaming-function-parsing-enhancements.md` (23KB) |       0       |      0      |   4    | 🛠 Dev      | 스트리밍 개선안. Ollama, OpenAI, Anthropic        |

**개선 제안**: `workspace_output_visibility_fix.md`는 "에이전트가 명령어 출력을 볼 수 없는 문제"로 단순화 가능. 사용자에게 유용한 정보.

---

### 2.6 docs/mcp/ (12개 파일)

**전체 사용자 접근성: 0.2 / 5 — 개발자 전용 100%**

| 파일명                                                  | 사용자 접근성 | 실행 가능성 | 충분함 | 분류        | 비고                               |
| ------------------------------------------------------- | :-----------: | :---------: | :----: | ----------- | ---------------------------------- |
| `claude-channels-dev-team-announcement.md` (9KB)        |       0       |      1      |   4    | 🛠 Dev      | MCP 서버 개발자 가이드             |
| `MCP_CONFIG_COMPARISON_ANALYSIS.md` (31KB)              |       0       |      0      |   5    | 🛠 Dev      | MCPConfig 구조 비교. TS vs Rust    |
| `builtin_tools_migration_report.md` (10KB)              |       0       |      0      |   4    | 🛠 Dev      | Rust vs TS 마이그레이션 검증       |
| `RUST_MCP_CONFIG_MIGRATION_STRATEGY.md` (29KB)          |       0       |      0      |   5    | 🛠 Dev      | 마이그레이션 전략, 하위 호환성     |
| `MCP_ACTIVATION_API_GUIDE.md` (11KB)                    |       0       |      1      |   4    | 🛠 Dev      | MCP 활성화 API 응답 스키마         |
| `claude-channels-mcp-server-reference.md` (25KB)        |       0       |      1      |   5    | 🛠 Dev      | MCP 서버 레퍼런스 가이드           |
| `API_RESPONSE_SCHEMA_FOR_USER_ACTIVATED_MCPs.md` (29KB) |       0       |      0      |   5    | 🛠 Dev      | API 응답 스키마 설계               |
| `claude-channels-dev-task-assignment.md` (4KB)          |       0       |      1      |   3    | 🛠 Dev      | 개발팀 작업 지시. P0/P1 작업       |
| `migration_status_update_v4.md` (6KB)                   |       0       |      0      |   4    | 📊 Status   | **V4: Fully Compliant**. 최종 결론 |
| `claude-channels-implementation-status.md` (6KB)        |       0       |      0      |   4    | 📊 Status   | 구현 상태 기록                     |
| `migration_status_update_v2.md` (3KB)                   |       0       |      0      |   2    | 🗑️ Obsolete | **구식**. V4에서 해결됨            |
| `migration_status_update_v3.md` (3KB)                   |       0       |      0      |   3    | 🗑️ Obsolete | **구식**. V4에서 해결됨            |

**중요 발견**: migration_status_update_v2.md, v3.md — V4에서 "모든 문제 해결"로 결론났지만 삭제되지 않음. 9KB의 구식 문서가 남아 있음.

---

## 3. 파일별 사용자 vs 개발자 분류

### 3.1 실제 사용자 접근 가능 파일 (총 6개)

| 파일                              | 사용자 접근성 | 설명                          |
| --------------------------------- | :-----------: | ----------------------------- |
| `agent-vibe-charter.md`           |      4/5      | 에이전트의 성격과 행동 원칙   |
| `open-source-launch-manifesto.md` |      4/5      | 프로젝트 비전과 커뮤니티 약속 |
| `ai-soul-manifesto.md`            |      3/5      | 에이전트 리커버리 원칙        |
| `soul-lounge-recovery-loop.md`    |      2/5      | 컨텍스트 이탈 방지 메커니즘   |
| `skill-mention-system.md`         |      2/5      | @참조 / /명령어 사용법        |
| `thinking-display-fix.md`         |      1/5      | Think 표시 문제 (일반화 가능) |

### 3.2 완전히 개발자 전용 파일 (총 43개)

- architecture/ 21개 (agent-vibe-charter, ai-soul-manifesto, soul-lounge 제외)
- specs/ 5개 (skill-mention-system, bubbles 제외)
- tauri/ 1개
- fixes/ 1개 (thinking-display-fix 제외)
- improvements/ 2개 (workspace_output_visibility_fix 제외)
- mcp/ 12개 전체

---

## 4. 누락된 사용자 문서

### 4.1 시급도 높은 사용자 문서 (필수)

| 문서                       | 설명                                        | 우선순위 |
| -------------------------- | ------------------------------------------- | :------: |
| **에이전트 사용 가이드**   | 에이전트 생성, 대화 시작, 세션 관리         |    P0    |
| **MCP 서버 연결 가이드**   | 외부 MCP 서버 연결 방법 (stdio/HTTP)        |    P0    |
| **빌트인 도구 사용법**     | planning, workspace, browser 도구 사용 예시 |    P0    |
| **스킬 시스템 안내**       | @참조, /명령어, 스킬 활성화                 |    P1    |
| **에이전트 상태 이해하기** | Idle/Running/Paused/Error 상태 설명         |    P1    |
| **세션 계보 이해하기**     | 부모/자식 세션, 트리 구조                   |    P1    |

### 4.2 시급도 중간 사용자 문서 (권장)

| 문서                         | 설명                           | 우선순위 |
| ---------------------------- | ------------------------------ | :------: |
| **에이전트 리커버리 시스템** | Soul Lounge, loop detection    |    P2    |
| **에이전트 성격 설정**       | Agent Vibe Charter 기반        |    P2    |
| **워크스페이스 구조**        | 파일 시스템, 작업 디렉토리     |    P2    |
| **스트리밍 표시 방식**       | Think Bubble, 메시지 버블      |    P2    |
| **프로세스 관리**            | 백그라운드 프로세스, 출력 읽기 |    P2    |

### 4.3 시급도 낮은 사용자 문서 (선택)

| 문서                    | 설명               | 우선순위 |
| ----------------------- | ------------------ | :------: |
| **피드백 및 버그 보고** | 사용자 피드백 경로 |    P3    |
| **자주 묻는 질문**      | 일반 사용자 FAQ    |    P3    |

---

## 5. 권장 리 구조

```
docs/
├── README.md                          # 문서 인덱스 (기존 유지)
│
├── user/                              # 🆕 사용자 문서 (NEW)
│   ├── getting-started.md             # 첫 설치, 첫 대화
│   ├── agent-management.md            # 에이전트 생성/설정/성격
│   ├── session-guide.md               # 세션 관리, 계보, 상태
│   ├── tool-usage.md                  # 빌트인 도구 사용법
│   ├── mcp-server-setup.md            # MCP 서버 연결
│   ├── skill-system.md                # 스킬, @참조, /명령어
│   ├── troubleshooting.md             # 문제 해결
│   └── faq.md                         # 자주 묻는 질문
│
├── developer/                         # 🆕 개발자 문서 (기존 docs/에서 이동)
│   ├── architecture/                  # 기존 docs/architecture/ 이동
│   ├── specs/                         # 기존 docs/specs/ 이동
│   ├── mcp/                           # 기존 docs/mcp/ 이동
│   ├── fixes/                         # 기존 docs/fixes/ 이동
│   └── improvements/                  # 기존 docs/improvements/ 이동
│
├── tauri/                             # 🆕 Tauri 관련 (기존 docs/tauri/ 이동)
│   └── dragdrop.md                    # 삭제 권장 (공식 문서 링크로 교체)
│
└── archive/                           # 🆕 아카이브 (새로 생성)
    ├── scheduled-group-work-plan.md   # 구현 안됨 → 아카이브
    ├── migration_status_update_v2.md  # 구식 → 아카이브
    └── migration_status_update_v3.md  # 구식 → 아카이브
```

### 5.1 이동/삭제/아카이브 대상

| 작업                  | 파일 수 | 대상                                                                                       |
| --------------------- | :-----: | ------------------------------------------------------------------------------------------ |
| **user/로 이동**      |   0개   | 새 문서 작성 필요                                                                          |
| **developer/로 이동** |  48개   | 기존 architecture/, specs/, mcp/, fixes/, improvements/                                    |
| **tauri/로 이동**     |   1개   | dragdrop.md (나중에 삭제)                                                                  |
| **archive/로 이동**   |   3개   | scheduled-group-work-plan.md, migration_status_update_v2.md, migration_status_update_v3.md |
| **삭제**              |   1개   | tauri/dragdrop.md (Tauri 공식 문서 링크로 교체)                                            |

---

## 6. 개선 우선순위 로드맵

### Phase 1: 정리 (1주)

- [ ] migration_status_update_v2.md, v3.md 삭제 (V4로 충분)
- [ ] scheduled-group-work-plan.md archive/로 이동
- [ ] tauri/dragdrop.md 삭제 또는 Tauri 공식 링크로 교체
- [ ] docs/ → docs/developer/ 구조 리팩토링

### Phase 2: 사용자 문서 작성 (2-3주)

- [ ] `docs/user/getting-started.md` — 첫 설치, 첫 대화
- [ ] `docs/user/agent-management.md` — 에이전트 생성, 설정, 성격
- [ ] `docs/user/tool-usage.md` — 빌트인 도구 사용 예시
- [ ] `docs/user/mcp-server-setup.md` — MCP 서버 연결 가이드

### Phase 3: 기존 문서 사용자화 (2주)

- [ ] `agent-vibe-charter.md` → `docs/user/agent-personality.md`로 일반화
- [ ] `skill-mention-system.md` → `docs/user/skill-system.md`로 단순화
- [ ] `thinking-display-fix.md` → `docs/user/troubleshooting.md`에 포함

---

## 7. 결론

### 현재 상태

| 항목                   | 점수  | 비고                                              |
| ---------------------- | :---: | ------------------------------------------------- |
| **전체 사용자 접근성** | 0.8/5 | 49개 중 6개만 제한적 사용자 접근 가능             |
| **실행 가능성**        | 1.1/5 | 비개발자가 따라할 수 있는 단계 없음               |
| **충분함**             | 2.3/5 | 개발자에게는 충분, 사용자 관점에서 핵심 정보 누락 |
| **문서 정리**          | 2.0/5 | 구식 파일 3개, 복사본 1개 방치                    |

### 핵심 문제

1. **사용자 문서의 전무**: 49개 중 비개발자가 이해할 수 있는 문서는 6개뿐
2. **개발자 중심의 과도한 기술 디테일**: PlantUML, Tokio primitives, JSON Schema가 일반 사용자에게 의미 없음
3. **구식 문서 누적**: migration_status_update_v2/v3는 V4에서 해결됨. 삭제 또는 아카이브 필요
4. **복사본 문서**: tauri/dragdrop.md는 Tauri 공식 문서의 복사본

### 권장 조치

1. **즉시**: migration_status_update_v2.md, v3.md 삭제
2. **1주 내**: docs/ → docs/developer/ 구조 리팩토링
3. **2-3주 내**: docs/user/ 디렉토리 생성 및 핵심 사용자 문서 6개 작성
4. **지속**: 새 기능 추가 시 반드시 사용자 문서 동반 작성

---

_감사 완료: 2026-08-08_
