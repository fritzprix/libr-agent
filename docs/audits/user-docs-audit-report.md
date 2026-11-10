# LibrAgent Frontend/User-Facing Documentation Audit

> **작성일**: 2026-08-07  
> **작성자**: Frontend Architect (Sub-Agent)  
> **대상**: LibrAgent 문서군 (`docs/` 전체)

---

## 1. Executive Summary

LibrAgent는 **사용자 문서가 심각하게 부족**한 상태입니다. 현재 `docs/`의 50개 이상 파일 중 실제로 비개발자가 앱을 사용하는법을 설명하는 파일은 **2개**에 불과합니다. 나머지 95% 이상은 개발자용 아키텍처/구현 문서입니다.

| 지표             | 값                                               |
| ---------------- | ------------------------------------------------ |
| 총 문서 수       | 50+                                              |
| 사용자 대상 문서 | 2 (navigation-guide.md, getting-started.md 일부) |
| 개발자 대상 문서 | 48+                                              |
| 사용자 문서 비율 | ~4%                                              |

---

## 2. Existing Docs Audit (1-5 Scale)

### 2.1 `docs/guides/` (14 files)

| #   | 파일                                         | User Accessibility | Actionability | Completeness | 비고                                                                        |
| --- | -------------------------------------------- | :----------------: | :-----------: | :----------: | --------------------------------------------------------------------------- |
| 1   | `getting-started.md`                         |       **2**        |     **3**     |    **3**     | 개발자 환경 설정 중심. API 키 관리, MCP 서버 연결은 포함하나 UI 가이드 없음 |
| 2   | `navigation-guide.md`                        |       **4**        |     **4**     |    **4**     | **최고의 사용자 문서**. UI 라우트→기능 매핑이 명확함                        |
| 3   | `troubleshooting.md`                         |       **2**        |     **2**     |    **2**     | 소스 코드 라인 참조(`src-tauri/src/lib.rs Lines 188-250`)가 많아 개발자용   |
| 4   | `system-prompt-guide.md`                     |       **1**        |     **1**     |    **1**     | AI 에이전트용 가이드. 사용자와 무관                                         |
| 5   | `agent-v2-frontend-integration.md`           |       **1**        |     **1**     |    **1**     | 35KB 개발자 아키텍처 문서                                                   |
| 6   | `architecture-documentation-guide.md`        |       **1**        |     **1**     |    **1**     | 개발자 문서                                                                 |
| 7   | `builtin-tool-comparison.md`                 |       **1**        |     **1**     |    **1**     | 29KB 개발자 문서                                                            |
| 8   | `builtin_tool_bp.md`                         |       **1**        |     **1**     |    **1**     | 16KB 개발자 문서                                                            |
| 9   | `external-mcp-server-implementation.md`      |       **1**        |     **1**     |    **1**     | 30KB 구현 가이드                                                            |
| 10  | `persistent_shell_test_request.md`           |       **1**        |     **1**     |    **1**     | 테스트 요청 문서                                                            |
| 11  | `release-build-debugging.md`                 |       **1**        |     **1**     |    **1**     | 빌드 디버깅 가이드                                                          |
| 12  | `release-build-troubleshooting.md`           |       **1**        |     **1**     |    **1**     | 빌드 트러블슈팅                                                             |
| 13  | `session-scheduling-implementation-guide.md` |       **1**        |     **1**     |    **1**     | 14KB 구현 가이드                                                            |
| 14  | `ui-resource-implementation.md`              |       **1**        |     **1**     |    **1**     | 9KB 구현 가이드                                                             |

**guides/ 평균 점수**: Accessibility 1.4, Actionability 1.5, Completeness 1.5

### 2.2 `docs/features/` (6 files + 2 subdirs)

