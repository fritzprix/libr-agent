# Issue #1513: Per-Assistant Bundled Skills — 구현 계획

## 1. 개요

**문제**: 모든 assistant가 모든 bundled skill에 접근하며, assistant 정의가 Rust 코드에 hardcoded되어 있음.
**해결**: 파일 기반 선언형 assistant 정의 + assistant별 스킬 증분 배포.

## 2. 신규 디렉토리 구조

```
src-tauri/
└── bundled_assistants/          ← 신규 디렉토리
    ├── Master Mind/
    │   ├── prompt.md            ← mastermind_system_prompt() 내용
    │   └── mcp-config.json      ← assistant config JSON
    ├── Libr Assistant/
    │   ├── prompt.md
    │   └── mcp-config.json
    ├── Coding Expert/
    │   ├── prompt.md
    │   └── mcp-config.json
    └── App Wizard/
        ├── prompt.md
        └── mcp-config.json
```

## 3. 파일별 상세 계획

### 3.1 `bundled_assistants/Master Mind/prompt.md`

```markdown
You are Master Mind: the command orchestrator for complex, high-impact missions.
You coordinate strategy, delegate execution to specialist agents, and keep shared knowledge coherent under pressure.

PRIME DIRECTIVE:
Deliver reliable outcomes by combining planning discipline, evidence-based execution, and continuous situational awareness.

AUTONOMY CHART:

1. AI AGENCY: Respect specialist autonomy and decision quality. Do not micromanage execution details that can be handled by capable agents.
2. DELEGATION DEFAULT: Prefer delegation for efficiency and throughput.
3. DIRECT ACTION ALLOWED: Direct execution is always allowed when speed, clarity, or risk control justifies it.
4. RISK-AWARE CHOICE: Choose delegation vs direct action by expected reliability, latency, and token cost.
5. RECOVERY DUTY: If a path fails, provide immediate fallback and continue mission flow.
6. NO ZOMBIE MODE: Avoid rigid hard bans except explicit security/safety constraints.

COMMAND PROTOCOL:

1. MISSION CONTROL: Define objective, constraints, and success criteria before action.
2. ORCHESTRATION: Break work into tracked steps, assign priorities, and route work to the right specialist.
3. EVIDENCE FIRST: Never claim completion without verification (files, commands, tool outputs, current state).
4. KNOWLEDGE OPERATIONS: Capture critical findings (IDs, paths, decisions, risks) and reuse them deliberately.
5. REAL-TIME INTELLIGENCE: Pull fresh information via available tools when uncertainty exists; avoid stale assumptions.

ATTENTION ECONOMY:

- Keep active tool families minimal for each phase.
- Prefer delegation over direct multi-tool thrashing.
- Require explicit reason before switching tool domains.

KNOWLEDGE LOOP:

- Persist critical discoveries to shared memory.
- Retrieve and reconcile prior knowledge before major decisions.
- Prefer reusable knowledge over repeating expensive investigation.

SPECIALIST COORDINATION MODEL:

- Libr Agent: general field operations and cross-domain execution.
- Coding Expert: implementation/refactor/debug execution.
- App Wizard: environment, MCP, agent configuration execution.
- Master Mind: strategy, delegation, quality gates, conflict resolution.

OPERATING STYLE:

- Strategic, decisive, and explicit
- No vague status reports
- No hidden assumptions
- Clear next action at every step

FAILSAFE RULES:

- If required data is missing, ask precise questions.
- If a command or tool fails, report exact failure and recovery path.
- If risk escalates, recommend controlled rollback or containment.
```

### 3.2 `bundled_assistants/Master Mind/mcp-config.json`

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

### 3.3 `bundled_assistants/Libr Assistant/prompt.md`

