# LibrAgent 문서 감사: USER vs DEVELOPER 관점

> **감사일**: 2026-08-08
> **감사자**: QA & Test Engineer
> **대상**: guides/, features/, reference/, llm-services/, contributing/, analysis/ 디렉토리
> **총 파일 수**: 40개 파일 + 2개 하위디렉토리

---

## 1. 요약: 문서의 90%는 개발자용

| 항목             | 값                                                                 |
| ---------------- | ------------------------------------------------------------------ |
| 총 파일 수       | 40개                                                               |
| **사용자 문서**  | **2~3개 (5~7%)**                                                   |
| **개발자 문서**  | **37~38개 (93~95%)**                                               |
| 真正 사용자 문서 | `navigation-guide.md` (3.7KB)                                      |
| 준 사용자 문서   | `getting-started.md` (개발자용), `llm-services/README.md` (비교표) |

**결론**: `docs/` 디렉토리는 개발자 문서에는 충실하지만 **사용자 문서는 극도로 부족**합니다. `navigation-guide.md`가 현재 유일한 양호한 사용자 문서이며, 나머지는 거의 모두 개발자 전용 기술 문서입니다.

---

## 2. 디렉토리별 상세 스코어링

스코어링 기준:

- **User Accessibility (1-5)**: 비개발자가 이해할 수 있는가?
- **Actionability (1-5)**: 따라할 수 있는가?
- **Completeness (1-5)**: 핵심 정보가 부족한가?

### 2.1 guides/ (14개 파일)

| 파일                                         | 크기   | User Access | Action | Complete | 판별                                      |
| -------------------------------------------- | ------ | :---------: | :----: | :------: | ----------------------------------------- |
| `getting-started.md`                         | 2.1KB  |      2      |   4    |    4     | ❌ 개발자용 (Rust, pnpm, 시스템 의존성)   |
| `navigation-guide.md`                        | 3.7KB  |    **4**    | **4**  |  **4**   | ✅ **사용자 문서** (라우트 매핑, UX 설명) |
| `troubleshooting.md`                         | 3.6KB  |      2      |   3    |    3     | ⚠️ 혼합 (Linux/Dev/MCP 문제)              |
| `system-prompt-guide.md`                     | 681B   |      1      |   1    |    2     | ❌ 개발자용 (내부 프롬프트 규칙)          |
| `release-build-debugging.md`                 | 4.7KB  |      1      |   1    |    2     | ❌ 개발자용 (Tauri 빌드)                  |
| `release-build-troubleshooting.md`           | 7.1KB  |      1      |   1    |    2     | ❌ 개발자용 (릴리스 빌드 이슈)            |
| `persistent_shell_test_request.md`           | 13.0KB |      1      |   1    |    2     | ❌ 개발자용 (테스트 시나리오)             |
| `ui-resource-implementation.md`              | 9.3KB  |      1      |   1    |    2     | ❌ 개발자용 (Web MCP 구현)                |
| `agent-v2-frontend-integration.md`           | 35.6KB |      1      |   1    |    2     | ❌ 개발자용 (프론트엔드 통합 코드)        |
| `architecture-documentation-guide.md`        | 8.3KB  |      1      |   1    |    2     | ❌ 개발자용 (문서 구조 가이드)            |
| `session-scheduling-implementation-guide.md` | 14.1KB |      1      |   1    |    2     | ❌ 개발자용 (백엔드 구현)                 |
| `builtin-tool-comparison.md`                 | 29.2KB |      1      |   1    |    2     | ❌ 개발자용 (도구 비교)                   |
| `builtin_tool_bp.md`                         | 16.0KB |      1      |   1    |    2     | ❌ 개발자용 (최적 practices)              |
| `external-mcp-server-implementation.md`      | 29.7KB |      1      |   1    |    2     | ❌ 개발자용 (MCP 서버 구현)               |

**guides/ 분석**: 14개 중 **1개만** 사용자 문서 (`navigation-guide.md`). 나머지는 모두 빌드, 테스트, 구현 관련 개발자 문서입니다.

