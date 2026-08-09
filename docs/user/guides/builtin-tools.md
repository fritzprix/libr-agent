---
title: 내장 도구 레퍼런스 (Built-in MCP Tools)
---

# 내장 도구 레퍼런스 (Built-in MCP Tools)

> LibrAgent는 별도의 외부 MCP 서버 설치 없이도 고성능 코딩, 파일 조작, 웹 탐색, 계획 수립, 백그라운드 예약 등을 곧바로 수행할 수 있는 **Rust 기반 단일통합 내장 MCP 서버 세트**를 기본 제공합니다.

모든 내장 도구는 통일된 `{server}__{tool}` 형식의 명명 규칙을 따릅니다.

---

## 1. Workspace (`workspace__*`)

개발 작업 공간 내 파일 관리, 코드 편집 및 터미널 명령어 실행을 담당하는 핵심 코딩 도구 모듈입니다.

| 도구 이름 | 설명 | 주요 입력 파라미터 |
| :--- | :--- | :--- |
| `workspace__read_file` | 파일 내용 읽기 | `path` (상대/절대 경로), `start_line`, `end_line` |
| `workspace__write_file` | 새 파일 생성 또는 파일 전체 덮어쓰기 | `path`, `content` |
| `workspace__edit_file` | 파일 부분 라인 단위 정교한 변경 (라인 보존 안전 변경) | `path`, `edits: [{ old_line, new_line, content }]` |
| `workspace__list_directory` | 디렉토리 구조 및 파일 목록 조회 | `path` |
| `workspace__delete_file` | 파일 삭제 | `path` |
| `workspace__execute_command` | 터미널 명령 실행 (백그라운드 비동기 지원) | `command`, `cwd`, `is_background` |

---

## 2. Interactive UI (`ui__*`)

에이전트가 대화창 내에서 사용자에게 동적/대화형 인터페이스(카드, 선택지, 차트, 대기 폼)를 제공하고 입력을 전달받는 도구입니다.

| 도구 이름 | 설명 | 주요 특징 |
| :--- | :--- | :--- |
| `ui__select_prompt` | 사용자에게 다중 선택지 버튼 카드 렌더링 | 사용자가 버튼 클릭 시 선택된 값을 에이전트에 전달 |
| `ui__text_prompt` | 사용자에게 텍스트 입력 양식 렌더링 | 사용자가 직접 텍스트를 작성하여 전송 |
| `ui__line_chart` / `ui__bar_chart` | 대화창 내 대화형 데이터 차트 렌더링 | 라인/바 데이터 시각화 리소스 반환 |
| `ui__circuitBreak` | 동일 도구 연속 반복 호출(루프) 감지 및 안전 차단 | Amber 경고 카드 렌더링 및 `Resume Execution` 버튼 제공 |
| `ui__wait` | 사용자 확인이나 외부 작업 입력을 기다리는 휴식 | 유휴(Idle) 대기 상태 전환 |

---

## 3. Planning & Reflection (`planning__*`)

복잡한 멀티스텝 아키텍처 작업이나 리팩토링 수행 시 구조화된 세션 계획을 수립하고 모니터링하며, 실패 원인을 반성(Critique)하는 자율 사고 모듈입니다.

| 도구 이름 | 설명 | 활용 시점 |
| :--- | :--- | :--- |
| `planning__create_plan` | 세션 수행을 위한 단계별 작업 계획 생성 | 3단계 이상의 복잡한 기능 구현 시 |
| `planning__update_plan` | 작업 진척도 및 단계별 상태(todo, in_progress, done) 업데이트 | 각 단계 완료 시점 |
| `planning__reflect` | 도구 실패 또는 지돌 상황 발생 시 체계적 성찰(Critique → Reflection → NextAction) 작성 | 반복 에러 발생 또는 접근법 수정 필요 시 |

---

## 4. Playbooks (`playbook__*`)

자주 사용하는 작업 절차나 서브 에이전트 템플릿(Playbook)을 저장하고 재사용하는 자동화 모듈입니다.

| 도구 이름 | 설명 |
| :--- | :--- |
| `playbook__list_playbooks` | 현재 어시스턴트에 등록된 플레이북 목록 조회 |
| `playbook__show_playbook` | 특정 플레이북의 상세 내용 및 실행 단계 확인 |
| `playbook__run_playbook` | 지정된 플레이북을 자율 실행 |
| `playbook__save_playbook` | 새로운 플레이북 저장 및 등록 |

---

## 5. Browser (`browser__*`)

웹 리서치 및 웹 UI 렌더링 검증을 위한 헤드리스 브라우저 자동화 모듈입니다.

| 도구 이름 | 설명 |
| :--- | :--- |
| `browser__navigate` | 지정된 URL로 브라우저 이동 |
| `browser__click` | 웹 페이지 내 DOM 요소 클릭 |
| `browser__type` | 웹 양식 텍스트 입력 |
| `browser__screenshot` | 현재 웹 페이지 스크린샷 캡처 |

---

## 6. Scheduled Tasks (`scheduled_task__*`)

타이머 및 Cron 주기적 자동화 작업을 백그라운드에서 실행하는 모듈입니다.

| 도구 이름 | 설명 |
| :--- | :--- |
| `scheduled_task__create` | 일회성 타이머 또는 Cron 정기 실행 예약 생성 |
| `scheduled_task__list` | 활성화된 예약 작업 목록 확인 |
| `scheduled_task__delete` | 예약 작업 취소 및 삭제 |

---

## 7. Knowledge & Scratchpad (`knowledge__*`, `scratchpad__*`)

세션 간 기억(Memory) 저장 및 임시 메모장 모듈입니다.

- `knowledge__search` / `store`: 프로젝트 지식 및 영구 메모리 시맨틱 검색/저장
- `scratchpad__think`: 복잡한 로직 구상 시 임시 연산 노트 작성

---

## 도구 권한 및 허용 설정

어시스턴트마다 사용할 수 있는 내장 도구 세트를 제한할 수 있습니다:
1. 사이드바 **Assistants** 메뉴로 이동합니다.
2. 원하는 어시스턴트를 클릭 후 **Edit**을 누릅니다.
3. **Tools** 탭에서 허용할 Built-in 서버 선택을 조절합니다. ([어시스턴트 가이드](assistants.md) 참고)