```markdown
You are the Libr Agent: a field operations specialist in the Master Mind command structure.
Your primary directive is to provide ACCURATE, VERIFIED assistance by combining knowledge with action.

CORE PROTOCOLS:

1. VERIFICATION: Always use tools to verify facts about the current environment. Never rely solely on training data. If you cannot verify, state uncertainty explicitly.
2. ACTION INTEGRITY: Before editing, verify file exists and read content. After action, verify outcome and report actual results.
3. UNCERTAINTY: If user intent is ambiguous or tool results are unexpected, ask questions—do not hallucinate.

COMPLEX TASKS (3+ steps):

1. Define and record your overall objective
2. Break down into actionable steps and track progress
3. Save critical findings (file paths, IDs, discoveries) for later reference

CONTEXT MANAGEMENT:
Your conversation context is limited. For complex tasks: establish persistent goals, save critical findings to scratchpad (limit ~10 items), and reference saved information instead of re-gathering.

TEAM DOCTRINE:

- When a higher-level plan exists, execute your assigned part with discipline.
- Report concrete outcomes, blockers, and evidence for command-level decisions.

ATTENTION ECONOMY:

- Prefer the smallest viable toolset per step.
- Do not hop tools unless current evidence requires it.
- Finish one investigative thread before opening another.
```

### 3.4 `bundled_assistants/Libr Assistant/mcp-config.json`

```json
{
  "description": "Field operations specialist for verified research, execution, and cross-domain task delivery.",
  "mcpServerIds": [],
  "deletionProtected": true,
  "localServices": [],
  "allowedBuiltInServiceAliases": [
    "attachments",
    "workspace",
    "browser",
    "planning",
    "playbook"
  ]
}
```

### 3.5 `bundled_assistants/Coding Expert/prompt.md`

```markdown
You are the Coding Expert: an engineering execution specialist under Master Mind command.
You handle implementation-heavy software tasks with precision and evidence.

INTEGRITY PROTOCOLS:

1. READ BEFORE WRITE: Never edit code without reading current state first.
2. VERIFY: After edits, verify compilation/tests pass. Report EXACT errors if they fail.
3. NO BLIND EDITS: Always verify code context. Do not guess.

COMPLEX TASKS (multi-file/refactoring):

1. Define objective (e.g., "Refactor auth to JWT")
2. Break into steps, track progress, adjust plan as needed
3. Save critical info: file paths, function names, dependencies, architectural decisions

CONTEXT MANAGEMENT:
Your context is limited. For complex tasks: establish persistent goals, save code structure info to scratchpad (limit ~10 items), reference saved info instead of re-analyzing.

CORE COMPETENCIES:

- Analyze code structure and patterns before changes
- Consider system architecture and design patterns
- Apply SOLID principles and best practices
- Make surgical, incremental changes

TEAM DOCTRINE:

- When strategy is provided, translate it into safe, verifiable code changes.
- Return exact results, diffs, and technical risks for command-level review.

ATTENTION ECONOMY:

- Stay in code-analysis/edit/verification loop unless mission scope changes.
- Avoid unnecessary tool switching during implementation.
```

### 3.6 `bundled_assistants/Coding Expert/mcp-config.json`

```json
{
  "description": "Engineering execution specialist for implementation, refactoring, debugging, and verification.",
  "mcpServerIds": [],
  "deletionProtected": true,
  "localServices": [],
  "allowedBuiltInServiceAliases": [
    "workspace",
    "planning",
    "attachments",
    "playbook"
  ]
}
```

### 3.7 `bundled_assistants/App Wizard/prompt.md`

```markdown
You are the App Wizard: an environment and systems setup specialist under Master Mind command.
Your role is to help users configure the application, manage agents, and set up MCP servers.

CORE PRINCIPLES:

1. CONFIGURATION FOCUS: Configure settings only, not runtime behavior.
2. VERIFICATION: Verify system requirements before making changes.

COMPLEX TASKS (multi-step setup):

1. Define setup objective clearly
2. Break into configuration steps, track progress, verify each change
3. Save critical info: agent IDs/names, MCP configs (commands, paths, env vars), system requirements

CONTEXT MANAGEMENT:
Your context is limited. For complex setup: establish persistent goals, save configuration details to scratchpad (limit ~10 items), reference saved info instead of re-querying.

CAPABILITIES:

1. AGENTS: Create, update, list, search. Write detailed system prompts following best practices.
2. MCP SERVERS: Register, configure (args, paths, env vars), explain requirements.
3. ENVIRONMENT: Detect OS, verify dependencies, guide installation, validate readiness.

TEAM DOCTRINE:

- Execute setup plans reliably and surface operational risks early.
- Provide exact verification checkpoints for command-level go/no-go decisions.

ATTENTION ECONOMY:

- Stay focused on environment/configuration operations.
- Only escalate to broader tools when setup verification demands it.
```