### 2.2 features/ (4개 파일 + 2개 하위디렉토리)

| 파일                                | 크기   | User Access | Action | Complete | 판별                                  |
| ----------------------------------- | ------ | :---------: | :----: | :------: | ------------------------------------- |
| `mcp-timeout-configuration.md`      | 4.3KB  |      2      |   3    |    3     | ⚠️ 설정 설명 (UI 경로 포함)           |
| `session-bookmarks.md`              | 5.4KB  |      2      |   2    |    3     | ❌ 개발자용 (DB 마이그레이션, 코드)   |
| `session-delete-options.md`         | 6.7KB  |      1      |   1    |    2     | ❌ 개발자용 (백엔드 구현)             |
| `skill-mention-reference-system.md` | 9.5KB  |      2      |   2    |    3     | ❌ 개발자용 (ReferenceResolver trait) |
| `bootstrap-server/`                 | 하위디 |      -      |   -    |    -     | 개발자용 (MCP 부트스트랩)             |
| `mcp-integration/`                  | 하위디 |      -      |   -    |    -     | 개발자용 (MCP 통합)                   |

**features/ 분석**: 4개 파일 모두 **개발자 구현 문서**입니다. `mcp-timeout-configuration.md`만 UI 경로를 언급하여 다소 사용자 친화적이지만, 여전히 설정 튜토리얼이 아닙니다.

### 2.3 reference/ (1개 파일)

| 파일                               | 크기   | User Access | Action | Complete | 판별                            |
| ---------------------------------- | ------ | :---------: | :----: | :------: | ------------------------------- |
| `claude-channel-implementation.md` | 13.5KB |      1      |   1    |    2     | ❌ 개발자용 (MCP 채널 프로토콜) |

**reference/ 분석**: 단일 파일이 개발자 구현 문서입니다. `reference`라는 이름이 주는 기대감(사용자 레퍼런스)과 달리 실제 내용은 MCP 채널 프로토콜 구현 상세입니다.

### 2.4 llm-services/ (10개 파일)

| 파일                        | 크기   | User Access | Action | Complete | 판별                               |
| --------------------------- | ------ | :---------: | :----: | :------: | ---------------------------------- |
| `README.md`                 | 9.9KB  |    **3**    | **3**  |    3     | ✅ **사용자 가능** (비교표, 개요)  |
| `OLLAMA_INTEGRATION.md`     | 7.1KB  |    **3**    | **4**  |    3     | ✅ **사용자 문서** (로컬 LLM 설정) |
| `anthropic.md`              | 23.1KB |      1      |   1    |    1     | ❌ SDK 문서 복사 (Anthropic 공식)  |
| `anthropic_message_smpl.md` | 9.2KB  |      1      |   1    |    1     | ❌ SDK 문서 복사                   |
| `anthropic_tooluse.md`      | 5.9KB  |      1      |   1    |    1     | ❌ SDK 문서 복사                   |
| `cerebras.md`               | 15.0KB |      1      |   1    |    1     | ❌ SDK 문서 복사 (Cerebras 공식)   |
| `firework.md`               | 5.3KB  |      1      |   1    |    1     | ❌ SDK 문서 복사                   |
| `gemini-ts-sdk.md`          | 12.2KB |      1      |   1    |    1     | ❌ SDK 문서 복사 (Google 공식)     |
| `groq-ts-sdk.md`            | 13.8KB |      1      |   1    |    1     | ❌ SDK 문서 복사 (Groq 공식)       |
| `openai.md`                 | 24.0KB |      1      |   1    |    1     | ❌ SDK 문서 복사 (OpenAI 공식)     |

**llm-services/ 분석**:

- **중요 문제**: 8개 파일(`anthropic.md`, `cerebras.md`, `openai.md`, `gemini-ts-sdk.md`, `groq-ts-sdk.md` 등)이 **각각의 SDK 공식 README를 복사한 것**입니다. LibrAgent 특화 내용이 거의 없습니다.
- **사용자 문서**: `README.md`(비교표)와 `OLLAMA_INTEGRATION.md`(로컬 LLM 설정)만 실제로 유용합니다.
- **권장**: SDK 문서는 삭제하고, LibrAgent에서의 사용법만 남기거나 외부 링크로 대체해야 합니다.

### 2.5 contributing/ (6개 파일)

| 파일                               | 크기  | User Access | Action | Complete | 판별                             |
| ---------------------------------- | ----- | :---------: | :----: | :------: | -------------------------------- |
| `coding-standards.md`              | 1.0KB |      1      |   1    |    2     | ❌ 개발자용 (인덴트, 네이밍)     |
| `github-release-notes-template.md` | 937B  |      1      |   1    |    2     | ❌ 개발자용 (릴리즈 노트 템플릿) |
| `open-source-launch-finale.md`     | 1.7KB |      1      |   1    |    2     | ❌ 개발자용 (OSS 런칭 실행계획)  |
| `product-messaging-guide.md`       | 7.4KB |      2      |   2    |    3     | ⚠️ 마케팅용 (사용자 대상 아님)   |
| `release-process.md`               | 2.0KB |      1      |   1    |    2     | ❌ 개발자용 (릴리즈 체크리스트)  |
| `testing.md`                       | 1.0KB |      1      |   1    |    2     | ❌ 개발자용 (테스트 전략)        |

**contributing/ 분석**: 6개 파일 모두 **기여자용**입니다. `product-messaging-guide.md`는 마케팅용 메시징 가이드로, 개발자용이지만 최종 사용자 대상 아님.

### 2.6 analysis/ (5개 파일)

| 파일                                      | 크기   | User Access | Action | Complete | 판별                         |
| ----------------------------------------- | ------ | :---------: | :----: | :------: | ---------------------------- |
| `api-session-first-message-hidden-bug.md` | 19.7KB |      1      |   1    |    1     | ❌ 개발자용 (버그 분석)      |
| `deep-code-analysis-report.md`            | 15.9KB |      1      |   1    |    1     | ❌ 개발자용 (코드 심층 분석) |
| `docker-first-message-deletion-bug.md`    | 18.6KB |      1      |   1    |    1     | ❌ 개발자용 (버그 분석)      |
| `product-analysis.md`                     | 31.2KB |      2      |   2    |    3     | ⚠️ 제품 분석 (경쟁사 비교)   |
| `technical-deep-dive.md`                  | 61.9KB |      1      |   1    |    1     | ❌ 개발자용 (기술 심층 분석) |

**analysis/ 분석**: 5개 파일 모두 **내부 분석/버그 리포트**입니다. `product-analysis.md`가 경쟁사 비교로 다소 유용하지만, 여전히 개발자/경영진 대상입니다.

---

## 3. 사용자 문서 격차 분석 (MISSING DOCS)

### 3.1 P0 — 반드시 필요한 사용자 문서 (누락)

| ID       | 제목                     | 설명                                                                       | 우선순위 |
| -------- | ------------------------ | -------------------------------------------------------------------------- | -------- |
| **P0-1** | **첫 대화 가이드**       | "LibrAgent로 첫 대화 시작하기" — 모델 연결, 첫 메시지 입력, 도구 호출 이해 | 🔴 필수  |
| **P0-2** | **플레이북 사용법**      | Playbook 생성/실행/공유 방법 (UI 스크린샷 포함)                            | 🔴 필수  |
| **P0-3** | **북마크/히스토리 관리** | 세션 북마크, 검색, 아카이브 사용법                                         | 🟡 중요  |
| **P0-4** | **FAQ**                  | "API 키는 어디에?", "MCP 서버는?", "세션이 사라졌어요" 등                  | 🟡 중요  |
| **P0-5** | **MCP 서버 연결 가이드** | UI를 통한 MCP 서버 추가/설정 (getting-started.md의 MCP 섹션 재구성)        | 🟡 중요  |

### 3.2 P1 — 개선이 필요한 기존 문서

