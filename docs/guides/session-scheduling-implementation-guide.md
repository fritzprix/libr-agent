# 🛠️ Session Scheduling Extension — Implementation Guide

이 문서는 `scheduled_task` 시스템에 **세션 전용 스케줄링(N:1 매핑)** 기능을 추가하는 구체적인 구현 방안을 안내합니다.

> **Related:** 배경·아키텍처·로드맵은 [Session Scheduling Proposal](../proposals/session-scheduled-callback.md)을 참조하세요.

---

## 📋 구현 체크리스트

### Phase 1: Core Logic

#### 단계 1: DB 마이그레이션

- [ ] `scheduled_tasks` 테이블에 `task_category TEXT NOT NULL DEFAULT 'GLOBAL'` 추가
- [ ] `cron_expression` 컬럼 Nullable 로 변경
- [ ] `src-tauri/migration/src/lib.rs`에 마이그레이션 등록

#### 단계 2: Entity & Repository

- [ ] `src-tauri/src/entity/scheduled_task.rs` — `task_category: String` 추가, `cron_expression: Option<String>` 변경
- [ ] `src-tauri/src/repositories/scheduled_task_repository.rs` — `CreateScheduledTaskParams`에 `task_category`, `session_id`, `next_run_at` 확장
- [ ] `UpdateScheduledTaskParams`에 `task_category` 추가 (필요 시)

#### 단계 3: Service Layer

- [ ] `src-tauri/src/services/scheduled_task_service.rs` — `CreateScheduledTaskInput`에 `task_category`, `session_id`, `next_run_at` 추가
- [ ] SESSION + one-shot: `enforce_minimum_interval` 우회 (`cron_expression` NULL 일 때)
- [ ] SESSION: `session_id` 필수 검증
- [ ] SESSION + `delaySeconds`: cron 계산 대신 전달된 `next_run_at` 사용

#### 단계 4: Runner 분기

- [ ] `src-tauri/src/scheduled/runner.rs` — `execute_task`에 `task_category` match 분기
- [ ] `execute_global_task`: 기존 로직 추출 (변경 없음)
- [ ] `execute_session_callback`: pinned `session_id` 직접 주입
- [ ] One-shot 완료: `enabled = false`, `next_run_at = None` (`list_due_tasks`가 `enabled = true`만 조회)
- [ ] 세션 소실 시: `enabled = false` + 경고 로그 (GLOBAL 과 달리 새 세션 생성 금지)

#### 단계 5: MCP 도구

- [ ] `src-tauri/src/mcp/builtin/scheduled_task/tools.rs` — `scheduleCallback` 도구 스키마 추가
- [ ] `handlers.rs` — `handle_schedule_callback` 구현 (`ScheduledTaskService` 경유)
- [ ] `mod.rs` — dispatch 등록
- [ ] `src-tauri/src/mcp/server/tools.rs` — static tool 목록 갱신

#### 단계 6: Tests

- [ ] `scheduled_task_policy_tests.rs` — SESSION governance 예외
- [ ] Runner SESSION 분기 통합 테스트 (one-shot 완료, 세션 소실, resume)
- [ ] MCP `scheduleCallback` handler 테스트

### Phase 2: Frontend

#### 단계 7: Tauri Commands

- [ ] `src-tauri/src/commands/scheduled_task_commands.rs` — 세션별 스케줄 list/cancel API
- [ ] `task_category = 'SESSION'` + `session_id` 필터

#### 단계 8: UI

- [ ] 세션 사이드바 "Schedules" 섹션
- [ ] One-Shot: 카운트다운 + 취소 버튼
- [ ] Recurring: 다음 실행 시각 + 삭제 버튼
- [ ] `src/lib/backend/scheduled-tasks` — 프론트엔드 타입·API 연동

### Phase 3: Polish