### 3.8 `bundled_assistants/App Wizard/mcp-config.json`

```json
{
  "description": "Environment and configuration specialist for MCP setup, agent management, and system readiness.",
  "mcpServerIds": [],
  "deletionProtected": true,
  "localServices": [],
  "allowedBuiltInServiceAliases": [
    "setup-wizard",
    "tool",
    "agent",
    "workspace",
    "planning",
    "attachments"
  ]
}
```

## 4. Rust 코드 변경 상세

### 4.1 `src-tauri/src/services/assistant_init.rs` 리팩토링

#### 4.1.1 신규 데이터 구조

// Whitelist of allowed builtin service aliases (Security Lens recommendation)
const KNOWN_BUILTIN_SERVICES: &[&str] = &[
"planning", "knowledge", "browser", "workspace", "code-executor",
"attachments", "playbook", "agent", "setup-wizard", "tool",
];

#[derive(Debug, Clone, Deserialize)] #[serde(rename_all = "camelCase")] // Correctness Lens: prevents silent empty config on camelCase JSON
struct BundledAssistantConfig {
description: String, #[serde(default)]
mcp_server_ids: Vec<String>, #[serde(default = "default_false")]
deletion_protected: bool, #[serde(default)]
local_services: Vec<String>,
allowed_builtin_service_aliases: Vec<String>,
/// Security Lens: reject oversized configs to prevent memory exhaustion #[serde(default = "default_max_aliases")]
\_max_aliases: usize,
}

fn default_false() -> bool { false }
fn default_max_aliases() -> usize { 20 } #[derive(Debug, Clone)]
struct BundledAssistant {
name: String,
prompt: String,
config: BundledAssistantConfig,
}

````

#### 4.1.2 `load_bundled_assistants()` 함수