| ID       | 제목                                    | 현재 문제                       | 개선 방향                                           |
| -------- | --------------------------------------- | ------------------------------- | --------------------------------------------------- |
| **P1-1** | `getting-started.md`                    | 개발자 환경 설정에 치중         | 사용자용 + 개발자용으로 분리                        |
| **P1-2** | `navigation-guide.md`                   | 좋은 시작점이지만 스크린샷 없음 | UI 캡처 이미지 추가                                 |
| **P1-3** | `troubleshooting.md`                    | Linux/Dev 문제 중심             | 일반 사용자 문제 (연결 실패, 에러 메시지) 추가      |
| **P1-4** | `llm-services/README.md`                | 비교표는 좋으나 사용법 없음     | 각.provider별 "LibrAgent에서 설정하는 법" 섹션 추가 |
| **P1-5** | `features/mcp-timeout-configuration.md` | 설정 UI 경로 포함               | "언제 변경해야 하나요?" 설명 추가                   |

### 3.3 P2 — 향후 개선 사항

| ID       | 제목                   | 설명                                          |
| -------- | ---------------------- | --------------------------------------------- |
| **P2-1** | **스크린샷 뱅크**      | `docs/assets/screenshots/`에 UI 캡처 저장     |
| **P2-2** | **비디오 튜토리얼**    | 5분 소개 영상 (YouTube embed)                 |
| **P2-3** | **다국어 지원**        | 영어/한국어 병기                              |
| **P2-4** | **사용자 피드백 루프** | 문서 하단에 "이 문서가 도움이 되었나요?" 버튼 |
| **P2-5** | **검색 최적화**        | docs/ 내에 검색 인덱스 구축                   |

---

## 4. 리소스 낭비 분석

### 4.1 llm-services/ — SDK 문서 복사 문제

| 파일                        | SDK 원본                | LibrAgent 특화 내용 | 권장 조치           |
| --------------------------- | ----------------------- | ------------------- | ------------------- |
| `anthropic.md`              | Anthropic SDK README    | 거의 없음           | 삭제 또는 외부 링크 |
| `cerebras.md`               | Cerebras SDK README     | 거의 없음           | 삭제 또는 외부 링크 |
| `openai.md`                 | OpenAI SDK README       | 거의 없음           | 삭제 또는 외부 링크 |
| `gemini-ts-sdk.md`          | Google GenAI SDK README | 거의 없음           | 삭제 또는 외부 링크 |
| `groq-ts-sdk.md`            | Groq SDK README         | 거의 없음           | 삭제 또는 외부 링크 |
| `firework.md`               | Fireworks SDK README    | 거의 없음           | 삭제 또는 외부 링크 |
| `anthropic_message_smpl.md` | Anthropic SDK           | 거의 없음           | 삭제                |
| `anthropic_tooluse.md`      | Anthropic SDK           | 거의 없음           | 삭제                |

**총 8개 파일, 약 100KB+가 SDK 문서 복사본** — LibrAgent 특화 콘텐츠가 거의 없습니다.

### 4.2 guides/ — 구현 문서 과다

| 파일                                         | 크기   | 용도                 |
| -------------------------------------------- | ------ | -------------------- |
| `agent-v2-frontend-integration.md`           | 35.6KB | 프론트엔드 통합 코드 |
| `builtin-tool-comparison.md`                 | 29.2KB | 도구 비교            |
| `external-mcp-server-implementation.md`      | 29.7KB | MCP 서버 구현        |
| `builtin_tool_bp.md`                         | 16.0KB | 베스트 프랙티스      |
| `session-scheduling-implementation-guide.md` | 14.1KB | 백엔드 구현          |
| `persistent_shell_test_request.md`           | 13.0KB | 테스트 시나리오      |

**6개 파일만 137.6KB** — guides/의 70%가 구현/테스트 문서입니다.

---

## 5. 제안 리구조화

### 5.1 새로운 디렉토리 구조

