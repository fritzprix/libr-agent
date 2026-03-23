# Builtin Service Overhaul — Master Plan

## Bird's Eye View

```
현재                           Phase 1                        Phase 2
─────────────────────          ─────────────────────          ─────────────────────
content_store (이름 혼란)  →   attachments (명확한 이름)  →   stable ID 기반 참조
alias = NAME 상수 직결          backward-compat alias 유지      alias/name 변경 자유
DB에 문자열 직렬화 박힘          DB: 문자열 그대로 (호환)         DB: stable ID로 마이그레이션
```

**핵심 목적:**

- Phase 1: 에이전트 인지 부하 감소. 이름만 봐도 scope/lifecycle을 즉시 이해.
- Phase 2: 이름 ↔ 식별자 결합 해제. 이후 어떤 이름 변경도 DB/코드 파급 없이 가능.

---

## Phase 1 — `content_store` → `attachments` Rename

### 목표

`builtin_content_store__*` prefix와 `canonical: 'content_store'`를 `attachments`로 변경.
기존 DB에 저장된 어시스턴트 설정 깨지지 않게 backward-compat 보장.

### 변경 파일 목록

#### 🔴 Rust — 기능 직결 (컴파일/라우팅 깨짐)

| 파일                                             | 변경 내용                                                                                                        |
| ------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/mcp/builtin/content_store/mod.rs` | `NAME = "attachments"`                                                                                           |
| `src-tauri/src/mcp/server/tools.rs`              | match arm `"content_store" \| "contentstore"` → `"attachments" \| "content_store" \| "contentstore"` (폴백 유지) |
| `src-tauri/src/mcp/service_proxy/factory.rs`     | 동일하게 `"attachments" \| "content_store" \| "contentstore"`                                                    |
| `src-tauri/src/mcp/builtin/mod.rs`               | `pub mod content_store` 유지 (디렉토리 rename 안 함)                                                             |
| `src-tauri/src/services/assistant_init.rs`       | 기본 어시스턴트 초기화 시 alias `"attachments"` 로 변경                                                          |

> **디렉토리명 (`content_store/`)은 안 바꾼다.** Rust module 경로는 내부 구현 detail — 외부 노출 없음. 이름 바꾸면 모든 `use` import 전부 손봐야 함. 득보다 실이 크다.

#### 🔴 Frontend — tool ID 하드코딩

| 파일                                                            | 변경 내용                                                   |
| --------------------------------------------------------------- | ----------------------------------------------------------- |
| `src/features/agent/context/AgentResourceAttachmentContext.tsx` | `builtin_content_store__*` → `builtin_attachments__*` (3곳) |
| `src/components/shared/SessionFilesPopover.tsx`                 | `builtin_content_store__read` 변경                          |
| `src/features/agent/api/agent-backend.ts`                       | `builtin_content_store__add` 변경                           |
| `src/lib/assistant/runtime-builtins.ts`                         | `canonical: 'content_store'` → `'attachments'`              |
| `src/lib/__tests__/utils.extractBuiltInServiceAlias.test.ts`    | 테스트 문자열 업데이트                                      |
| `src/lib/message-preprocessor.ts`                               | 이미 `search`로 변경됨 — `sessionId` 힌트 문자열 확인       |

#### 🟡 DB 호환성 리스크 (핵심)

`allowedBuiltInServiceAliases`는 `assistant.config` TEXT JSON blob 안에 저장됨:

```json
{ "allowedBuiltInServiceAliases": ["content_store", "browser", ...] }
```

기존 유저 레코드에 `"content_store"` 가 박혀있음. Rust 라우팅에서 폴백을 유지하면
기능은 깨지지 않지만, UI에서 "어떤 서비스가 활성화됐는지" 표시가 어긋날 수 있음.

**해결 방법:** `canonicalizeAlias()` 함수에 alias 매핑 레이어 추가.

```typescript
// runtime-builtins.ts
const ALIAS_MIGRATIONS: Record<string, string> = {
  content_store: 'attachments',
  contentstore: 'attachments',
};
function canonicalizeAlias(alias: string): string | null {
  const normalized = (
    ALIAS_MIGRATIONS[alias.toLowerCase()] ?? alias
  ).toLowerCase();
  return _canonicals.has(normalized) ? normalized : null;
}
```

→ DB에서 읽은 `"content_store"`가 런타임에 `"attachments"`로 투명하게 매핑됨.

### 리스크 평가

| 리스크               | 수준    | 대응                                             |
| -------------------- | ------- | ------------------------------------------------ |
| 프론트 하드코딩 누락 | 🟡 중   | grep `builtin_content_store__` 전수 확인 후 진행 |
| DB 기존 설정 깨짐    | 🟡 중   | alias migration 매핑 + Rust 폴백 match arm       |
| Rust 라우팅 누락     | 🟢 낮음 | factory.rs + server/tools.rs 2곳만               |
| 테스트 실패          | 🟢 낮음 | 문자열 교체 수준                                 |

---

## Phase 2 — Stable ID 기반 참조 시스템

### 목표

`allowedBuiltInServiceAliases`의 값을 변경 가능한 이름 문자열에서
변경 불가능한 stable ID로 교체. 이후 어떤 이름 변경도 DB/코드 파급 없이 가능.

### 설계

#### Rust — BuiltinServiceId enum 도입

```rust
// src-tauri/src/mcp/builtin/service_id.rs
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuiltinServiceId {
    Planning,
    Workspace,
    Knowledge,
    Assistant,
    Skills,
    Playbook,
    Attachments,  // formerly content_store
    Swarm,
    Ui,
    Browser,
    Bootstrap,
    McpManager,
}

