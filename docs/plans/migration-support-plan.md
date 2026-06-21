# LibrAgent Migration Support — Final Implementation Plan (Consensus-Reviewed)

> Review panel: Frontend & UX, Backend Rust / DB, Security & Edge Cases  
> Verdict: **APPROVE_WITH_CONDITIONS** — 6 Critical + 12 Significant fixes applied

---

## 1. Objective

LibrAgent 설정과 사용자 데이터를 기기간 (집 ↔ 직장) 에 내보내기/가져오기.  
**세션/메시지/계획 데이터는 제외** — 마이그레이션의 목적은 설정 복원이지 대화 기록 이전이 아님.  
예상 파일 크기: **10~100MB**

---

## 2. 마이그레이션 파일 포맷

**확장자**: `.libragent-migration` (ZIP 아카이브)

**구조**:

```
.migration/
├── manifest.json           # 버전, 앱 버전, 내보낸 시각, SHA-256 hash
├── settings.json           # P0 필수
├── assistants.json         # P0 필수
├── mcp_servers.json        # P0 필수
├── playbooks.json          # P0 필수
├── scheduled_tasks.json    # P0 필수
└── user_skills/            # P0 필수 (symlink skip)
    └── <skill-name>/
        └── SKILL.md + 기타 파일
```

**manifest.json**:

```json
{
  "format_version": 1,
  "app_version": "0.8.0",
  "exported_at": "2025-07-15T10:30:00Z",
  "manifest_hash": "sha256:abcdef..."
}
```

**제외 데이터 (v1 범위 밖)**: sessions, messages, planning_goals, planning_todos, planning_scratchpad, knowledge, knowledge_chunk, store, content

---

## 3. Tauri 백엔드 커맨드 (4개)

### 3.1 `export_migration`

**시그니처**:

```rust
#[tauri::command]
pub async fn export_migration(
    window: tauri::Window,
    output_path: String,
    include_sensitive_data: bool, // Security#1: UI toggle, default: false
) -> Result<MigrationExportInfo, String>
```

**동작**:

1. **output_path 검증** — XDG_DATA_HOME/Downloads 등 사용자 쓰기 가능 디렉토리만 허용 (Security#10)
2. 임시 ZIP 디렉토리 생성
3. `manifest.json` 작성 (현재 앱 버전 + SHA-256 hash)
4. 각 Repository에서 데이터 쿼리 → JSON 직렬화
5. **symlink skip** — `user_skills/` 복사 시 symlink는 무조건 skip (Security#5: 외부 파일 읽기 방지)
6. ZIP 압축 → output_path 저장
7. `MigrationExportInfo` 반환 (파일 크기, 포함된 섹션 및 항목 수)

**사용 Repository**:

- SettingsRepository → settings.json
- AssistantRepository → assistants.json
- McpServerRepository → mcp_servers.json
- PlaybookRepository → playbooks.json
- ScheduledTaskRepository → scheduled_tasks.json
- `skill_service::get_user_skills_directory()` → user_skills/

### 3.2 `import_migration`

**시그니처**:

```rust
#[tauri::command]
pub async fn import_migration(
    window: tauri::Window,
    file_path: String,
    conflict_strategy: String, // "skip" | "overwrite" | "merge"
) -> Result<MigrationImportResult, String>
```

**동작 (Atomicity 보장)**:

1. **ZIP Slip 방지** — ZIP entry 추출 전 `SecurityValidator::validate_path_for_write()` 필수 적용 (Security#4)
2. **Billion-Laughs 방지** — `serde_json::de::Deserializer::set_recursion_limit(64)` + 최대 JSON 크기 20MB(단일 파일)/250MB(전체 ZIP) 제한 (Security#3)
3. `manifest.json` 검증 (format_version + manifest_hash)
4. **BackupManager로 사전 백업 생성**
5. **트랜잭션 시작** — main DB에서 단일 격리 트랜잭션 시작 (외래키 제약조건 위반 방지를 위해 **종속성 역순 삭제 및 정방향 삽입** 순서 강제) (P0-Critical Resolution)
6. **데이터베이스 적용** — 트랜잭션 내에서 설정, 어시스턴트, MCP 서버, 플레이북, 스케줄드 태스크 순차 주입 및 충돌 처리
7. **참조 무결성 검증** — 커밋 직전 `PRAGMA foreign_key_check;`를 트랜잭션 내에서 수행하여 관계 무결성 체크
8. 성공 시: 트랜잭션 커밋 (데이터 영구 저장) 및 임시 디렉토리에서 실시간 복사된 user_skills 파일 시스템 적용
9. 실패 시: 트랜잭션 롤백 및 `BackupManager` 사전 백업 파일을 활용한 DB/파일 시스템 원상 복구 (원자성 및 무결성 보장)
10. 성공 후: MCP 서버 재인증 trigger, 스케줄드 태스크 next_run_at 재계산

**충돌 해결 전략 (Consensus: merge는 settings만 허용)**:

| 섹션            | skip                        | overwrite      | merge                                 |
| --------------- | --------------------------- | -------------- | ------------------------------------- |
| settings        | 기존 키 유지                | 기존 값 교체   | ✅ JSON deep merge (flat key-value만) |
| assistants      | 기존 ID 유지                | 전체 설정 교체 | ❌ skip/overwrite만                   |
| mcp_servers     | 기존 name 유지              | 전체 설정 교체 | ❌ skip/overwrite만                   |
| playbooks       | 기존 (id+assistant_id) 유지 | 전체 교체      | ❌ skip/overwrite만                   |
| scheduled_tasks | 기존 ID 유지                | 전체 교체      | ❌ skip/overwrite만                   |
| user_skills     | 기존 디렉토리 유지          | 전체 교체      | ❌ skip/overwrite만                   |

**가져오기 순서 (강제)**:

```
1. settings.json          ← 어시스턴트 config 참조 가능하므로 먼저
2. assistants.json        ← 나머지가 참조
3. mcp_servers.json       ← 어시스턴트가 참조
4. playbooks.json         ← assistant_id 참조 확인 필요
5. scheduled_tasks.json   ← assistant_id, playbook 참조 확인 필요
6. user_skills/           ← 파일 시스템 (독립적)
```

**import 후 처리**:

- MCP 서버: `reverify_mcp_servers()` 커맨드 호출 (Backend#8)
- 스케줄드 태스크: cron 유효성 검사 후 `next_run_at` 재계산 (Backend#9)

### 3.3 `inspect_migration`

**시그니처**:

```rust
#[tauri::command]
pub async fn inspect_migration(file_path: String) -> Result<MigrationPreview, String>
```

**동작**:

1. **in-memory ZIP parsing** — `BufReader::new(file)` + `zip::read::ZipArchive::new()`로 메모리에서만 파싱 (Security#3: disk write 없음)
2. `manifest.json` 읽기 + hash 검증 (Security#11)
3. **ZIP 내 JSON 파일 파싱하여 실제 항목 수 반환** — 크기 추정이 아닌 item count (Backend#11)
4. 호환성 상태 반환: `compatible` | `warning` | `incompatible` (Frontend#1)

**반환 타입**:

```rust
pub struct MigrationPreview {
    pub format_version: u32,
    pub app_version: Option<String>,
    pub exported_at: Option<String>,
    pub compatibility: CompatibilityStatus, // compatible | warning | incompatible
    pub sections: Vec<SectionPreview>,
    pub total_size_bytes: u64,
    pub file_path: String, // 프론트엔드 doImport에서 참조 필요 (Backend#14)
}

pub struct SectionPreview {
    pub name: String,
    pub item_count: usize,
    pub size_bytes: u64,
}

pub enum CompatibilityStatus {
    Compatible,
    NewerVersion { message: String },
    Incompatible { message: String },
}
```

---

## 4. 프론트엔드 구현

### 4.1 백엔드 래퍼 (`src/lib/backend/migration.ts`)

```typescript
import { safeInvoke } from './core';

export type ConflictStrategy = 'skip' | 'overwrite' | 'merge';

export interface MigrationExportInfo {
  file_path: string;
  file_size_bytes: number;
  sections: string[];
}

export interface MigrationImportResult {
  sections_imported: Record<string, MigrationSectionReport>;
  total_imported: number;
  total_skipped: number;
  total_errors: number;
}

export interface MigrationSectionReport {
  success: number;
  skipped: number;
  errors: string[];
}

export interface SectionPreview {
  name: string;
  item_count: number;
  size_bytes: number;
}

export type CompatibilityStatus = 'compatible' | 'warning' | 'incompatible';

export interface MigrationPreview {
  format_version: number;
  app_version: string | null;
  exported_at: string | null;
  compatibility: CompatibilityStatus;
  sections: SectionPreview[];
  total_size_bytes: number;
  file_path: string; // inspect_migration에서 전달됨 (Backend#14)
}

export async function exportMigration(
  outputDir: string,
  includeSensitiveData: boolean,
): Promise<MigrationExportInfo> {
  return safeInvoke<MigrationExportInfo>('export_migration', {
    output_path: outputDir,
    include_sensitive_data: includeSensitiveData,
  });
}

export async function importMigration(
  filePath: string,
  conflictStrategy: ConflictStrategy,
): Promise<MigrationImportResult> {
  return safeInvoke<MigrationImportResult>('import_migration', {
    file_path: filePath,
    conflict_strategy: conflictStrategy,
  });
}

export async function inspectMigration(
  filePath: string,
): Promise<MigrationPreview> {
  return safeInvoke<MigrationPreview>('inspect_migration', {
    file_path: filePath,
  });
}

// Post-import: MCP 서버 재인증
export async function reverifyMcpServers(): Promise<
  Record<string, 'success' | 'error' | 'skipped'>
> {
  return safeInvoke<Record<string, 'success' | 'error' | 'skipped'>>(
    'reverify_mcp_servers',
    {},
  );
}
```

### 4.2 마이그레이션 훅 (`src/features/migration/useMigration.ts`)

```typescript
import { useState, useCallback } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import {
  exportMigration,
  importMigration,
  inspectMigration,
  reverifyMcpServers,
  type MigrationPreview,
  type MigrationExportInfo,
  type MigrationImportResult,
  type ConflictStrategy,
} from '@/lib/backend/migration';

export type MigrationPhase =
  | 'idle'
  | 'selecting'
  | 'inspecting'
  | 'importing'
  | 'exporting'
  | 'complete'
  | 'error';

export interface UseMigrationReturn {
  phase: MigrationPhase;
  preview: MigrationPreview | null;
  exportInfo: MigrationExportInfo | null;
  importResult: MigrationImportResult | null;
  error: string | null;
  progress: number; // 0-100 (Tauri event 기반)
  selectedFilePath: string | null; // inspect 시 저장, import 시 사용
  selectedExportDir: string | null;
  includeSensitiveData: boolean;
  setIncludeSensitiveData: (val: boolean) => void;
  selectExportFile: () => Promise<void>;
  selectImportFile: () => Promise<void>;
  doExport: () => Promise<void>;
  doImport: (strategy: ConflictStrategy) => Promise<void>;
  doReverifyMcp: () => Promise<void>;
  reset: () => void;
}

export function useMigration(): UseMigrationReturn {
  const [phase, setPhase] = useState<MigrationPhase>('idle');
  const [preview, setPreview] = useState<MigrationPreview | null>(null);
  const [exportInfo, setExportInfo] = useState<MigrationExportInfo | null>(
    null,
  );
  const [importResult, setImportResult] =
    useState<MigrationImportResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState(0);
  const [selectedFilePath, setSelectedFilePath] = useState<string | null>(null);
  const [selectedExportDir, setSelectedExportDir] = useState<string | null>(
    null,
  );
  const [includeSensitiveData, setIncludeSensitiveData] =
    useState<boolean>(false);

  const reset = useCallback(() => {
    setPhase('idle');
    setPreview(null);
    setExportInfo(null);
    setImportResult(null);
    setError(null);
    setProgress(0);
    setSelectedFilePath(null);
    setSelectedExportDir(null);
    setIncludeSensitiveData(false);
  }, []);

  const selectExportFile = useCallback(async () => {
    setPhase('selecting');
    const selected = await open({
      title: '내보낼 폴더 선택',
      directory: true,
      multiple: false,
    });
    if (selected && typeof selected === 'string') {
      setSelectedExportDir(selected);
    }
    setPhase('idle');
  }, []);

  const selectImportFile = useCallback(async () => {
    setPhase('selecting');
    // tauri_plugin_dialog으로 .libragent-migration 파일 선택
    const selected = await open({
      title: '마이그레이션 파일 선택',
      filters: [{ name: 'Migration', extensions: ['libragent-migration'] }],
      multiple: false,
    });
    if (selected) {
      const path = typeof selected === 'string' ? selected : selected[0];
      setSelectedFilePath(path);
      await doInspect(path);
    }
  }, []);

  const doInspect = useCallback(async (filePath: string) => {
    setPhase('inspecting');
    setError(null);
    try {
      const result = await inspectMigration(filePath);
      setPreview(result);
      setSelectedFilePath(filePath);
      setPhase('idle');
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setPhase('error');
    }
  }, []);

  const doExport = useCallback(async () => {
    if (!selectedExportDir) {
      setError('저장 폴더가 선택되지 않았습니다.');
      return;
    }
    setPhase('exporting');
    setProgress(0);
    setError(null);
    try {
      const result = await exportMigration(
        selectedExportDir,
        includeSensitiveData,
      );
      setExportInfo(result);
      setPhase('complete');
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setPhase('error');
    }
  }, [selectedExportDir, includeSensitiveData]);

  const doImport = useCallback(
    async (strategy: ConflictStrategy) => {
      if (!selectedFilePath) return;
      setPhase('importing');
      setProgress(0);
      setError(null);
      try {
        const result = await importMigration(selectedFilePath, strategy);
        setImportResult(result);
        setPhase('complete');
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
        setPhase('error');
      }
    },
    [selectedFilePath],
  );

  const doReverifyMcp = useCallback(async () => {
    try {
      return await reverifyMcpServers();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      return {};
    }
  }, []);

  return {
    phase,
    preview,
    exportInfo,
    importResult,
    error,
    progress,
    selectedFilePath,
    selectedExportDir,
    includeSensitiveData,
    setIncludeSensitiveData,
    selectExportFile,
    selectImportFile,
    doExport,
    doImport,
    doReverifyMcp,
    reset,
  };
}
```

### 4.3 마이그레이션 페이지 UI 흐름

```
┌─────────────────────────────────────────────┐
│  📦 데이터 마이그레이션                     │
│  LibrAgent 설정, 어시스턴트, 스킬을         │
│  다른 기기間で 이전합니다                   │
├─────────────────────────────────────────────┤
│                                             │
│  ┌──────────────┐    ┌──────────────┐      │
│  │  📤 내보내기  │    │  📥 가져오기  │      │
│  │              │    │              │      │
│  │  LibrAgent   │    │  마이그레이션 │      │
│  │  설정을      │    │  파일 선택    │      │
│  │  아카이브로  │    │              │      │
│  │  내보냅니다  │    │  [파일 선택] │      │
│  │              │    │              │      │
│  │  [내보내기]  │    └──────────────┘      │
│  └──────────────┘                           │
│                                             │
├─────────────────────────────────────────────┤
│  🔒 민감 데이터 포함:                       │
│  ☐ API 키 및 인증 정보 (기본: 제외)        │
│  ☐ 포함 시 ZIP에 평문 API 키가 저장됨      │
│  가져오기 미리보기 (파일 선택 후 표시):      │
│  마이그레이션 아카이브 v1                    │
│  내보낸 시각: 2025-07-15 (v0.8.0 기준)      │
│  호환성: ✅ 호환                            │
│                                             │
│  포함 섹션:                                 │
│  ✓ Settings (42개 항목)                     │
│  ✓ Assistants (5개 항목)                    │
│  ✓ MCP Servers (3개 항목)                   │
│  ✓ Playbooks (2개 항목)                     │
│  ✓ Scheduled Tasks (1개 항목)               │
│  ✓ User Skills (7개 항목)                   │
│                                             │
│  충돌 해결 전략:                              │
│  ○ Skip (기존 유지)                         │
│  ○ Overwrite (기존 교체)                    │
│  ○ Merge (기존에 추가 — settings만 적용)    │
│                                             │
│  ⚠ merge는 settings에만 적용됩니다          │
│  (assistants/mcp_servers는 overwrite 권장)  │
│                                             │
│  [⚠ 가져오기 전 자동 백업이 생성됩니다]     │
│  [가져오기 실행]                            │
├─────────────────────────────────────────────┤
│  진행률 / 결과:                             │
│  ████████████░░░░ 75% (Tauri event 기반)    │
│  Settings: 42/42 ✓                          │
│  Assistants: 5/5 ✓                          │
│  MCP Servers: 3/3 ✓                         │
│  Playbooks: 2/2 ✓                           │
│  Scheduled Tasks: 1/1 ✓                     │
│  User Skills: 6/7 ⚠ (1개 충돌: skip)        │
│                                             │
│  [MCP 서버 재인증] (가져오기 후)            │
└─────────────────────────────────────────────┘
```

---

## 5. 구현 단계

### Phase 0: 기반

1. `src-tauri/src/commands/migration_commands.rs` 생성
2. `src-tauri/src/models/migration.rs`에 내보내기/가져오기 모델 정의
3. `src-tauri/src/lib.rs`에 커맨드 등록

### Phase 1: 백엔드 핵심

4. `export_migration` 구현 — P0 Repository 쿼리, JSON 직렬화, user_skills 복사 (symlink skip), ZIP
5. `inspect_migration` 구현 — ZIP Slip 방지, manifest hash 검증, item count 반환, 호환성 상태
6. `import_migration` 구현 — **단일 트랜잭션**, **임시 DB + atomic swap**, 충돌 해결, **Rollback**
7. `reverify_mcp_servers` 구현 — post-import MCP 검증 (Backend#8)
8. 버전 호환성 체크 (`check_compatibility`)

### Phase 2: 프론트엔드

9. `src/lib/backend/migration.ts` — Tauri 커맨드 래퍼 + `ConflictStrategy` enum
10. `src/features/migration/useMigration.ts` — 상태 관리 훅 + `selectedFilePath` 별도 관리
11. `src/features/migration/MigrationPage.tsx` — UI 컴포넌트 + 보안 경고
12. Settings 또는 네비게이션에 라우트 추가

### Phase 3: 테스트

13. 내보내기 → 검사 → 가져오기 순환 테스트 (atomicity 포함)
14. 충돌 해결 (skip/overwrite) 테스트 — merge는 settings만
15. 버전 호환성 (구버전→새버전, 새버전→구버전) 테스트
16. ZIP Slip 방어 테스트 (malicious ZIP)

---

## 6. 보안 및 개인정보

| 항목               | 정책                                                                                                         |
| ------------------ | ------------------------------------------------------------------------------------------------------------ |
| **API 키**         | settings에 포함 (Tauri crypto 정적 암호화). ZIP은 **평문**. UI에서 `include_sensitive_data` toggle 기본 제외 |
| **OAuth 토큰**     | 제외. 재인증 필요                                                                                            |
| **파일 포맷**      | ZIP 평문 JSON — 암호화 없음                                                                                  |
| **예상 파일 크기** | 10~100MB                                                                                                     |
| **symlink**        | export 시 무조건 skip (외부 파일 읽기 방지)                                                                  |
| **JSON 크기**      | 최대 20MB (단일 파일), 250MB (전체 ZIP decompressed)                                                         |
| **ZIP Slip**       | `SecurityValidator::validate_path_for_write()` 필수                                                          |
| **ZIP Bomb**       | in-memory parsing (extract 없음), decompressed size 250MB 제한                                               |

---

## 7. 버전 호환성

```rust
const CURRENT_FORMAT_VERSION: u32 = 1;
const MIN_COMPATIBLE_VERSION: u32 = 1;

fn check_compatibility(manifest_version: u32) -> CompatibilityStatus {
    if manifest_version < MIN_COMPATIBLE_VERSION {
        Incompatible { message: "지원하지 않는 포맷 v{manifest_version}. 최소 필요: v{MIN_COMPATIBLE_VERSION}" }
    } else if manifest_version > CURRENT_FORMAT_VERSION {
        NewerVersion { message: "새로운 버전의 LibrAgent로 만든 파일입니다. 일부 기능 사용 불가." }
    } else {
        Compatible
    }
}
```

---

## 8. 에러 처리

| 에러                | UI 메시지                                                               | 복구 방법                    |
| ------------------- | ----------------------------------------------------------------------- | ---------------------------- |
| ZIP Slip 탐지       | "불법 경로가 포함된 마이그레이션 파일입니다."                           | 재내보내기                   |
| JSON 크기 초과      | "마이그레이션 파일이 너무 큽니다. (최대 250MB)"                         | 재내보내기                   |
| 단일 파일 크기 초과 | "마이그레이션 파일 내 특정 설정 파일의 용량이 너무 큽니다. (최대 20MB)" | 재내보내기                   |
| 버전 불일치         | "호환되지 않는 마이그레이션 포맷입니다."                                | 버전 업그레이드/다운그레이드 |
| import 실패         | "가져오기 실패: {세부사항}. 백업이 생성되었습니다."                     | 백업 복원 또는 재시도        |
| Cron 유효성 실패    | "유효하지 않은 cron 표현식: {expression}"                               | 소스 수정 또는 skip          |
| 파일시스템 에러     | "{path}에 쓸 수 없습니다: {세부사항}"                                   | 권한 확인                    |

---

## 9. 향후 확장 (v1 범위 밖)

- **세션 수동 백업**: 별도 Tauri 커맨드로 SQLite 덤프
- **클라우드 동기화**: 마이그레이션 파일을 클라우드에 푸시/풀
- **선택적 가져오기**: 사용자가 가져올 섹션 직접 선택
- **마이그레이션 diff**: 가져오기 전에 변경될 내용 정확히 표시
- **압축**: 대용량 섹션에 zstd/gzip 적용

---

## 컨센서스 리뷰 참고

| 리뷰어           | 관점                  | 판정                    |
| ---------------- | --------------------- | ----------------------- |
| session-dfa42747 | Frontend & UX         | approve-with-caveats    |
| session-110aad4c | Backend Rust / DB     | approve-with-conditions |
| session-30598082 | Security & Edge Cases | approve-with-conditions |

**수렴된 결정**: `merge` 전략은 **settings만** 허용. 다른 모든 섹션은 `skip`/`overwrite`만.