```rust
fn load_bundled_assistants(resource_dir: &Path) -> Result<Vec<BundledAssistant>, String> {
    let base = resource_dir.join("bundled_assistants");

    if !base.exists() {
        return Ok(Vec::new());
    }

    let mut assistants = Vec::new();
    for entry in std::fs::read_dir(&base).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let assistant_dir = entry.path();

        if !assistant_dir.is_dir() {
            continue;
        }

        // Security Lens: path traversal prevention
        let name = assistant_dir
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("Invalid assistant dir name")?
            .to_string();

        if name.contains("..") || name.contains('/') {
            log::warn!("Skipping assistant dir '{}' (path traversal attempt)", name);
            continue;
        }

        // Load prompt.md
        let prompt_path = assistant_dir.join("prompt.md");
        let prompt = std::fs::read_to_string(&prompt_path)
            .map_err(|e| format!("Failed to read prompt.md for {}: {}", name, e))?;

        // Security Lens: JSON size limit (max 64KB)
        let config_path = assistant_dir.join("mcp-config.json");
        let config_bytes = std::fs::read(&config_path)
            .map_err(|e| format!("Failed to read mcp-config.json for {}: {}", name, e))?;
        if config_bytes.len() > 64 * 1024 {
            log::warn!("Skipping {} (mcp-config.json exceeds 64KB)", name);
            continue;
        }

        let config: BundledAssistantConfig = serde_json::from_slice(&config_bytes)
            .map_err(|e| format!("Invalid mcp-config.json for {}: {}", name, e))?;

        // Security Lens: whitelist validation
        for alias in &config.allowed_builtin_service_aliases {
            if !KNOWN_BUILTIN_SERVICES.contains(&alias.as_str()) {
                log::warn!("Skipping {} (unknown builtin service alias: {})", name, alias);
                return Err(format!("Unknown builtin service alias '{}' in {}", alias, name));
            }
        }

        assistants.push(BundledAssistant { name, prompt, config });
    }

    // Sort by name for deterministic order
    assistants.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(assistants)
````

#### 4.1.3 `ensure_assistant_from_bundled()` 함수

```rust
async fn ensure_assistant_from_bundled(
    repo: &crate::repositories::SqliteAssistantRepository,
    bundled: &BundledAssistant,
) -> Result<(), String> {
    // Name 기준 조회 (UUID 아님)
    let exists = repo
        .check_assistant_exists(&bundled.name)
        .await
        .map_err(|e| format!("Failed to check for {}: {}", bundled.name, e))?;

    let mut config_value = if exists {
        // Existing: prompt/config 변경 감지 후 업데이트
        let assistants = repo
            .list_assistants()
            .await
            .map_err(|e| format!("Failed to list assistants: {}", e))?;

        let target = assistants
            .into_iter()
            .find(|a| a.name == bundled.name)
            .ok_or_else(|| format!("Assistant {} not found after existence check", bundled.name))?;

        serde_json::from_str::<Value>(&target.config).unwrap_or_else(|_| json!({}))
    } else {
        // New: config 생성
        json!({})
    };

    // Update description
    config_value["description"] = Value::String(bundled.config.description.clone());

    // Update systemPrompt (changes are applied)
    config_value["systemPrompt"] = Value::String(bundled.prompt.clone());

    // Update MCP config fields
    config_value["mcpServerIds"] = Value::Array(
        bundled.config.mcp_server_ids
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect()
    );
    config_value["deletionProtected"] = Value::Bool(bundled.config.deletion_protected);
    config_value["localServices"] = Value::Array(
        bundled.config.local_services
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect()
    );
    config_value["allowedBuiltInServiceAliases"] = Value::Array(
        bundled.config.allowed_builtin_service_aliases
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect()
    );

    if exists {
        // Update existing assistant
        let assistants = repo
            .list_assistants()
            .await
            .map_err(|e| format!("Failed to list assistants: {}", e))?;

        let target = assistants
            .into_iter()
            .find(|a| a.name == bundled.name)
            .ok_or_else(|| format!("Assistant {} disappeared", bundled.name))?;

        repo.update_assistant(&target.id, None, Some(config_value.to_string()))
            .await
            .map_err(|e| format!("Failed to update {}: {}", bundled.name, e))?;
    } else {
        // Create new assistant
        let id = uuid::Uuid::new_v4().to_string();
        repo.create_assistant(id, bundled.name.clone(), config_value.to_string())
            .await
            .map_err(|e| format!("Failed to create {}: {}", bundled.name, e))?;
    }

    Ok(())
}
```

#### 4.1.4 `ensure_default_assistants()` 리팩토링

```rust
pub async fn ensure_default_assistants() -> Result<(), String> {
    let bundled = load_bundled_assistants()?;

    if bundled.is_empty() {
        // Fallback: 기존 hardcoded 방식 (기존 assistant_init.rs 내용)
        log::warn!("No bundled assistants found, falling back to hardcoded defaults");
        return ensure_default_assistants_hardcoded().await;
    }

    let repo = crate::get_assistant_repository();

    for assistant in &bundled {
        log::info!("Ensuring assistant: {}", assistant.name);
        if let Err(e) = ensure_assistant_from_bundled(&repo, assistant).await {
            log::warn!("Failed to ensure assistant {}: {}", assistant.name, e);
            // Continue with next assistant (graceful degradation)
        }
    }

    Ok(())
}

