# Per-Assistant Bundled Skills

## Why

### 현재 문제

1. **모든 assistant가 모든 bundled skill에 접근** — `sync_managed_system_skills_snapshot()`이 `bundled_skills/`의 모든 스킬을 전역 `system_skills/`로 배포. 각 assistant의 역할과 관련 없는 스킬도 모두 노출됨.

2. **assistant 정의가 Rust 코드에 hardcoded** — `src-tauri/src/services/assistant_init.rs`에서 `systemPrompt`, `mcp-config`, `allowedBuiltInServiceAliases` 등이 문자열 리터럴로 정의. 새 assistant 추가/수정을 위해 Rust 재컴파일이 필요.

3. **per-assistant skill 할당이 불가능** — assistant별로 필요한 스킬만 분리하여 배포할 메커니즘이 없음.

4. **빌드 타임 검증 부재** — assistant 이름과 실제 bundled skill 디렉토리 간 불일치를 잡을 방법이 없음.

### 해결 목표

- 파일 기반 선언형 assistant 정의 (Rust 코드에서 분리)
- assistant별 필수 스킬만 배포
- 빌드 타임에 구조/참조 검증
- 새 assistant 추가 시 파일 복사/수정만으로 가능

## What

### 새로운 디렉토리 구조

```
bundled_assistants/
├── Master Mind/
│   ├── prompt.md                          # system_prompt 텍스트
│   ├── mcp-config.json                    # assistant config JSON
│   └── bundled_skills/                    # 이 assistant 전용 스킬
│       ├── delegate/
│       │   └── SKILL.md
│       ├── divide-conquer/
│       │   └── SKILL.md
│       └── pipeline/
│           └── SKILL.md
├── Coding Expert/
│   ├── prompt.md
│   ├── mcp-config.json
│   └── bundled_skills/
│       ├── workspace/
│       │   └── SKILL.md
│       └── planning/
│           └── SKILL.md
├── App Wizard/
│   ├── prompt.md
│   ├── mcp-config.json
│   └── bundled_skills/
│       └── ...
└── Libr Assistant/
    ├── prompt.md
    ├── mcp-config.json
    └── bundled_skills/
        └── ...
```

### mcp-config.json 스키마

```json
{
  "description": "Command orchestrator that plans strategy, delegates to specialists, and enforces quality gates.",
  "mcpServerIds": [],
  "deletionProtected": true,
  "localServices": [],
  "allowedBuiltInServiceAliases": [
    "planning",
    "attachments",
    "playbook",
    "agent"
  ]
}
```

필수 필드: `description`, `allowedBuiltInServiceAliases`
선택 필드: `mcpServerIds`, `deletionProtected` (기본값: `false`), `localServices`

### 동작 흐름

```
앱 시작
  │
  ├─ scan_bundled_assistants("bundled_assistants/")
  │   └─ 디렉토리 스캔 → assistant 목록 생성
  │
  ├─ ensure_default_assistants()
  │   ├─ DB에서 assistant 존재 확인 (name 기준)
  │   ├─ 없으면: prompt.md + mcp-config.json으로 생성
  │   └─ 있으면: prompt/config 변경 시 업데이트
  │
  └─ sync_assistant_bundled_skills()
      ├─ 각 assistant의 bundled_assistants/{name}/bundled_skills/ 스캔
      ├─ <data>/assistants/{uuid}/skills/{skill}/ 에 복사
      └─ 변경 감지 (hash 기반) + 증분 배포
```

## How

### 1. 디렉토리 구조 생성

```bash
# 신규 디렉토리 생성
mkdir -p "bundled_assistants/Master Mind/bundled_skills"
mkdir -p "bundled_assistants/Coding Expert/bundled_skills"
mkdir -p "bundled_assistants/App Wizard/bundled_skills"
mkdir -p "bundled_assistants/Libr Assistant/bundled_skills"
```

각 assistant 디렉토리 내:

- `prompt.md`: 현재 `assistant_init.rs`의 system_prompt 문자열을 마크다운 파일로 분리
- `mcp-config.json`: 현재 config JSON을 파일로 분리
- `bundled_skills/`: assistant별로 필요한 스킬 디렉토리만 복사

### 2. Rust 코드 변경

**`src-tauri/src/services/assistant_init.rs`**

```rust
// BEFORE: hardcoded
let system_prompt = r#"You are Master Mind: ..."#;
let config = json!({ "description": "...", "systemPrompt": system_prompt, ... });

// AFTER: file-based
fn load_bundled_assistants() -> Result<Vec<BundledAssistant>, String> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("bundled_assistants");
    // 디렉토리 스캔 → prompt.md + mcp-config.json 파싱
}

async fn ensure_default_assistants() -> Result<(), String> {
    let assistants = load_bundled_assistants()?;
    for a in assistants {
        ensure_assistant_from_bundled(&repo, &a).await?;
    }
    Ok(())
}
```

**`src-tauri/src/lifecycle/app_setup.rs`**

