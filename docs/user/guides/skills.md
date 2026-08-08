---
title: 스킬 (Skills)
---

# 스킬 (Skills)

> 스킬은 `SKILL.md`가 있는 **재사용 절차**입니다. `@skill:이름`으로 불러오거나, 에이전트가 `<available_skills>`에서 고릅니다.  
> 앱 기본 제공분은 번들(`bundled_skills` → 런타임 `system_skills`). 직접 만든 스킬은 **scope**에 따라 다른 폴더에 둡니다.

---

## 스킬이란?

| | 설명 |
|---|------|
| **무엇인가** | `SKILL.md`(+ 선택적 `scripts/`·`references/`·`assets/`) 폴더. *언제·어떤 순서로* 할지 적은 운영 절차 |
| **무엇이 아닌가** | Settings 메뉴나 MCP 서버 자체는 아님. builtin/MCP를 **어떻게 쓸지** 알려 주는 레이어 |
| **누가 쓰나** | 사용자가 `@skill:…`로 넣거나, 에이전트가 상황에 맞게 참조 |

**builtin**이 실행기라면 **스킬**은 플레이북에 가깝습니다.  
MCP 설치 UI는 [Extensions](extensions.md) / [커스텀 MCP](custom-mcp.md)를 보세요.

---

## Scope (어디에 설치되나)

같은 이름의 스킬이 여러 scope에 있으면 **우선순위가 높은 쪽만** 남습니다(소문자 이름 기준 first-wins).

### 우선순위 (높음 → 낮음)

`workspace` → `assistant` → `agent import`(IDE 스킬) → `global`(user) → `system`(번들 미러)

### Scope 한눈에

**workspace** — 이 세션/프로젝트만

- 경로: <code v-pre>{workspace}/.libragent/skills/{name}/</code>
- 메모: 기본 추천. 지우기 쉬움.

**assistant** — 특정 어시스턴트만

- 경로: <code v-pre>{dataDir}/assistants/{assistantId}/skills/{name}/</code>
- 메모: Assistants 편집의 Skills 탭과 연동.

**global** (user) — 거의 모든 세션

- 경로: <code v-pre>{dataDir}/user_skills/{name}/</code>
- 메모: 전역 커스텀 스킬. “시스템에 넣고 싶다”면 여기 (아래 system 아님).

**system** (번들) — 앱이 제공

- 경로: <code v-pre>{dataDir}/system_skills/{name}/</code>
- 메모: `bundled_skills` 미러. 커스텀을 여기에 넣지 마세요 (앱 시작 시 정리됨).

**agent import** — 워크스페이스에 IDE 스킬이 있을 때

- 경로: `.cursor/skills/`, `.agents/skills/` 등
- 메모: 자동 발견.

`dataDir` 예: Linux `~/.local/share/com.fritzprix.libragent/`, macOS `~/Library/Application Support/com.fritzprix.libragent/`, Windows `%APPDATA%\com.fritzprix.libragent\`.

### 헷갈리기 쉬운 점

| 말 | 실제 |
|----|------|
| “시스템/글로벌에 넣고 싶다” | 커스텀은 **user_skills (global)** — **system_skills 아님** |
| “번들 스킬 수정” | 개발자가 `src-tauri/bundled_skills/`에 넣는 워크플로 |
| 워크스페이스 루트에 `my-skill/` | 스캔 안 됨. 반드시 <code v-pre>.libragent/skills/{name}/</code> |

### UI에서 scope

| 목적 | 어디 |
|------|------|
| 로컬 스킬 폴더 | **Settings → General → Skills Directory** 또는 **Extensions → Skills** |
| 어시스턴트별 스킬 | Assistants → **Skills** 탭 |
| 파일로 배포 | 아래 skill-deployer |

---

## 쓰는 방법 (`@skill:`)

1. Chat 세션 입력에 `@` → `@skill:`
2. 이름 선택 후 할 일을 적습니다.

```
@skill:deep-research
경쟁사 A/B 최근 발표를 비교해 Markdown 보고서로 정리해줘.
```

파일은 `@file:경로`로 넣을 수 있습니다. 목록은 **다음 에이전트 턴**에 갱신되는 경우가 많습니다.

---

## 스킬 만들기 · 배포하기 (메타 스킬)

| 단계 | 스킬 | 역할 |
|------|------|------|
| 1. 작성·검증 | `@skill:skill-creator` | frontmatter, `validate_skill.py --strict` |
| 2. 설치 | `@skill:skill-deployer` | workspace / global / assistant 경로에 복사 |

```
@skill:skill-creator
weekly-notes 스킬을 만들고 --strict 로 검증해줘.
```

```
@skill:skill-deployer
weekly-notes 를 workspace scope로 이 세션에 배포해줘.
```

`system_skills` / `bundled_skills`에는 **배포하지 않습니다.** 모르겠으면 scope는 **workspace**.

---

## 번들 스킬 카탈로그 (요약)

앱 기본 포함 (`src-tauri/bundled_skills/` → `system_skills/`). `@skill:` 자동완성이 최신입니다.

### 시작·환경

`setup-wizard`, `tool-installer`, `agent-init`, `computer-diagnosis`

### 멀티 에이전트 · 오케스트레이션

서브 에이전트(자식 세션)란 무엇이고 어떤 스킬을 고를지는 **[서브 에이전트 · 오케스트레이션](sub-agents.md)** 에서 다룹니다.

`delegate`, `teamwork`, `org`, `org-restructure`, `divide-conquer`, `hub-spoke`, `pipeline`, `consensus-delegation`, `gatekeeper`, `pair-programming`, `recruit`, `boost`

### 일정

`schedule`, `session-schedule`

### 조사·문서

`deep-research`, `knowledge-distiller`, `to-md`, `docx`, `pptx`, `data-viz`, `workspace-indexer`, `repo-wiki`, `soul-awakening`

### 개발·연동·제작

`git-workflow`, `bench`, `fine-tune`, `email-integration`, `calendar-mgmt`, `telegram-cli`, `x-cli`, `skill-creator`, `skill-deployer`, `tool-creator`, `playbook-creator`

### 자주 하는 조합

| 하고 싶은 일 | 추천 |
|--------------|------|
| 런타임 설치 | `@skill:setup-wizard` + App Wizard |
| MCP 추천 설치 | [Extensions](extensions.md) |
| MCP 커스텀/가져오기 | [커스텀 MCP](custom-mcp.md) · `@skill:tool-installer` |
| 내 절차를 스킬로 | `skill-creator` → `skill-deployer` |
| 자식 세션·팀 오케스트레이션 | [서브 에이전트 가이드](sub-agents.md) |
| 어시스턴트 설정 | [Assistants](assistants.md) |
| 반복 실행 | [Playbooks](playbooks.md) · [자동화](automation.md) |

---

## 관련 문서

- [서브 에이전트 · 오케스트레이션](sub-agents.md)
- [Extensions (MCP)](extensions.md)
- [커스텀 MCP](custom-mcp.md)
- [5분 시작 가이드](../getting-started/5-minute-tutorial.md)
- [에이전트 첫 대화](../getting-started/first-agent.md)
- [문제 해결](troubleshooting.md)