// 기존 hardcoded 로직을 별도 함수로 분리 (fallback용)
#[allow(dead_code)]
async fn ensure_default_assistants_hardcoded() -> Result<(), String> {
    // 기존 assistant_init.rs의 ensure_default_assistants() 본문을 복사
    // ...
}
```

### 4.2 `src-tauri/src/lifecycle/app_setup.rs` 변경

#### 4.2.1 `spawn_assistant_skills_startup_work()` 함수 추가

```rust
fn spawn_assistant_skills_startup_work(bundled_assistants_dir: PathBuf, base_data_dir: PathBuf) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = sync_assistant_bundled_skills(&bundled_assistants_dir, &base_data_dir).await {
            log::warn!("⚠️  Failed to sync assistant bundled skills: {}", e);
        } else {
            info!("✅ Assistant bundled skills synchronized");
        }
    });
}

async fn sync_assistant_bundled_skills(
    bundled_assistants_dir: &Path,
    base_data_dir: &Path,
) -> Result<(), String> {
    // 1. assistant_init에서 로드한 assistant 목록을 여기서도 로드
    //    (중복 로딩 피하려면 load_bundled_assistants()를 shared module으로 이동)
    let assistants = crate::services::assistant_init::load_bundled_assistants()?;

    for assistant in assistants {
        let assistant_skills_dir = bundled_assistants_dir
            .join(&assistant.name)
            .join("bundled_skills");

        // assistant의 UUID를 찾아야 함 → DB 조회
        let repo = crate::get_assistant_repository();
        let assistants_db = repo.list_assistants().await.map_err(|e| e.to_string())?;
        let target_assistant = assistants_db
            .into_iter()
            .find(|a| a.name == assistant.name)
            .ok_or_else(|| format!("Assistant {} not found in DB", assistant.name))?;

        let target_skills_dir = base_data_dir
            .join("assistants")
            .join(&target_assistant.id)
            .join("skills");

        // 2. assistant_skills_dir 스캔 → 각 skill 디렉토리별 sync
        if !assistant_skills_dir.exists() {
            continue; // skill이 없는 assistant는 skip
        }

        for skill_entry in std::fs::read_dir(&assistant_skills_dir).map_err(|e| e.to_string())? {
            let skill_entry = skill_entry.map_err(|e| e.to_string())?;
            let skill_dir = skill_entry.path();

            if !skill_dir.is_dir() {
                continue;
            }

            let skill_name = skill_dir
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or("Invalid skill dir name")?
                .to_string();

            let target_skill_dir = target_skills_dir.join(&skill_name);

            // 3. hash 비교 + 증분 배포 (기존 replace_skill_directory_atomically 패턴 재사용)
            let source_hash = hash_skill_directory(&skill_dir)?;

            // Check if target exists and matches
            let needs_update = if target_skill_dir.exists() {
                let target_hash = hash_skill_directory(&target_skill_dir)?;
                source_hash != target_hash
            } else {
                true
            };

            if needs_update {
                replace_skill_directory_atomically(&skill_dir, &target_skill_dir)?;
                log::info!("Synced skill '{}' for assistant '{}'", skill_name, assistant.name);
            }
        }
    }

    Ok(())
}
```

#### 4.2.2 기존 `spawn_managed_skills_startup_work()` 호출 측 수정

```rust
// 기존 코드에서:
spawn_managed_skills_startup_work(bundled_skills_dir.clone(), system_skills_dir.clone());

// 아래를 추가:
spawn_assistant_skills_startup_work(bundled_assistants_dir.clone(), base_data_dir.clone());
```

### 4.3 `src-tauri/build.rs` 변경

```rust
fn main() {
    println!("cargo:rerun-if-changed=bundled_skills");
    println!("cargo:rerun-if-changed=bundled_assistants");

    if let Err(error) = sync_bundled_skills_into_profile_output() {
        panic!(
            "failed to mirror bundled_skills into the target profile output directory: {}",
            error
        );
    }

    // NEW: mirror bundled_assistants
    if let Err(error) = sync_bundled_assistants_into_profile_output() {
        panic!(
            "failed to mirror bundled_assistants into the target profile output directory: {}",
            error
        );
    }

    tauri_build::build()
}

#[path = "build_support/bundled_skills.rs"]
mod bundled_skills;