- [ ] User Interruption: 사용자 메시지 전송 시 해당 세션의 SESSION 스케줄 일괄 취소
- [ ] 세션별 동시성 락 (busy 세션 skip 로직 재사용 또는 강화)

---

## 📐 데이터 모델 상세

### 기존 `scheduled_tasks` 테이블 (변경 전)

```sql
CREATE TABLE scheduled_tasks (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    cron_expression TEXT NOT NULL,
    schedule_timezone TEXT NOT NULL,
    assistant_id TEXT NOT NULL,
    group_id TEXT,
    group_name TEXT,
    message TEXT NOT NULL,
    yolo_mode BOOLEAN NOT NULL DEFAULT 0,
    created_by_session_id TEXT,
    session_id TEXT,                -- GLOBAL: 첫 실행 후 핀닝
    workspace_override TEXT,
    enabled BOOLEAN NOT NULL DEFAULT 1,
    last_run_at INTEGER,
    next_run_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
```

### 변경 후

```sql
ALTER TABLE scheduled_tasks ADD COLUMN task_category TEXT NOT NULL DEFAULT 'GLOBAL';
-- cron_expression: NOT NULL 제약 제거 (SESSION one-shot 에서 NULL 허용)
```

### One-Shot 완료 처리 (스키마 변경 없음)

`completed_at` 컬럼은 **추가하지 않습니다**. 기존 `enabled` + `next_run_at`으로 충분합니다.

```rust
// One-shot 실행 후 record_run 대신:
repo.update_scheduled_task(&task.id, UpdateScheduledTaskParams {
    enabled: Some(false),
    next_run_at: Some(None),
    ..Default::default()
}).await?;
```

`list_due_tasks`는 `enabled = true AND next_run_at <= now` 조건이므로, 비활성화된 one-shot 은 자동 제외됩니다.

---

## 💻 코드 예시

### 1. Entity 수정 (`src-tauri/src/entity/scheduled_task.rs`)

```rust
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "scheduled_tasks")]
pub struct Model {
    // ... 기존 필드들 ...

    /// "GLOBAL" (existing cron-backed tasks) or "SESSION" (in-session callbacks)
    pub task_category: String,

    /// Nullable for SESSION one-shot tasks (delaySeconds-based)
    pub cron_expression: Option<String>,
}
```

### 2. Runner 분기 (`src-tauri/src/scheduled/runner.rs`)

```rust
async fn execute_task(
    manager: &AgentSessionManager,
    task: &crate::entity::scheduled_task::Model,
    now_ms: i64,
) -> Result<(), String> {
    match task.task_category.as_str() {
        "GLOBAL" => execute_global_task(manager, task, now_ms).await,
        "SESSION" => execute_session_callback(manager, task, now_ms).await,
        other => Err(format!("Unknown task_category: {other}")),
    }
}

async fn execute_session_callback(
    manager: &AgentSessionManager,
    task: &crate::entity::scheduled_task::Model,
    now_ms: i64,
) -> Result<(), String> {
    let session_id = task
        .session_id
        .as_deref()
        .ok_or_else(|| format!("SESSION task '{}' has no session_id", task.id))?;

    let active_sessions = get_active_sessions();
    let active_session_ids = {
        let sessions = active_sessions.read().await;
        sessions.keys().cloned().collect::<HashSet<_>>()
    };

    // 세션 소실: GLOBAL 과 달리 새 세션을 만들지 않음
    let session_repo = get_session_repository();
    if !active_session_ids.contains(session_id)
        && session_repo.get_session(session_id).await?.is_none()
    {
        let repo = get_scheduled_task_repository();
        repo.update_scheduled_task(
            &task.id,
            UpdateScheduledTaskParams {
                enabled: Some(false),
                next_run_at: Some(None),
                ..Default::default()
            },
        )
        .await?;
        log::warn!(
            "⏰ SESSION task '{}' disabled — target session {} no longer exists",
            task.name,
            session_id
        );
        return Ok(());
    }

    if !active_session_ids.contains(session_id) {
        manager.resume_session(session_id).await?;
    }

    // busy 세션 skip (기존 GLOBAL 로직과 동일)
    {
        let sessions = active_sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            if session.is_running {
                log::info!("⏰ Skipping SESSION task '{}' — session {} is busy", task.name, session_id);
                return Ok(());
            }
        }
    }

    let user_message = Message {
        id: Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        role: "user".to_string(),
        content: vec![MCPContent::Text {
            text: task.message.clone(),
            is_error: None,
        }],
        source: Some(MessageSource::ScheduledTask), // 기존 enum 재사용
        // ... 나머지 필드는 execute_global_task 와 동일하게 채움
        ..Default::default()
    };

    manager
        .inject_messages(session_id.to_string(), vec![user_message])
        .await?;

    let repo = get_scheduled_task_repository();
    let is_one_shot = task.cron_expression.is_none();

    if is_one_shot {
        repo.update_scheduled_task(
            &task.id,
            UpdateScheduledTaskParams {
                enabled: Some(false),
                next_run_at: Some(None),
                ..Default::default()
            },
        )
        .await?;
    } else {
        let cron = task.cron_expression.as_deref().unwrap_or("");
        let next_run_at = compute_next_run_for_schedule_timezone(
            cron,
            now_ms,
            &task.schedule_timezone,
        )?;
        repo.record_run(&task.id, None, now_ms, next_run_at).await?;
    }

    Ok(())
}
```