impl BuiltinServiceId {
    pub fn current_alias(&self) -> &str {
        match self {
            Self::Attachments => "attachments",
            Self::Planning => "planning",
            // ...
        }
    }
}
```

#### DB 마이그레이션

`assistant.config` JSON blob 안의 `allowedBuiltInServiceAliases` 배열을
문자열 → enum variant 직렬화 값으로 교체하는 one-time migration.

```sql
-- 개념적 표현 (실제론 SeaORM migration으로 작성)
-- assistant 레코드 순회하며 config JSON 업데이트
-- "content_store" → "attachments"
-- 기타 이름 변경이 있었다면 동일하게 처리
```

#### Frontend — BuiltinServiceId 타입 도입

```typescript
// runtime-builtins.ts
export type BuiltinServiceId =
  | 'planning'
  | 'workspace'
  | 'knowledge'
  | 'assistant'
  | 'skills'
  | 'playbook'
  | 'attachments'
  | 'swarm'
  | 'ui'
  | 'browser'
  | 'bootstrap'
  | 'mcp_manager';

// DB 저장값은 이 타입의 literal string
// display name은 별도 매핑 테이블
```

#### 라우팅 변경

```rust
// factory.rs — 이름 문자열 대신 ID로 match
pub fn create_builtin_server(service_id: &BuiltinServiceId, ...) -> ... {
    match service_id {
        BuiltinServiceId::Attachments => ContentStoreServer::new(...),
        BuiltinServiceId::Planning => PlanningServer::new(...),
        // ...
    }
}
```

### 변경 파일 목록

| 레이어 | 파일                                | 변경 내용                                         |
| ------ | ----------------------------------- | ------------------------------------------------- |
| Rust   | `mcp/builtin/service_id.rs` (신규)  | BuiltinServiceId enum 정의                        |
| Rust   | `mcp/service_proxy/factory.rs`      | string match → enum dispatch                      |
| Rust   | `mcp/server/tools.rs`               | string match → enum dispatch                      |
| Rust   | `mcp/builtin/mod.rs`                | 등록 로직 ID 기반으로                             |
| Rust   | `mcp/service_proxy/mod.rs`          | 에러 메시지 내 alias 참조                         |
| DB     | `migration/` (신규)                 | config JSON alias 값 마이그레이션                 |
| TS     | `lib/assistant/runtime-builtins.ts` | BuiltinServiceId 타입 도입, ALIAS_MIGRATIONS 제거 |
| TS     | `models/assistant.ts` (추정)        | allowedBuiltInServiceAliases 타입 강화            |

### 리스크 평가

| 리스크                                        | 수준    | 대응                                                        |
| --------------------------------------------- | ------- | ----------------------------------------------------------- |
| DB migration 실패 시 기존 설정 전부 무효      | 🔴 높음 | 마이그레이션 전 config 백업 로직, 실패 시 원본 유지         |
| Rust enum serde 직렬화 값 변경 시 역호환 깨짐 | 🟡 중   | `#[serde(alias = "content_store")]` 로 구버전도 deserialize |
| Phase 1 건너뛰고 Phase 2만 하면 복잡도 급증   | 🟡 중   | Phase 1 완료 후 진행 강제                                   |

---

## 진행 순서 요약

```
[지금] Phase 1 (1~2시간)
  1. Rust NAME 상수 변경 + 폴백 match arm 추가
  2. frontend 하드코딩 전수 교체
  3. canonicalizeAlias() alias migration 매핑 추가
  4. 빌드 + 테스트 확인

[다음 sprint] Phase 2 (별도 PR)
  1. BuiltinServiceId enum 설계 확정
  2. DB migration 작성 + 검증
  3. 라우팅 enum dispatch로 교체
  4. frontend 타입 강화
```

Phase 1은 현재 PR (`dev/0.5.x`) 에 포함 가능.
Phase 2는 반드시 별도 PR — DB 마이그레이션이 포함되므로 QA 범위가 다름.