fn sync_bundled_assistants_into_profile_output() -> io::Result<()> {
    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").map_err(|error| io::Error::other(error.to_string()))?,
    );
    let source_dir = manifest_dir.join("bundled_assistants");
    if !source_dir.exists() {
        return Ok(());
    }

    let out_dir =
        PathBuf::from(env::var("OUT_DIR").map_err(|error| io::Error::other(error.to_string()))?);
    let Some(profile_dir) = profile_output_dir(&out_dir) else {
        return Err(io::Error::other(format!(
            "Failed to resolve target profile directory from OUT_DIR={}",
            out_dir.display()
        )));
    };

    let deployed_dir = profile_dir.join("bundled_assistants");
    bundled_skills::mirror_bundled_assistants(&source_dir, &deployed_dir)
}
```

### 4.4 `src-tauri/build_support/bundled_skills.rs` 변경

```rust
// 기존 mirror_bundled_skills() 유지 + 신규 함수 추가

pub fn mirror_bundled_assistants(source_dir: &Path, target_dir: &Path) -> io::Result<()> {
    if !source_dir.exists() {
        if target_dir.exists() {
            fs::remove_dir_all(target_dir)?;
        }
        return Ok(());
    }

    if target_dir.exists() {
        fs::remove_dir_all(target_dir)?;
    }
    fs::create_dir_all(target_dir)?;

    for entry in fs::read_dir(source_dir)? {
        let entry = entry?;
        let src_path = entry.path();
        if !src_path.is_dir() {
            continue;
        }

        // 각 assistant 디렉토리는 prompt.md + mcp-config.json 필수
        if !src_path.join("prompt.md").is_file() || !src_path.join("mcp-config.json").is_file() {
            log::warn!(
                "Skipping assistant dir {} (missing prompt.md or mcp-config.json)",
                src_path.display()
            );
            continue;
        }

        copy_dir_recursive(&src_path, &target_dir.join(entry.file_name()))?;
    }

    Ok(())
}
```

### 4.5 `scripts/validate-assistant-skills.cjs` (신규)

```javascript
#!/usr/bin/env node
/**
 * Validate bundled_assistants/ structure at build time.
 *
 * Checks:
 * 1. Each {assistant_name}/ has prompt.md
 * 2. Each {assistant_name}/ has mcp-config.json
 * 3. mcp-config.json fields are valid
 * 4. bundled_skills/ subdirectories have SKILL.md
 * 5. No orphaned directories (in bundled_assistants/ but not in assistant_init.rs)
 * 6. No missing directories (in assistant_init.rs but not in bundled_assistants/)
 */

const fs = require('fs');
const path = require('path');

const KNOWN_BUILTIN_SERVICES = new Set([
  'planning',
  'knowledge',
  'browser',
  'workspace',
  'code-executor',
  'attachments',
  'playbook',
  'agent',
  'setup-wizard',
  'tool',
]);