> **리팩토링 팁:** 기존 `execute_task` 본문을 `execute_global_task`로 추출한 뒤, 최상위 `execute_task`에서 category 분기만 추가하면 diff 가 최소화됩니다.

### 3. Service Layer (`src-tauri/src/services/scheduled_task_service.rs`)

```rust
pub struct CreateScheduledTaskInput {
    // ... 기존 필드 ...
    pub task_category: String,           // "GLOBAL" | "SESSION"
    pub session_id: Option<String>,      // SESSION: 필수
    pub next_run_at: Option<i64>,        // SESSION + delaySeconds: 사전 계산값
}

// create_scheduled_task_with_governance 내부:
let is_session_one_shot = input.task_category == "SESSION"
    && input.cron_expression.is_none();

if !is_session_one_shot {
    enforce_minimum_interval(&input.cron_expression, ...)?;
}

let next_run_at = if let Some(precomputed) = input.next_run_at {
    Some(precomputed)
} else {
    compute_next_run_for_schedule_timezone(
        input.cron_expression.as_deref().unwrap_or(""),
        now_ms,
        normalized_schedule_timezone,
    )?
};
```

### 4. MCP 도구 스키마 (`src-tauri/src/mcp/builtin/scheduled_task/tools.rs`)

```rust
fn schedule_callback_tool() -> MCPTool {
    MCPTool {
        name: "scheduleCallback".to_string(),
        description: "Schedule a one-shot or recurring callback for the current session.".to_string(),
        input_schema: object_prop(
            vec![
                ("message", string_prop(1, 8000, Some("Message to inject when the callback fires."))),
                ("name", string_prop(1, 200, Some("Optional label shown in the sidebar."))),
                ("delaySeconds", integer_prop(1, 86400, Some("One-shot delay in seconds. Mutually exclusive with cronExpression."))),
                ("cronExpression", string_prop(1, 50, Some("Cron for recurring callbacks. Mutually exclusive with delaySeconds."))),
            ],
            vec!["message".to_string()],
            None,
        ),
        // ...
    }
}
```

### 5. Handler (`src-tauri/src/mcp/builtin/scheduled_task/handlers.rs`)

