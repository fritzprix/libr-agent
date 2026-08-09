# 📋 LibrAgent Documentation Audit — Master Report

> **감사일**: 2026-08-08  
> **감사자**: Coding Expert (Hub) + 5 Sub-Agents (Divide-Conquer)  
> **대상**: `docs/` 전체 (143개 파일, 27개 디렉토리)

---

## 🎯 Executive Summary

| 지표                   | 값                 |
| ---------------------- | ------------------ |
| **총 문서 수**         | 143개 파일         |
| **사용자 문서**        | **2~3개 (1-2%)**   |
| **개발자 문서**        | **140개 (98-99%)** |
| **사용자 접근성 평균** | **0.8-1.4 / 5**    |
| **실행 가능성 평균**   | **1.1-1.5 / 5**    |

**결론**: LibrAgent의 `docs/`는 **개발자가 개발자를 위한 문서**만 가득합니다. 사용자가 찾는 "이거 뭐하는 앱이야?", "어떻게 시작하지?", "이 기능으로 뭘 할 수 있어?" 같은 내용이 전혀 없습니다.

---

## 🔴 P0 — 즉시 작성해야 할 사용자 문서 (5개)

| ID       | 제목                 | 설명                                                                 | 우선순위 |
| -------- | -------------------- | -------------------------------------------------------------------- | -------- |
| **P0-1** | **5분 시작 가이드**  | 앱 설치 → 첫 에이전트 대화까지. 스크린샷 포함, 개발 환경 설정 불필요 | 🔴 필수  |
| **P0-2** | **에이전트 첫 대화** | 세션 생성, 프롬프트 입력, 에이전트 응답 이해. UI 요소 설명 포함      | 🔴 필수  |
| **P0-3** | **모델 연결하기**    | Settings에서 API 키 설정, Provider 선택, 모델 변경. 스크린샷 기반    | 🔴 필수  |
| **P0-4** | **기능별 튜토리얼**  | 브러우징, 파일 작업, 터미널, MCP 서버 연결 등 핵심 기능 사용법       | 🔴 필수  |
| **P0-5** | **FAQ 문서**         | "API 키는 어디에?", "MCP 서버는?", "세션이 사라졌어요" 등            | 🟡 중요  |

---

## 🟡 P1 — 기존 문서 개선 (8개)

| 파일명                              | 문제점                                                                    | 개선안                                                                                                              |
| ----------------------------------- | ------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `getting-started.md`                | 개발자 환경 설정이 주내용. 일반 사용자는 `pnpm tauri dev`가 필요 없음     | **2개 버전으로 분리**: (1) `docs/user/getting-started.md` — 일반 사용자용, (2) `docs/developer/setup.md` — 개발자용 |
| `troubleshooting.md`                | 소스 코드 라인 참조(`src-tauri/src/lib.rs Lines 188-250`)가 많아 개발자용 | **사용자 관점 재작성**: 증상 → 원인 → 해결 (소스 코드 참조 제거)                                                    |
| `navigation-guide.md`               | 좋음. 하지만 스크린샷이 없음                                              | **스크린샷 추가**, 실제 UI 캡처로 보강                                                                              |
| `mcp-timeout-configuration.md`      | 구현 상세가 압도적                                                        | **사용자 섹션만 추출**하여 별도 문서로 이동                                                                         |
| `session-bookmarks.md`              | 구현 문서. UX 설명이 흩어져 있음                                          | **사용자 가이드 추출**: "세션 북마크 사용법"                                                                        |
| `session-delete-options.md`         | 구현 문서                                                                 | **사용자 가이드 추출**: "세션 삭제 및 관리"                                                                         |
| `skill-mention-reference-system.md` | `@skill:name` 사용법이 구현 상세에 묻혀 있음                              | **사용자 가이드 추출**: "스킬 참조 (`@skill:name`) 사용법"                                                          |
| `llm-services/*.md`                 | 10개 파일 모두 구현/설정 중심, 8개는 SDK 공식 문서 복사본                 | **사용자용 통합 가이드**: "LLM Provider 설정 가이드" (1개로 통합)                                                   |

---

## 🟢 P2 — 향후 개선 사항 (6개)

| 제목                       | 설명                      |
| -------------------------- | ------------------------- |
| `faq/common-questions.md`  | 일반적으로 묻는 질문 모음 |
| `faq/error-codes.md`       | 에러 코드 및 해결책       |
| `scenarios/code-review.md` | 코드 리뷰 시나리오        |
| `scenarios/research.md`    | 리서치 시나리오           |
| `scenarios/automation.md`  | 자동화 시나리오           |
| `glossary.md`              | 용어 사전                 |

---

## 📁 Proposed Directory Structure (Audience-based)

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
│   ├── persistent-shell-test-request.md
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

## 📊 Audit Scores by Directory

| 디렉토리             | User Access. | Actionability | Completeness | 파일 수 |
| -------------------- | :----------: | :-----------: | :----------: | :-----: |
| `docs/guides/`       |     1.4      |      1.5      |     1.5      |   14    |
| `docs/features/`     |     1.8      |      2.2      |     1.8      |    6    |
| `docs/reference/`    |     1.0      |      1.0      |     1.0      |    1    |
| `docs/llm-services/` |     1.0      |      2.0      |     2.0      |   10    |
| `docs/architecture/` |     0.8      |      1.1      |     2.3      |   24    |
| `docs/specs/`        |     0.7      |      1.0      |     2.5      |    7    |
| `docs/mcp/`          |     0.2      |      0.5      |     2.0      |   12    |

**navigation-guide.md가 현재 최고 점수 (4/5)** — 스크린샷 추가 시 P0급 가능.

---

## 📝 Migration Plan (4 Phases)

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

## 🎯 Priority Matrix

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

## 📦 Sub-Agent Reports

| 세션                               | 보고서                                | 파일 수 | 핵심 발견                              |
| ---------------------------------- | ------------------------------------- | :-----: | -------------------------------------- |
| Frontend Architect (65fba1d167)    | `docs/user/docs-audit-report.md`      |   50+   | 사용자 문서 2개만 존재 (4%)            |
| QA Engineer (c6005ed777)           | `docs/audit/frontend-audit-report.md` |   40    | llm-services/ 8개 파일 SDK 복사본      |
| Rust Backend Engineer (d2c3c4f4bd) | `docs/audit/backend-audit-report.md`  |   49    | migration_status_update_v2/v3 obsolete |
| MCP Specialist (9c0145387e)        | —                                     |    —    | MCP 서버 연결 가이드 필요              |
| Frontend Architect (54c72a15bc)    | —                                     |    —    | UI 리소스 전달 파이프라인 검증         |

---

## ✅ Next Actions

1. **[FE]** `docs/user/getting-started/5-minute-tutorial.md` 작성 (P0-1)
2. **[FE]** 앱 스크린샷 캡처 (Settings, Chat, History, MCP Servers 화면)
3. **[FE]** `docs/user/README.md` 생성
4. **[BE]** migration_status_update_v2.md, v3.md 삭제 (V4로 충분)
5. **[BE]** scheduled-group-work-plan.md archive/로 이동
6. **[BE]** tauri/dragdrop.md 삭제 (Tauri 공식 문서 링크로 교체)

---

_이 문서는 divide-conquer 패턴으로 5개 서브 에이전트가 병렬 감사한 결과를 hubsing 합니다._