function validate() {
  const root = path.join(__dirname, '..', 'src-tauri');
  const assistantsDir = path.join(root, 'bundled_assistants');
  const assistantInitPath = path.join(
    root,
    'src',
    'services',
    'assistant_init.rs',
  );

  let errors = [];
  let warnings = [];

  // 1. Check bundled_assistants/ exists
  if (!fs.existsSync(assistantsDir)) {
    console.error('❌ bundled_assistants/ directory does not exist');
    process.exit(1);
  }

  // 2. Enumerate assistant directories
  const assistantDirs = fs.readdirSync(assistantsDir).filter((name) => {
    const p = path.join(assistantsDir, name);
    return fs.statSync(p).isDirectory();
  });

  if (assistantDirs.length === 0) {
    console.error('❌ No assistant directories found in bundled_assistants/');
    process.exit(1);
  }

  // 3. Parse assistant_init.rs to find hardcoded assistant names
  const initContent = fs.readFileSync(assistantInitPath, 'utf8');
  const hardcodedNames = new Set();
  // Look for patterns like: let name = "Master Mind"; or check_assistant_exists("Master Mind")
  const namePattern = /["']([A-Za-z][A-Za-z0-9 ]+)["]/g;
  let match;
  // Extract names from known patterns in assistant_init.rs
  const knownPatterns = [
    /check_assistant_exists\(["']([^"']+)["']\)/g,
    /ensure_assistant_description\([^,]+, ["']([^"']+)["']/g,
    /ensure_assistant_system_prompt\([^,]+, ["']([^"']+)["']/g,
  ];
  for (const pattern of knownPatterns) {
    while ((match = pattern.exec(initContent)) !== null) {
      hardcodedNames.add(match[1]);
    }
  }

  const fileBasedNames = new Set(assistantDirs);

  // 5. Orphaned check
  const orphans = [...hardcodedNames].filter(
    (name) => !fileBasedNames.has(name),
  );
  if (orphans.length > 0) {
    errors.push(
      `Orphaned hardcoded assistants (in assistant_init.rs but not in bundled_assistants/): ${orphans.join(', ')}`,
    );
  }

  // 6. Missing check
  const missing = [...fileBasedNames].filter(
    (name) => !hardcodedNames.has(name),
  );
  if (missing.length > 0) {
    warnings.push(
      `New assistants in bundled_assistants/ not yet in assistant_init.rs (may be intentional): ${missing.join(', ')}`,
    );
  }

  // 4. Validate each assistant directory
  for (const assistantName of assistantDirs) {
    const assistantPath = path.join(assistantsDir, assistantName);

    // prompt.md
    const promptPath = path.join(assistantPath, 'prompt.md');
    if (!fs.existsSync(promptPath)) {
      errors.push(`${assistantName}/ missing prompt.md`);
      continue;
    }
    const promptContent = fs.readFileSync(promptPath, 'utf8');
    if (promptContent.trim().length < 50) {
      warnings.push(
        `${assistantName}/prompt.md seems too short (${promptContent.trim().length} chars)`,
      );
    }

    // mcp-config.json
    const configPath = path.join(assistantPath, 'mcp-config.json');
    if (!fs.existsSync(configPath)) {
      errors.push(`${assistantName}/ missing mcp-config.json`);
      continue;
    }
    try {
      const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));

      // Required fields
      if (
        !config.description ||
        typeof config.description !== 'string' ||
        config.description.trim().length === 0
      ) {
        errors.push(
          `${assistantName}/mcp-config.json: "description" is required and must be non-empty string`,
        );
      }
      if (!Array.isArray(config.allowedBuiltInServiceAliases)) {
        errors.push(
          `${assistantName}/mcp-config.json: "allowedBuiltInServiceAliases" is required and must be an array`,
        );
      } else {
        for (const alias of config.allowedBuiltInServiceAliases) {
          if (!KNOWN_BUILTIN_SERVICES.has(alias)) {
            warnings.push(
              `${assistantName}/mcp-config.json: unknown builtin service alias "${alias}"`,
            );
          }
        }
      }
      if (
        config.mcpServerIds !== undefined &&
        !Array.isArray(config.mcpServerIds)
      ) {
        errors.push(
          `${assistantName}/mcp-config.json: "mcpServerIds" must be an array`,
        );
      }
      if (
        config.deletionProtected !== undefined &&
        typeof config.deletionProtected !== 'boolean'
      ) {
        errors.push(
          `${assistantName}/mcp-config.json: "deletionProtected" must be a boolean`,
        );
      }
      if (
        config.localServices !== undefined &&
        !Array.isArray(config.localServices)
      ) {
        errors.push(
          `${assistantName}/mcp-config.json: "localServices" must be an array`,
        );
      }
    } catch (e) {
      errors.push(
        `${assistantName}/mcp-config.json: invalid JSON — ${e.message}`,
      );
    }

    // bundled_skills/ (optional but validated if present)
    const skillsDir = path.join(assistantPath, 'bundled_skills');
    if (fs.existsSync(skillsDir)) {
      const skillDirs = fs.readdirSync(skillsDir).filter((name) => {
        const p = path.join(skillsDir, name);
        return fs.statSync(p).isDirectory();
      });
      for (const skillName of skillDirs) {
        const skillPath = path.join(skillsDir, skillName);
        if (!fs.existsSync(path.join(skillPath, 'SKILL.md'))) {
          errors.push(
            `${assistantName}/bundled_skills/${skillName}/ missing SKILL.md`,
          );
        }
      }
    }
  }

  // Report
  console.log('\n--- bundled_assistants/ Validation Report ---');
  console.log(`Checked ${assistantDirs.length} assistant directories`);

  if (warnings.length > 0) {
    console.log(`\n⚠️  ${warnings.length} warning(s):`);
    for (const w of warnings) console.log(`  ⚠️  ${w}`);
  }

  if (errors.length > 0) {
    console.log(`\n❌ ${errors.length} error(s):`);
    for (const e of errors) console.log(`  ❌ ${e}`);
    console.log('\nValidation FAILED. Fix errors before committing.\n');
    process.exit(1);
  }

  console.log('\n✅ Validation PASSED\n');
}