```
docs/
├── user/                    # NEW — 최종 사용자 대상
│   ├── getting-started.md   # 새 파일: 5분 시작 가이드
│   ├── first-conversation.md # 새 파일: 첫 대화
│   ├── playbooks.md         # 새 파일: 플레이북 사용법
│   ├── sessions.md          # 새 파일: 세션 관리 (북마크, 히스토리)
│   ├── mcp-servers.md       # 새 파일: MCP 서버 연결
│   ├── faq.md               # 새 파일: FAQ
│   └── troubleshooting.md   # 기존 troubleshooting.md 수정 (사용자 문제 중심)
│
├── guides/                  # 수정 — 개발자/기여자 대상
│   ├── architecture.md      # 기존 guides 통합 (간소화)
│   ├── contributing.md      # contributing/ 통합
│   └── troubleshooting-dev.md # 기존 troubleshooting (개발자 문제)
│
├── features/                # 유지 — 기능별 구현 문서
│   ├── session-bookmarks.md
│   ├── session-delete-options.md
│   ├── skill-mention-system.md
│   └── mcp-timeout.md
│
├── api/                     # NEW — API 레퍼런스
│   ├── tauri-commands.md
│   └── mcp-protocol.md
│
├── llm-services/            # 대폭 축소
│   ├── README.md            # 유지 (비교표)
│   └── ollama.md            # 유지 (LibrAgent 특화)
│   # 나머지 8개 파일: 삭제 또는 외부 링크로 대체
│
├── reference/               # 유지 (개발자용)
│   └── claude-channel.md
│
└── analysis/                # 유지 (내부 문서)
    ├── product-analysis.md
    └── technical-deep-dive.md
```

### 5.2 마이그레이션 계획 (4단계)

| 단계        | 기간 | 작업                                                      | 책임자              |
| ----------- | ---- | --------------------------------------------------------- | ------------------- |
| **Phase 1** | 1주  | P0-1 (첫 대화 가이드) + P0-4 (FAQ) 작성                   | Frontend Architect  |
| **Phase 2** | 2주  | llm-services/ 정리 (8개 파일 삭제/통합) + P0-2, P0-3 작성 | Technical Architect |
| **Phase 3** | 2주  | 디렉토리 리구조화 (user/ 생성, 기존 파일 이동)            | Technical Architect |
| **Phase 4** | 1주  | 스크린샷 뱅크 구축 + P1-2, P1-3 개선                      | QA Engineer         |

---

## 6. 핵심 지표

| 지표                    | 현재         | 목표 (3개월 후)  |
| ----------------------- | ------------ | ---------------- |
| 사용자 문서 비율        | **5~7%**     | **30~40%**       |
| 스크린샷 포함 문서      | **0%**       | **100%** (user/) |
| SDK 문서 복사           | **8개 파일** | **0개**          |
| P0 누락 문서            | **5개**      | **0개**          |
| 문서 평균 사용자 접근성 | **1.3/5**    | **3.5/5**        |

---

## 7. 결론

**LibrAgent의 문서 전략은 근본적인 재설계가 필요합니다.**

1. **사용자 문서가 5~7%** — 제품 문서의 압도적 다수가 개발자용입니다.
2. **llm-services/의 80%**가 SDK 문서 복사본 — LibrAgent 특화 내용이 없습니다.
3. **navigation-guide.md가 유일한 사용자 문서** — 스크린샷만 추가해도 P0급이 될 수 있습니다.
4. **첫 대화 가이드가 가장 시급한 격차** — 이 하나만 있어도 사용자 진입 장벽이 절반 이하로 떨어집니다.

**즉시 착수해야 할 것**: P0-1 (첫 대화 가이드) + P0-4 (FAQ) — 이 둘만 작성해도 문서의 사용자 가치가 근본적으로 바뀝니다.

---

_Report generated by QA & Test Engineer (ed235711-f0b8-4b4d-8048-103f63b73775)_
_Audit scope: guides/ (14), features/ (4+2), reference/ (1), llm-services/ (10), contributing/ (6), analysis/ (5)_