| #   | 파일                                           | User Accessibility | Actionability | Completeness | 비고                                                      |
| --- | ---------------------------------------------- | :----------------: | :-----------: | :----------: | --------------------------------------------------------- |
| 1   | `mcp-timeout-configuration.md`                 |       **3**        |     **4**     |    **3**     | "Usage" 섹션이 있음. UI 경로(⚙️ → Advanced)가 명시됨      |
| 2   | `session-bookmarks.md`                         |       **2**        |     **2**     |    **2**     | 백엔드/프론트엔드 구현 상세. UX 디자인 섹션만 사용자 관련 |
| 3   | `session-delete-options.md`                    |       **2**        |     **2**     |    **2**     | SP7 구현 문서. UI 레이아웃 다이어그램만 사용자 관련       |
| 4   | `skill-mention-reference-system.md`            |       **2**        |     **3**     |    **3**     | `@skill:name` 사용법 설명이 있으나 구현 상세가 압도적     |
| 5   | `bootstrap-server/implementation-summary.md`   |       **1**        |     **1**     |    **1**     | 구현 요약                                                 |
| 6   | `mcp-integration/rust-backend-architecture.md` |       **1**        |     **1**     |    **0**     | **0 bytes** — 빈 파일                                     |

**features/ 평균 점수**: Accessibility 1.8, Actionability 2.2, Completeness 1.8

### 2.3 `docs/reference/` (1 file)

| #   | 파일                               | User Accessibility | Actionability | Completeness | 비고                    |
| --- | ---------------------------------- | :----------------: | :-----------: | :----------: | ----------------------- |
| 1   | `claude-channel-implementation.md` |       **1**        |     **1**     |    **1**     | 13.5KB 개발자 구현 문서 |

### 2.4 `docs/llm-services/` (10 files)

| #    | 파일      | User Accessibility | Actionability | Completeness | 비고                                                                              |
| ---- | --------- | :----------------: | :-----------: | :----------: | --------------------------------------------------------------------------------- |
| 1-10 | 모든 파일 |       **1**        |     **2**     |    **2**     | LLM Provider 통합 가이드 (Anthropic, OpenAI, Gemini, Groq 등). 모두 개발자/설정용 |

**llm-services/ 평균 점수**: Accessibility 1.0, Actionability 2.0, Completeness 2.0

---

## 3. Missing User Documentation Report

### 🔴 P0 (필수 — 즉시 작성)

| 우선순위 | 제목                 | 설명                                                                 | 예상 분량        |
| :------: | -------------------- | -------------------------------------------------------------------- | ---------------- |
|   P0-1   | **5분 시작 가이드**  | 앱 설치 → 첫 에이전트 대화까지. 스크린샷 포함, 개발 환경 설정 불필요 | 2-3페이지        |
|   P0-2   | **에이전트 첫 대화** | 세션 생성, 프롬프트 입력, 에이전트 응답 이해. UI 요소 설명 포함      | 2페이지          |
|   P0-3   | **모델 연결하기**    | Settings에서 API 키 설정, Provider 선택, 모델 변경. 스크린샷 기반    | 1-2페이지        |
|   P0-4   | **기능별 튜토리얼**  | 브러우징, 파일 작업, 터미널, MCP 서버 연결 등 핵심 기능 사용법       | 각 1페이지 × 5개 |

### 🟡 P1 (개선 — 1-2주 내)

| 파일명                              | 문제점                                                                | 개선안                                                                                                              |
| ----------------------------------- | --------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `getting-started.md`                | 개발자 환경 설정이 주내용. 일반 사용자는 `pnpm tauri dev`가 필요 없음 | **2개 버전으로 분리**: (1) `docs/user/getting-started.md` — 일반 사용자용, (2) `docs/developer/setup.md` — 개발자용 |
| `troubleshooting.md`                | 소스 코드 참조가 많아 접근성 낮음                                     | **사용자 관점 재작성**: 증상 → 원인 → 해결 (소스 코드 참조 제거)                                                    |
| `navigation-guide.md`               | 좋음. 하지만 스크린샷이 없음                                          | **스크린샷 추가**, 실제 UI 캡처로 보강                                                                              |
| `mcp-timeout-configuration.md`      | 구현 상세가 압도적                                                    | **사용자 섹션만 추출**하여 별도 문서로 이동                                                                         |
| `session-bookmarks.md`              | 구현 문서. UX 설명이 흩어져 있음                                      | **사용자 가이드 추출**: "세션 북마크 사용법"                                                                        |
| `session-delete-options.md`         | 구현 문서                                                             | **사용자 가이드 추출**: "세션 삭제 및 관리"                                                                         |
| `skill-mention-reference-system.md` | `@skill:name` 사용법이 구현 상세에 묻혀 있음                          | **사용자 가이드 추출**: "스킬 참조 (`@skill:name`) 사용법"                                                          |
| `llm-services/*.md`                 | 10개 파일 모두 구현/설정 중심                                         | **사용자용 통합 가이드**: "LLM Provider 설정 가이드" (1개로 통합)                                                   |

