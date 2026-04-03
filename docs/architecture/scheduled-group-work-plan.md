# 📑 LibrAgent: Scheduled Group Work (SGW) 구현 계획서

## 1. 개요 (Vision)

LibrAgent의 **Agent V2** 아키텍처와 **Scheduled Task** 시스템을 결합하여, 사용자가 정의한 워크스페이스 내에서 다수의 에이전트가 정해진 시간표에 따라 비동기적으로 협업하는 **'자율 협업 에이전트 허브'**를 구축합니다.

핵심은 에이전트 간의 직접적인 통신 대신, 공유 워크스페이스의 **`progress.md` (Shared Memory)**를 매개로 한 상태 공유와 협업입니다.

---

## 2. 아키텍처 설계

### 2.1 워크스페이스 구조 (The "Office")

`create_group_workspace` 스킬이 생성할 표준 디렉토리 구조입니다.

```text
/group-workspace/
├── coordination/
│   ├── config.json         # 그룹 에이전트 설정 (Role, Cron, Message)
│   └── progress.md         # [핵심] 에이전트 간 공유 작업 게시판 (Shared State)
├── skills/                 # 해당 그룹 전용 전문 스킬들
│   ├── role-a/SKILL.md
│   └── role-b/SKILL.md
├── agents.md               # 그룹 공통 지침 및 협업 프로토콜
└── docs/                   # 공유 문서 및 결과물
```

### 2.2 백엔드 확장 (`src-tauri/src/`)

- **Entity (`scheduled_task.rs`)**:
  - `group_id: Option<String>`: 태스크들을 하나의 그룹으로 묶는 식별자.
  - `group_role: Option<String>`: 그룹 내 역할 명칭 (Analyst, Fixer 등).
- **Service (`scheduled_task_service.rs`)**:
  - `sync_group_from_workspace(path)`: 특정 경로의 `config.json`을 읽어 `scheduled_tasks` 테이블에 일괄 등록/갱신하는 로직.
  - 모든 생성된 태스크에 `workspace_override`를 해당 경로로 강제 할당.

---

## 3. 핵심 메커니즘: 비동기 협업 프로토콜

에이전트들은 서로의 세션 내역을 공유하지 않지만, 다음 규칙에 따라 `progress.md`를 통해 협업합니다.

1.  **시작 지침**: "작업 시작 전 `coordination/progress.md`를 읽어 현재 상태를 파악하라."
2.  **작업 수행**: 자신의 역할(`group_role`)에 맞는 도구와 스킬을 사용하여 업무 수행.
3.  **종료 지침**: "작업 완료 후 `coordination/progress.md`에 자신의 로그와 다음 에이전트를 위한 가이드(Handover)를 업데이트하라."

---

## 4. 구현 단계 (Phase)

### Phase 1: `create_group_workspace` 스킬 개발

- 사용자의 목적을 분석하여 적절한 에이전트 수와 스케줄(Cron)을 제안.
- `coordination/config.json` 및 `progress.md` 초기 파일 생성.
- 각 역할에 맞는 워크스페이스 로컬 스킬(`skills/`) 생성.

### Phase 2: 그룹 태스크 동기화 엔진 구현

- 워크스페이스의 설정을 DB에 반영하는 Tauri 커맨드 추가.
- `config.json` 변경 시 자동으로 스케줄러 태스크를 갱신하는 감시(Watch) 로직 검토.

### Phase 3: 그룹 대시보드 UI

- Scheduled Task 관리 화면에서 그룹별 태스크 가시화.
- 공유 워크스페이스의 `progress.md`를 실시간 렌더링하여 진행 상황 브리핑 제공.

---

## 5. 기대 효과 및 활용 시나리오

### 활용 시나리오: "보안 패치 팀"

1.  **Scanner (09:00)**: 코드베이스를 스캔하여 취약점 리스트를 `progress.md`에 작성.
2.  **Fixer (10:00)**: `progress.md`에서 취약점 리스트를 확인, 실제 코드를 수정하고 결과를 기록.
3.  **Reviewer (11:00)**: 수정된 코드를 검토하고 최종 리포트를 작성하여 `docs/`에 저장.

### 이점

- **비용 최적화**: 24시간 상주 에이전트 없이 필요할 때만 깨워 사용 (토큰 절감).
- **격리 및 보안**: `workspace_override`를 통해 에이전트의 활동 범위를 특정 프로젝트로 제한.
- **사람과의 협업**: 사람이 중간에 `progress.md`를 직접 수정하여 에이전트의 방향을 수정 가능.

---

**작성일**: 2025-03-04
**작성자**: LibrAgent Coding Expert (Architecture Audit Specialist)