```rust
pub async fn handle_schedule_callback(
    server: &ScheduledTaskServer,
    args: Value,
    session_id: Option<String>,
) -> Result<MCPResult, String> {
    let args: ScheduleCallbackArgs = parse_args(args, "scheduleCallback")?;

    let session_id = session_id
        .or_else(|| server.session_id.clone())
        .ok_or("scheduleCallback requires an active session context")?;

    let now_ms = chrono::Utc::now().timestamp_millis();

    let (cron_expression, next_run_at) = match (args.delay_seconds, args.cron_expression) {
        (Some(delay), None) => {
            let next = now_ms + (delay as i64) * 1000;
            (None, Some(next))
        }
        (None, Some(cron)) => {
            let next = compute_next_run_for_schedule_timezone(
                &cron,
                now_ms,
                default_schedule_timezone(),
            )?
            .ok_or("Invalid cron expression: no future occurrences")?;
            (Some(cron), Some(next))
        }
        _ => return Err("Provide exactly one of delaySeconds or cronExpression".into()),
    };

    // ScheduledTaskService 경유 — governance·검증 일원화
    let created = ScheduledTaskService::create_scheduled_task(
        get_scheduled_task_repository(),
        CreateScheduledTaskInput {
            name: args.name.unwrap_or_else(|| "Scheduled Callback".to_string()),
            cron_expression,
            schedule_timezone: default_schedule_timezone().to_string(),
            assistant_id: server.assistant_id.clone().ok_or("Assistant ID required")?,
            group_id: None,
            group_name: None,
            message: args.message,
            yolo_mode: false,
            created_by_session_id: Some(session_id.clone()),
            workspace_override: None,
            task_category: "SESSION".to_string(),   // 명시적 설정 (DB default 에 의존하지 않음)
            session_id: Some(session_id),
            next_run_at,
        },
    )
    .await?;

    Ok(success_result(&format!("Callback scheduled (id: {})", created.id)))
}
```

---

## 🔍 주의사항

1. **하위 호환성**: `task_category` DB 기본값 `'GLOBAL'`. 기존 MCP CRUD 도구는 변경 없이 GLOBAL 태스크만 생성.
2. **One-shot 완료**: `completed_at` 컬럼 없음. `enabled = false` + `next_run_at = None` 패턴 사용.
3. **Service 레이어 필수**: Handler 에서 repository 직접 호출 금지. `ScheduledTaskService`가 cron 검증·governance·next_run_at 계산을 담당.
4. **Governance**: `enforce_minimum_interval`이 짧은 `delaySeconds`를 차단할 수 있음. SESSION one-shot 은 이 검증을 우회하도록 Service 에서 분기.
5. **동시성**: busy 세션 skip 로직은 GLOBAL 과 동일하게 재사용. Phase 3 에서 세션별 락 강화 검토.
6. **세션 소실**: SESSION 타입은 pinned 세션이 삭제되면 태스크를 비활성화. GLOBAL 의 `TaskSessionResolution::Create` 와 다름.
7. **One-shot busy 재시도**: 세션이 busy 이면 one-shot 은 `next_run_at` 을 갱신하지 않고 다음 scheduler tick 에 재시도합니다 (유실되지 않음). recurring 은 cron 기준으로 `next_run_at` 을 앞당깁니다.
8. **Down migration**: `m20260607_000034` 롤백 시 `SESSION` 태스크는 복원되지 않습니다. 운영 롤백 전 백업 필요.
9. **User Interruption**: Phase 3 — 사용자가 새 메시지를 보내면 해당 세션의 모든 `SESSION` 스케줄을 `enabled = false` 처리.

---

## 🚀 실행 순서

1. 마이그레이션 파일 생성 → `lib.rs` 등록 → 실행
2. Entity + Repository params 수정 → 컴파일 확인
3. `ScheduledTaskService` SESSION 경로 → 단위 테스트
4. Runner `execute_global_task` 추출 + SESSION 분기 → 통합 테스트
5. MCP `scheduleCallback` 도구 → handler 테스트
6. Tauri commands (list/cancel by session)
7. Frontend UI 연결

각 단계마다 `pnpm refactor:validate`로 빌드를 검증하세요.