### 🟢 P2 (향후 — 여유 시)

| 제목                       | 설명                      |
| -------------------------- | ------------------------- |
| `faq/common-questions.md`  | 일반적으로 묻는 질문 모음 |
| `faq/error-codes.md`       | 에러 코드 및 해결책       |
| `scenarios/code-review.md` | 코드 리뷰 시나리오        |
| `scenarios/research.md`    | 리서치 시나리오           |
| `scenarios/automation.md`  | 자동화 시나리오           |
| `glossary.md`              | 용어 사전                 |

---

## 4. Proposed User Documentation Structure

```
docs/
├── README.md                          # 기존 유지 — 개발자용 색인
├── user/                              # 🔑 신규: 사용자 중심 문서
│   ├── README.md                      # 사용자 문서 시작점
│   │
│   ├── getting-started/               # 🚀 시작
│   │   ├── install.md                 # 설치 방법 (데스크톱 앱)
│   │   ├── 5-minute-tutorial.md       # 5분 시작 가이드 (핵심!)
│   │   ├── first-agent.md             # 첫 에이전트 대화
│   │   └── connecting-models.md       # 모델 연결 및 API 키 설정
│   │
│   ├── guides/                        # 📖 사용 가이드
│   │   ├── agent-workflow.md          # 에이전트 작동 방식 이해
│   │   ├── mcp-servers.md             # MCP 서버 연결 및 관리
│   │   ├── automation.md              # 예약 작업 (Scheduled Tasks)
│   │   ├── skills.md                  # 스킬 (`@skill:name`) 사용법
│   │   ├── sessions.md                # 세션 관리 (북마크, 삭제, 히스토리)
│   │   ├── assistants.md              # 어시스턴트 프로필 관리
│   │   ├── playbooks.md               # 플레이북 사용법
│   │   └── troubleshooting.md         #常见问题 해결 (사용자 관점)
│   │
│   ├── faq/                           # ❓ 자주 묻는 질문
│   │   ├── common-questions.md        # 일반 FAQ
│   │   └── error-codes.md             # 에러 코드 및 해결책
│   │
│   └── scenarios/                     # 🎯 시나리오 기반 튜토리얼
│       ├── code-review.md             # 코드 리뷰 자동화
│       ├── research.md                # 리서치 및 요약
│       ├── file-management.md         # 파일 관리 자동화
│       └── web-browsing.md            # 웹 브라우징 자동화
│
├── guides/                            # 🔄 리팩토링: 개발자 전용으로 전환
│   ├── getting-started-dev.md         # 개발자 환경 설정 (기존 getting-started.md에서 분리)
│   ├── navigation-guide.md            # 유지 — 개발자용 라우트 매핑으로 수정
│   ├── troubleshooting-dev.md         # 개발자 트러블슈팅 (기존 troubleshooting.md에서 분리)
│   ├── system-prompt-guide.md         # 유지
│   ├── agent-v2-frontend-integration.md
│   ├── architecture-documentation-guide.md
│   ├── builtin-tool-comparison.md
│   ├── builtin_tool_bp.md
│   ├── external-mcp-server-implementation.md
│   ├── persistent_shell_test_request.md
│   ├── release-build-debugging.md
│   ├── release-build-troubleshooting.md
│   ├── session-scheduling-implementation-guide.md
│   └── ui-resource-implementation.md
│
├── features/                          # 🔄 리팩토링: 구현 문서로 명시
│   ├── mcp-timeout-configuration.md
│   ├── session-bookmarks.md
│   ├── session-delete-options.md
│   ├── skill-mention-reference-system.md
│   ├── bootstrap-server/
│   └── mcp-integration/
│
├── reference/                         # 유지
└── llm-services/                      # 🔄 리팩토링: 개발자 설정 가이드로 통합
    ├── provider-setup.md              # 신규: 모든 Provider 설정을 1개로 통합
    └── (기존 파일은 archives/로 이동 또는 삭제)
```

