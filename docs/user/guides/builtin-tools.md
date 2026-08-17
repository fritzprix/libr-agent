---
title: 내장 도구 레퍼런스 (Built-in MCP Tools)
---

# 내장 도구 레퍼런스 (Built-in MCP Tools)

> LibrAgent는 별도의 외부 MCP 서버 설치 없이도 고성능 코딩, 파일 조작, 웹 탐색, 계획 수립, 미디어 분석, 백그라운드 예약 등을 곧바로 수행할 수 있는 **Rust 기반 단일통합 내장 MCP 서버 세트**를 기본 제공합니다.

모든 내장 도구는 통일된 `{server}__{tool}` 형식의 명명 규칙을 따릅니다.

---

## 📌 기본 도구 (Core) vs 선택 도구 (Optional) 구분

LibrAgent 내장 도구는 **모든 세션에 기본으로 활성화되는 코어 도구**와 **어시스턴트 설정에서 필요에 따라 켜고 끌 수 있는 선택 도구**로 구분됩니다.

### 1️⃣ 기본 활성화 도구 (Core Built-ins — 항상 사용 가능)

어시스턴트 생성 시 기본 적용되며, 에이전트의 기본적인 작업 수행 및 UI 상호작용에 필수적인 도구입니다.

- **`workspace__*`**: 작업 공간 파일 읽기/쓰기/라인편집, 디렉터리 조회, 터미널 명령어 실행
- **`ui__*`**: 대화형 인터랙티브 카드 렌더링 및 결과 보고 (`presentInteractive`, `reportResult`)
- **`agent__*`**: 서브 에이전트 자율 생성 및 멀티 에이전트 오케스트레이션
- **`skills__*`**: 스킬 실행 및 컨텍스트 로딩
- **`playbook__*`**: 자동화 워크플로우 플레이북 목록 조회/실행/저장
- **`attachments__*`**: 세션 첨부 파일 관리 및 검색
- **`scheduled_task__*`**: 백그라운드 타이머 및 Cron 주기적 예약 작업 관리
- **`scratchpad__*`**: 추론 과정 기록 및 임시 계산 노트
- **`tool__*`**: 도구 디스커버리 및 시스템 도구 관리

### 2️⃣ 선택 활성화 도구 (Optional Built-ins — 어시스턴트 설정에서 선택)

특정 목적(웹 탐색, 미디어 파싱, 세부 계획 수립 등)을 위해 어시스턴트 설정(**Assistants → Edit → Tools**)에서 켜거나 끌 수 있는 도구입니다.

- **`media__*`**: 이미지/시각 매체 및 오디오 매체 분석 (`seeContent`, `listenContent`)
- **`browser__*`**: 헤드리스 브라우저 웹 탐색, 클릭, 텍스트 입력, 스크린샷
- **`planning__*`**: 다단계 계획 수립(`createGoal`), 단계별 상태 업데이트, 실패 성찰(`reflect`)
- **`knowledge__*`**: 시맨틱 지식 저장 및 영구 메모리 검색
- **`setup-wizard__*`**: 파이썬/Node/uv 런타임 자동 진단 및 환경 설치 마법사
- **`history__*`**: 이전 세션 이력 조회

---

## 🛠️ 내장 도구 상세 기능 레퍼런스

### 1. Workspace (`workspace__*`)

| 도구 이름                               | 설명                                        | 주요 파라미터                                     |
| :-------------------------------------- | :------------------------------------------ | :------------------------------------------------ |
| `workspace__readFile`                   | 파일 내용 읽기                              | `path`, `offset`, `size`                          |
| `workspace__writeFile`                  | 파일 생성, 덮어쓰기, 또는 append            | `path`, `mode`, `content`                         |
| `workspace__strReplace`                 | 기존 파일의 exact string 치환               | `path`, `old_string`, `new_string`, `replace_all` |
| `workspace__listDirectory`              | 디렉토리 구조 조회                          | `path`, `limit`                                   |
| `workspace__runShell` / `runPowerShell` | 터미널 명령어 실행 (비동기/백그라운드 지원) | `command`, `timeout`                              |

### 2. Media (`media__*`) 🎨 _(선택)_

