# Agent-Workspace File Contract: Skill Paths & Deployment Mechanics

이 문서는 **에이전트(LLM)와 LibrAgent 워크스페이스 MCP 런타임 간의 파일 경로 해석 및 도구 호출 계약(Tool Calling Contract)**을 정의합니다.

스킬 시스템에서 사용되는 `@` 가상 별칭(Skill Aliases)의 런타임 해석 규칙, `workspace` 도구(`readFile`, `writeFile`, `editFile`)에서의 동작 규약, 그리고 스킬 배포 시 에이전트가 따라야 하는 워크플로우를 규정합니다.

---

## 1. 아키텍처 개요 및 계층 구조

LibrAgent는 스킬 저장소를 4가지 격리 계층(System, User, Assistant, Workspace)으로 관리합니다.
에이전트가 호스트 환경의 복잡한 물리 절대경로(OS별 AppData, `~/.local/share` 등)를 계산하지 않도록 가상 네임스페이스 별칭을 제공합니다.

| 계층 (Scope) | 실제 호스트 물리 디렉토리 | 표준 별칭 (Canonical) | 축약/포괄 별칭 (Umbrella) | 권한 계약 (Agent Access) |
| :--- | :--- | :--- | :--- | :--- |
| **System** (번들 스킬) | `{baseDataDir}/system_skills/` | `@system-skills/...` | `@skills/system/...` | **Read-Only** |
| **User** (전역 사용자 스킬) | `{baseDataDir}/user_skills/` | `@user-skills/...` | `@skills/user/...` | **Read-Only** |
| **Assistant** (어시스턴트 전용) | `{baseDataDir}/assistants/{id}/skills/` | `@assistant-skills/...` | `@skills/assistant/...` | **Read-Only** |
| **Workspace** (프로젝트 전용) | `{workspaceRoot}/.libragent/skills/` | `@workspace-skills/...` | `@skills/workspace/...` | **Read-Only (별칭 기준)** |

---

## 2. Read / List Contract (읽기 및 조회 계약)

### 2.1 완전 자동 해석 (Zero Absolute Path)
에이전트는 절대경로를 알거나 입력할 필요가 없으며, 시스템이 제공하는 별칭을 그대로 사용해야 합니다.

1. **컨텍스트 주입**:
   * 시스템 프롬프트의 `<available_skills>` 블록에 스킬 목록이 제공되며, `<location>` 태그에 선호 경로로 별칭이 명시됩니다.
     ```xml
     <skill source="global" origin="system">
       <name>docx</name>
       <description>Word document manipulation...</description>
       <location>@system-skills/docx/SKILL.md</location>
     </skill>
     ```
2. **도구 호출**:
   * 에이전트는 `workspace__readFile(path: "@system-skills/docx/SKILL.md")` 또는 `workspace__listDirectory(path: "@workspace-skills")`를 호출합니다.
3. **런타임 변환**:
   * Rust 백엔드(`src-tauri/src/mcp/builtin/workspace/workspace_server/path_validation.rs`의 `validate_read_path_with_skill_access`)에서 별칭 접두사를 감지하여 세션에 바인딩된 실제 물리 디렉토리로 안전하게 변환합니다.
4. **경로 정규화 허용**:
   * 에이전트 모델의 사소한 경로 포맷 변형(`./@system-skills/...`, `/@system-skills/...`, `/workspace/@system-skills/...`)도 런타임에서 자동으로 정규화하여 정상 해석합니다.

---

## 3. Write / Deploy Contract (쓰기 및 배포 계약)

### 3.1 스킬 별칭 쓰기 원천 차단 (`Skill aliases are read-only`)
에이전트가 `workspace__writeFile` 또는 `workspace__editFile`에 `@` 스킬 별칭 경로를 전달하면 **에러와 함께 즉시 거부(Reject)**됩니다.

```json
{
  "isError": true,
  "content": "Skill aliases are read-only: `@workspace-skills` is a managed skill reference for read/list only. Writing through skill aliases creates a literal `@…` directory under the workspace that skill discovery does not scan. Write workspace skills to `.libragent/skills/{name}/SKILL.md`, or deploy with skill-deployer into `.libragent/skills/` / `user_skills/` (never `system_skills/`)."
}
```