validate();
```

## 5. 마이그레이션 단계별 실행 계획

### Phase 1: 디렉토리 구조 생성 및 데이터 이동 (1-2시간)

1. `src-tauri/bundled_assistants/` 디렉토리 생성
2. 4개 assistant 디렉토리 생성 (Master Mind, Libr Assistant, Coding Expert, App Wizard)
3. 각 디렉토리에 prompt.md, mcp-config.json 생성
4. 기존 `assistant_init.rs`의 hardcoded 데이터를 파일로 추출

### Phase 2: Rust 코드 변경 (3-4시간)

1. `assistant_init.rs`에 `BundledAssistantConfig`, `BundledAssistant` 구조체 추가
2. `load_bundled_assistants()` 함수 구현
3. `ensure_assistant_from_bundled()` 함수 구현
4. `ensure_default_assistants()` 리팩토링 (file-based + fallback)
5. `app_setup.rs`에 `spawn_assistant_skills_startup_work()`, `sync_assistant_bundled_skills()` 추가
6. `build.rs`에 `sync_bundled_assistants_into_profile_output()` 추가
7. `build_support/bundled_skills.rs`에 `mirror_bundled_assistants()` 추가

### Phase 3: 빌드 검증 (1-2시간)

1. `scripts/validate-assistant-skills.cjs` 생성
2. `package.json`에 `validate:assistants` 스크립트 추가
3. `refactor:validate` 파이프라인에 통합
4. `pnpm refactor:validate` 실행 및 통과 확인

### Phase 4: 테스트 (1시간)

1. 새 assistant 추가 시 자동 배포 검증
2. 빌드 타임 검증 (orphaned/missing detection)
3. 기존 assistant 동작 unchanged 검증

## 6. 기존 코드와의 통합 전략

### 6.1 `sync_managed_system_skills_snapshot()` 유지

기존 전역 시스템 스킬 배포는 **삭제하지 않고 유지**. assistant별 스킬 배포는 별도 파이프.

### 6.2 Fallback 전략

`load_bundled_assistants()`가 빈 배열을 반환하면 기존 hardcoded 로직으로 fallback. 빌드 실패 시 앱이 시작되지 않는 것을 방지.

### 6.3 기존 helper 함수 재사용

`ensure_assistant_description()`, `ensure_assistant_system_prompt()`는 file-based 버전에서도 재사용 가능.

## 7. Risk & Mitigation

| Risk                       | 영향도  | 완화 방안                                                               |
| -------------------------- | ------- | ----------------------------------------------------------------------- |
| 파일 경로 차이 (개발/배포) | 🔴 높음 | `app_dir.resource_dir()` 사용, `bundled_assistants/`를 resources로 복사 |
| mcp-config.json 파싱 실패  | 🟡 중형 | individual assistant loading에서 `warn!` + continue                     |
| 기존 assistant UUID 변경   | 🟡 중형 | name 기준 매핑, UUID는 runtime 유지                                     |
| 빌드 스크립트 변경 실패    | 🟡 중형 | 디렉토리 미존재 시 `return Ok(())`                                      |
| 증분 배포 race condition   | 🟢 낮음 | `replace_skill_directory_atomically()` atomic 패턴 재사용               |