```rust
// NEW 함수 추가
fn spawn_assistant_skills_startup_work(bundled_assistants_dir: PathBuf) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = sync_assistant_bundled_skills(&bundled_assistants_dir).await {
            log::warn!("⚠️  Failed to sync assistant bundled skills: {}", e);
        }
    });
}

async fn sync_assistant_bundled_skills(
    bundled_assistants_dir: &Path,
) -> Result<(), String> {
    // 1. DB에서 모든 assistant 조회
    // 2. assistant.name → bundled_assistants/{name}/ 매핑
    // 3. bundled_assistants/{name}/bundled_skills/{skill}/ →
    //    <data>/assistants/{uuid}/skills/{skill}/ 복사
    // 4. 변경 감지 + 증분 배포
}
```

### 3. 빌드 스크립트 변경

**`src-tauri/build_support/bundled_skills.rs`**

```rust
// 기존: bundled_skills/ mirror
// 추가: bundled_assistants/ 디렉토리 복사 (기존 mirror와 별도)

pub fn mirror_bundled_assistants(source_dir: &Path, target_dir: &Path) -> io::Result<()> {
    // bundled_assistants/ → <target>/bundled_assistants/ 복사
    // _assistant/ 구조와 달리 flat directory structure 유지
}
```

### 4. 빌드 타임 검증

**`scripts/validate-assistant-skills.cjs`** (신규)

```javascript
// 검증 항목:
// 1. 각 {assistant_name}/ 디렉토리에 prompt.md 존재?
// 2. 각 {assistant_name}/ 디렉토리에 mcp-config.json 존재?
// 3. mcp-config.json 필드 유효성?
//    - description: string, non-empty
//    - allowedBuiltInServiceAliases: string[], each in known set
//    - deletionProtected: boolean (default false)
//    - mcpServerIds: string[]
// 4. bundled_skills/ 내 스킬 디렉토리 존재?
//    - 각 하위 디렉토리에 SKILL.md 존재?
// 5. orphaned 디렉토리? (bundled_assistants/에 있지만 assistant_init.rs에 없는)
//    → Error
// 6. 누락된 디렉토리? (assistant_init.rs에 있지만 bundled_assistants/에 없는)
//    → Error
```

**`package.json` scripts에 추가:**

```json
{
  "scripts": {
    "validate:assistants": "node scripts/validate-assistant-skills.cjs",
    "refactor:validate": "pnpm lint && pnpm format && pnpm tauri check && pnpm validate:assistants && pnpm dead-code"
  }
}
```

### 5. 기존 assistant_init.rs 마이그레이션

기존 hardcoded 데이터를 `bundled_assistants/`로 이동:

| 기존 코드                    | 새 파일                                             |
| ---------------------------- | --------------------------------------------------- |
| `mastermind_system_prompt()` | `bundled_assistants/Master Mind/prompt.md`          |
| `Libr Assistant` config      | `bundled_assistants/Libr Assistant/mcp-config.json` |
| `Coding Expert` config       | `bundled_assistants/Coding Expert/mcp-config.json`  |
| `App Wizard` config          | `bundled_assistants/App Wizard/mcp-config.json`     |

각 assistant별 `bundled_skills/`에 필요한 스킬 복사.

## Migration Plan

### Phase 1: 구조 생성 및 데이터 이동

1. `bundled_assistants/` 디렉토리 구조 생성
2. 기존 assistant 정의 파일로 마이그레이션
3. 각 assistant별 `bundled_skills/` 구성

### Phase 2: Rust 코드 변경

1. `scan_bundled_assistants()` 함수 구현
2. `ensure_default_assistants()` 리팩토링
3. `sync_assistant_bundled_skills()` 함수 추가
4. `app_setup.rs`에 startup spawn 추가

### Phase 3: 빌드/검증

1. 빌드 스크립트에 `mirror_bundled_assistants()` 추가
2. `scripts/validate-assistant-skills.cjs` 구현
3. `pnpm refactor:validate` 파이프라인에 통합

### Phase 4: 테스트

1. 새 assistant 추가 시 자동 배포 검증
2. 빌드 타임 검증 (orphaned/missing detection)
3. 기존 assistant 동작 unchanged 검증

## Risks & Mitigations

| 리스크                               | 완화 방안                                              |
| ------------------------------------ | ------------------------------------------------------ |
| 파일 경로 오류 (개발/배포 환경 차이) | `env!("CARGO_MANIFEST_DIR")` 사용, 상대 경로 기반      |
| mcp-config.json 파싱 실패            | graceful degradation, warning 로그 후 앱 계속 실행     |
| 기존 assistant UUID 변경             | assistant name 기준 매핑, UUID는 runtime 생성 유지     |
| bundled_assistants/ 누락             | 빌드 타임 검증 Error로 차단                            |
| 스킬 복사 실패                       | warning 로그, 앱 계속 실행 (system_skills와 동일 패턴) |

## Open Questions

1. **mcp-config.json의 `systemPrompt` 필드 불필요?** — `prompt.md`가 sole source가 되므로 config에서 제거 가능.
2. **기존 assistant 데이터 마이그레이션** — 기존 DB에 저장된 assistant config를 새 포맷으로 변환할 필요 없음 (name 기준 매핑이므로).
3. **UI에서의 assistant 관리** — 향후 UI에서 assistant 생성 시 `bundled_assistants/` 구조를 참고하여 생성하면 일관성 유지 가능.