---

## 5. Migration Plan

### Phase 1: 신규 사용자 문서 생성 (1주)

| 순서 | 작업                                                  | 담당 | 기간 |
| :--: | ----------------------------------------------------- | ---- | :--: |
|  1   | `docs/user/README.md` 생성                            | FE   | 1일  |
|  2   | `docs/user/getting-started/5-minute-tutorial.md` 작성 | FE   | 2일  |
|  3   | `docs/user/getting-started/first-agent.md` 작성       | FE   | 1일  |
|  4   | `docs/user/getting-started/connecting-models.md` 작성 | FE   | 1일  |
|  5   | 스크린샷 캡처 및 삽입                                 | FE   | 1일  |

### Phase 2: 기존 문서 리팩토링 (2주)

| 순서 | 작업                                  | 내용                                                       |
| :--: | ------------------------------------- | ---------------------------------------------------------- |
|  1   | `getting-started.md` → 2개 분리       | 사용자용 (`docs/user/`) + 개발자용 (`docs/guides/`)        |
|  2   | `troubleshooting.md` → 2개 분리       | 사용자용 (`docs/user/guides/`) + 개발자용 (`docs/guides/`) |
|  3   | `navigation-guide.md` 수정            | 개발자 관점 재작성 + 스크린샷 추가                         |
|  4   | `features/` 문서에서 사용자 섹션 추출 | `docs/user/guides/`로 이동                                 |
|  5   | `llm-services/` 통합                  | `docs/llm-services/provider-setup.md`로 통합               |

### Phase 3: 보충 문서 작성 (2-3주)

| 순서 | 작업                                   | 내용                                                 |
| :--: | -------------------------------------- | ---------------------------------------------------- |
|  1   | `docs/user/guides/` 나머지 가이드 작성 | agent-workflow, mcp-servers, automation 등           |
|  2   | `docs/user/faq/` 생성                  | common-questions, error-codes                        |
|  3   | `docs/user/scenarios/` 생성            | code-review, research, file-management, web-browsing |
|  4   | `docs/user/glossary.md` 생성           | 용어 사전                                            |

### Phase 4: 최종 정리 (1주)

| 순서 | 작업                         | 내용                |
| :--: | ---------------------------- | ------------------- |
|  1   | `docs/README.md` 업데이트    | user/ 색인 추가     |
|  2   | 모든 문서 간 cross-link 확인 | 내부 링크 갱신      |
|  3   | 스크린샷 일관성 확인         | UI 버전 매칭        |
|  4   | 최종 리뷰                    | QA 및 스티어링 리뷰 |

---

## 6. Priority Matrix

```
                    중요도
                  높       중간
       빠름  [P0-1]    [P1-1]
       시간  [P0-2]    [P1-2]
       ↓     [P0-3]    [P1-3]
       ↓     [P0-4]    [P1-4]
```

**즉시 착수**: P0-1 (5분 시작 가이드) — 이 하나만 있어도 사용자 진입 장벽이 절반 이하로 떨어짐.

---

## 7. Key Insights

1. **navigation-guide.md가 현재 최고의 사용자 문서** — 스크린샷을 추가하면 P0급이 될 수 있음.
2. **getting-started.md는 개발자/사용자 혼재** — 분리해야 함. 사용자는 `pnpm tauri dev`가 필요 없다.
3. **features/ 문서의 UX 다이어그램이 유용** — 구현 상세는 유지하되, 사용자 가이드는 추출해야 함.
4. **llm-services/ 10개 파일은 통합 필요** — 사용자가 10개 Provider 문서를 읽을 일이 없음.
5. **스크린샷이 가장 큰 격차** — 모든 사용자 문서에 실제 UI 캡처가 필요.

---

## 8. Next Actions

1. **[FE]** `docs/user/getting-started/5-minute-tutorial.md` 작성 (P0-1)
2. **[FE]** 앱 스크린샷 캡처 (Settings, Chat, History, MCP Servers 화면)
3. **[FE]** `docs/user/README.md` 생성
4. **[PM]** 위 migration plan 리뷰 및 할당

---

_Audit completed by Frontend Architect. All scores are subjective estimates based on document content analysis._