| 도구 이름              | 설명                            | 주요 파라미터 |
| :--------------------- | :------------------------------ | :------------ |
| `media__seeContent`    | 이미지/비주얼 매체 분석 및 조회 | `url`         |
| `media__listenContent` | 오디오/음성 매체 파싱 및 분석   | `url`         |

### 3. Interactive UI (`ui__*`)

| 도구 이름                | 설명                                                   |
| :----------------------- | :----------------------------------------------------- |
| `ui__presentInteractive` | 대화형 카드, 선택 버튼, 양식 등 UI 컴포넌트 렌더링     |
| `ui__reportResult`       | 인터랙티브 UI 컴포넌트의 사용자 선택 및 입력 결과 반환 |

### 4. Browser (`browser__*`) _(선택)_

| 도구 이름                   | 설명                      |
| :-------------------------- | :------------------------ |
| `browser__navigateToUrl`    | 대상 URL 웹페이지 이동    |
| `browser__clickElement`     | DOM 클릭 요소 상호작용    |
| `browser__inputText`        | 입력 양식 텍스트 타이핑   |
| `browser__scrollPage`       | 웹페이지 스크롤           |
| `browser__listInteractable` | 상호작용 가능한 요소 추출 |
| `browser__evaluateJS`       | JS 커스텀 스크립트 실행   |

### 5. Planning & Reflection (`planning__*`) _(선택)_

| 도구 이름                   | 설명                                                    |
| :-------------------------- | :------------------------------------------------------ |
| `planning__createGoal`      | 복잡한 멀티스텝 작업 목표 수립                          |
| `planning__updateGoal`      | 목표 진척도 및 단계별 상태 업데이트                     |
| `planning__clearGoal`       | 활성 목표 초기화                                        |
| `planning__addTodo`         | 세부 할 일 항목 추가                                    |
| `planning__updateTodo`      | 세부 할 일 항목 상태 업데이트                           |
| `planning__clearSession`    | 세션 관련 계획 데이터 초기화                            |
| `planning__getCurrentState` | 현재 수립된 계획 및 목표 상태 조회                      |
| `planning__reflect`         | 도구 에러 발생 시 원인 성찰(Critique) 및 교정 방안 수립 |

### 6. Attachments & Scheduled Tasks (`attachments__*` / `scheduled_task__*`)

| 도구 이름                             | 설명                            |
| :------------------------------------ | :------------------------------ |
| `attachments__readAttachment`         | 세션 첨부파일 내용 조회         |
| `attachments__searchAttachments`      | 첨부파일 검색                   |
| `scheduled_task__createScheduledTask` | 타이머/Cron 정기 알림 예약 생성 |
| `scheduled_task__listScheduledTasks`  | 현재 예약된 작업 목록 조회      |
| `scheduled_task__getScheduledTask`    | 특정 예약 작업 상세 조회        |
| `scheduled_task__updateScheduledTask` | 예약 작업 설정 수정             |
| `scheduled_task__toggleScheduledTask` | 예약 작업 활성화/비활성화 전환  |
| `scheduled_task__deleteScheduledTask` | 예약 작업 취소 및 삭제          |

---

## 🧩 외부 도구가 추가로 필요한 경우 (How to Add More Tools)

기본 제공되는 내장 도구 세트로 해결하기 어려운 특수 작업(예: GitHub PR 관리, Slack 메시지 전송, ComfyUI 이미지 생성, arXiv 논문 검색 등)이 필요한 경우:

1. **추천 MCP 프리셋 설치 (Recommended Extensions)**:
   - 사이드바 **Extensions → Tools → Recommended Extensions**에서 원하는 기능(Brave Search, Exa, GitHub, Slack, arXiv 등)을 원클릭으로 추가합니다. ([Extensions 가이드](extensions.md) 참고)
2. **커스텀 MCP 서버 추가 (Add Custom MCP)**:
   - 로컬 `npx`/`uvx` 프로세스 기반 MCP 서버나 원격 HTTP SSE 서버를 직접 등록합니다. ([커스텀 MCP 가이드](custom-mcp.md) 참고)
3. **스킬 추가 (Skills)**:
   - 특수 작업 도메인에 대한 가이드라인 및 모듈을 스킬로 설치합니다. ([스킬 가이드](skills.md) 참고)