#### 차단 사유 및 설계 배경 (PR #1912 / commit `52b1c2544`)
* **Split-Brain 및 고아 디렉토리 방지**:
  * 만약 `@workspace-skills/...` 경로를 일반 파일 쓰기로 허용하면, 워크스페이스 내에 문자 그대로 `workspace/@workspace-skills/`라는 비표준 디렉토리가 생성됩니다.
  * 런타임 스킬 스캐너는 정규 경로(`.libragent/skills/`)만 탐색하므로, 에이전트는 스킬을 작성했다고 응답하지만 시스템은 스킬을 인식하지 못하는 불일치 버그가 발생합니다.
* **보안 경계 유지**:
  * `workspace` 파일 도구는 기본적으로 현재 세션 워크스페이스 루트 외부로의 쓰기를 차단하는 샌드박스 정책(`SecureFileManager`)을 가집니다. 스킬 별칭을 통해 상위 전역 디렉토리를 임의로 쓰게 허용할 경우 샌드박스 탈출 위험이 있습니다.

---

## 4. 에이전트 스킬 배포 워크플로우 규약

에이전트가 스킬을 새로 생성하거나 배포할 때는 대상 스코프에 따라 다음 계약을 준수해야 합니다.

### 4.1 워크스페이스 스코프 배포 (가장 권장됨)
* **대상 경로**: `.libragent/skills/<skill-name>/SKILL.md` (상대 경로)
* **도구**: `workspace__writeFile` 또는 `workspace__editFile`
* **동작**:
  1. 에이전트는 절대경로 없이 `.libragent/skills/<skill-name>/SKILL.md`로 파일 작성.
  2. 작성 완료 즉시 백엔드 스캐너 캐시가 무효화되어, 다음 호출부터 `@workspace-skills/<skill-name>/SKILL.md`로 읽기 가능.

### 4.2 전역(User) 스코프 배포
* **제약**: `workspace__writeFile`은 샌드박스 보안상 전역 앱 데이터 디렉토리 쓰기를 거부합니다.
* **해결 도구**: 메타 스킬인 `skill-deployer` CLI 실행
  ```bash
  python <skill-deployer-dir>/scripts/deploy_skill.py <소스폴더> --scope global
  ```
* **동작**: 스크립트 내부에서 OS별 데이터 디렉토리(`user_skills/`)를 감지하여 유효성 검사 후 배치합니다.

### 4.3 시스템(System) 스코프
* 앱 바이너리 번들 미러(`src-tauri/bundled_skills/`) 전용입니다.
* 에이전트는 시스템 스킬을 임의로 배포하거나 수정할 수 없습니다 (앱 재시작 시 자동 초기화됨).

---

## 5. 비교: `@teamwork` 계약과의 차이점

워크스페이스 도구는 `@teamwork` 별칭에 대해서는 상이한 정책을 적용합니다.

| 구분 | `@skills/...` / `@*-skills` | `@teamwork/...` |
| :--- | :--- | :--- |
| **목적** | 에이전트 지침 및 프롬프트 가이드 참조 | 협업 세션 산출물(계획, 아키텍처 문서 등) 공유 |
| **Read 계약** | ✅ 허용 (물리 디렉토리 자동 매핑) | ✅ 허용 (세션 팀워크 아티팩트 루트 자동 매핑) |
| **Write 계약** | ❌ **차단 (Read-Only)** | ✅ **허용** (팀워크 루트로 자동 리디렉션) |
| **경로 대체 수단** | `.libragent/skills/` (상대) 또는 `skill-deployer` | `@teamwork/...` 또는 `.libragent/teamwork/...` |

---

## 6. 결론 요약

1. **읽기(Read)**: 에이전트는 프롬프트의 `<location>`에 제공된 `@` 별칭을 사용하여 안전하게 스킬을 읽어야 하며, 호스트 절대경로를 알 필요가 없습니다.
2. **배포(Write)**: 스킬 별칭 쓰기는 차단되므로, 에이전트는 워크스페이스 스킬 배포 시 정규 상대경로인 **`.libragent/skills/<skill-name>/SKILL.md`**를 사용해야 하며, 전역 배포 시에는 **`skill-deployer`** 도구를 호출해야 합니다.
