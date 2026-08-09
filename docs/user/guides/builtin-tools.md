---
title: 내장 도구 레퍼런스 (Built-in MCP Tools)
---

# 내장 도구 레퍼런스 (Built-in MCP Tools)

> LibrAgent는 별도의 외부 MCP 서버 설치 없이도 고성능 코딩, 파일 조작, 웹 탐색, 계획 수립, 미디어 처리, 백그라운드 예약 등을 곧바로 수행할 수 있는 **Rust 기반 단일통합 내장 MCP 서버 세트**를 기본 제공합니다.

모든 내장 도구는 통일된 `{server}__{tool}` 형식의 명명 규칙을 따릅니다.

---

## 📌 기본 도구 (Core) vs 선택 도구 (Optional) 구분

LibrAgent 내장 도구는 **모든 세션에 기본으로 활성화되는 코어 도구**와 **어시스턴트 설정에서 필요에 따라 켜고 끌 수 있는 선택 도구**로 구분됩니다.

### 1️⃣ 기본 활성화 도구 (Core Built-ins — 항상 사용 가능)

어시스턴트 생성 시 기본 적용되며, 에이전트의 기본적인 작업 수행 및 UI 상호작용에 필수적인 도구입니다.

- **`workspace__*`**: 작업 공간 파일 읽기/쓰기/라인편집, 디렉터리 조회, 터미널 명령어 실행
- **`ui__*`**: 대화형 카드, 선택지 버튼(`select_prompt`), 텍스트 입력 양식(`text_prompt`), 차트, 루프 차단(`circuitBreak`)
- **`agent__*`**: 서브 에이전트 자율 생성 및 멀티 에이전트 오케스트레이션
- **`skills__*`**: 스킬 실행 및 컨텍스트 로딩
- **`playbook__*`**: 자동화 워크플로우 플레이북 목록 조회/실행/저장
- **`attachments__*`**: 세션 첨부 파일 관리 및 검색
- **`scheduled_task__*`**: 백그라운드 타이머 및 Cron 주기적 예약 작업 관리
- **`scratchpad__*`**: 추론 과정 기록 및 임시 계산 노트
- **`tool__*`**: 도구 디스커버리 및 시스템 도구 관리

### 2️⃣ 선택 활성화 도구 (Optional Built-ins — 어시스턴트 설정에서 선택)

특정 목적(웹 탐색, 미디어 편집, 세부 계획 수립 등)을 위해 어시스턴트 설정(**Assistants → Edit → Tools**)에서 켜거나 끌 수 있는 도구입니다.

- **`media__*`**: 이미지 처리, 리사이즈, 오디오/이미지 텍스트 추출
- **`browser__*`**: 헤드리스 브라우저 웹 탐색, 클릭, 텍스트 입력, 스크린샷
- **`planning__*`**: 다단계 계획 수립(`create_plan`), 단계별 상태 업데이트, 실패 성찰(`reflect`)
- **`knowledge__*`**: 시맨틱 지식 저장 및 영구 메모리 검색
- **`setup-wizard__*`**: 파이썬/Node/uv 런타임 자동 진단 및 환경 설치 마법사
- **`history__*`**: 이전 세션 이력 조회

---

## 🛠️ 내장 도구 상세 기능 레퍼런스

### 1. Workspace (`workspace__*`)

| 도구 이름                    | 설명                             | 주요 파라미터                     |
| :--------------------------- | :------------------------------- | :-------------------------------- |
| `workspace__read_file`       | 파일 내용 읽기                   | `path`, `start_line`, `end_line`  |
| `workspace__write_file`      | 파일 생성 및 전체 덮어쓰기       | `path`, `content`                 |
| `workspace__edit_file`       | 라인 범위 지정 정교한 코드 편집  | `path`, `edits`                   |
| `workspace__list_directory`  | 디렉토리 구조 조회               | `path`                            |
| `workspace__delete_file`     | 파일 삭제                        | `path`                            |
| `workspace__execute_command` | 터미널 명령어 실행 (비동기 지원) | `command`, `cwd`, `is_background` |

### 2. Media (`media__*`) 🎨 _(신규/선택)_

| 도구 이름              | 설명                                 | 주요 파라미터                   |
| :--------------------- | :----------------------------------- | :------------------------------ |
| `media__process_image` | 이미지 분석, 색상 및 메타데이터 추출 | `image_path`                    |
| `media__resize_image`  | 이미지 해상도 변경 및 리사이즈       | `image_path`, `width`, `height` |
| `media__extract_text`  | 이미지/오디오 내 텍스트/음성 추출    | `file_path`                     |

### 3. Interactive UI (`ui__*`)

| 도구 이름                          | 설명                                             |
| :--------------------------------- | :----------------------------------------------- |
| `ui__select_prompt`                | 사용자 다중 선택지 버튼 카드 렌더링              |
| `ui__text_prompt`                  | 사용자 텍스트 입력 폼 렌더링                     |
| `ui__line_chart` / `ui__bar_chart` | 데이터 시각화 차트 렌더링                        |
| `ui__circuitBreak`                 | 반복 무한 루프 감지 및 일시정지/재개 카드 렌더링 |
| `ui__wait`                         | 사용자 입력 및 외부 처리 휴식 대기               |

### 4. Browser (`browser__*`) _(선택)_

| 도구 이름             | 설명                    |
| :-------------------- | :---------------------- |
| `browser__navigate`   | 웹페이지 이동           |
| `browser__click`      | DOM 요소 클릭           |
| `browser__type`       | 입력 양식 텍스트 타이핑 |
| `browser__screenshot` | 웹페이지 화면 캡처      |

### 5. Planning & Reflection (`planning__*`) _(선택)_

| 도구 이름               | 설명                                                               |
| :---------------------- | :----------------------------------------------------------------- |
| `planning__create_plan` | 3단계 이상의 복잡한 멀티스텝 작업 계획 수립                        |
| `planning__update_plan` | 작업 진척도 및 단계별 상태(`todo`, `in_progress`, `done`) 업데이트 |
| `planning__reflect`     | 도구 에러 발생 시 원인 성찰(Critique) 및 교정 방안 수립            |

### 6. Scheduled Tasks (`scheduled_task__*`)

| 도구 이름                         | 설명                            |
| :-------------------------------- | :------------------------------ |
| `scheduled_task__create`          | 타이머/Cron 정기 알림 예약 생성 |
| `scheduled_task__list` / `delete` | 예약 작업 조회 및 취소          |

---

## 🧩 외부 도구가 추가로 필요한 경우 (How to Add More Tools)

기본 제공되는 내장 도구 세트로 해결하기 어려운 특수 작업(예: GitHub PR 관리, Slack 메시지 전송, ComfyUI 이미지 생성, arXiv 논문 검색 등)이 필요한 경우:

1. **추천 MCP 프리셋 설치 (Recommended Extensions)**:
   - 사이드바 **Extensions → Tools → Recommended Extensions**에서 원하는 기능(Brave Search, Exa, GitHub, Slack, arXiv 등)을 원클릭으로 추가합니다. ([Extensions 가이드](extensions.md) 참고)
2. **커스텀 MCP 서버 추가 (Add Custom MCP)**:
   - 로컬 `npx`/`uvx` 프로세스 기반 MCP 서버나 원격 HTTP SSE 서버를 직접 등록합니다. ([커스텀 MCP 가이드](custom-mcp.md) 참고)
3. **스킬 추가 (Skills)**:
   - 특수 작업 도메인에 대한 가이드라인 및 모듈을 스킬로 설치합니다. ([스킬 가이드](skills.md) 참고)
