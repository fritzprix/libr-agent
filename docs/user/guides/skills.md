---
title: 스킬 (Skills)
description: '스킬이 누구에게 적용되는지, @skill 호출, 만들기·배포(skill-creator / skill-deployer), 번들 스킬 요약을 설명합니다.'
---

# 스킬 (Skills)

> 스킬은 에이전트가 따르는 **재사용 절차**입니다. 채팅에서 `@skill:이름`으로 부르거나, 에이전트가 상황에 맞게 고릅니다.  
> 앱에 기본으로 들어 있는 스킬과, 내가 만들어 **이 프로젝트 / 이 어시스턴트 / 내 계정 전체**에 두는 스킬이 있습니다.

---

## 스킬이란?

|                   | 설명                                                                                 |
| ----------------- | ------------------------------------------------------------------------------------ |
| **무엇인가**      | “이런 일일 때 이렇게 하라”고 적어 둔 **운영 절차** (플레이북에 가깝음)               |
| **무엇이 아닌가** | Settings 메뉴나 MCP(확장 도구) 자체가 아님. 이미 있는 도구를 **어떻게 쓸지** 알려 줌 |
| **누가 쓰나**     | 사용자가 `@skill:…`로 넣거나, 에이전트가 알아서 참고                                 |

도구(내장·MCP)가 **실행기**라면, 스킬은 **하는 방법**입니다.  
MCP 설치는 [Extensions](extensions.md) / [커스텀 MCP](custom-mcp.md)를 보세요.

---

## 누구에게 적용되나

스킬을 둘 때(또는 deployer에 맡길 때) **적용 범위**를 고릅니다.

| 적용 범위                     | 의미                          | 이럴 때                                    |
| ----------------------------- | ----------------------------- | ------------------------------------------ |
| **이 프로젝트(워크스페이스)** | 지금 작업 폴더·이 세션 쪽만   | 기본 추천. 시험·프로젝트 전용 절차         |
| **이 어시스턴트**             | 고른 어시스턴트 프로필만      | 코딩 전용 / 리서치 전용처럼 역할이 다를 때 |
| **내 계정 전체(글로벌)**      | 거의 모든 세션에서 사용       | 자주 쓰는 나만의 절차를 항상 쓰고 싶을 때  |
| **앱 기본 제공**              | LibrAgent가 미리 넣어 둔 스킬 | 직접 만들지 않음. 목록은 아래 카탈로그     |

같은 이름 스킬이 여러 범위에 있으면 **더 좁은 쪽**(프로젝트 → 어시스턴트 → 계정 전체 → 앱 기본)이 우선합니다.

프로젝트 폴더에 Cursor 등 IDE 스킬이 있으면 LibrAgent가 함께 찾아 쓸 수 있습니다.

### 화면에서 확인

| 목적                   | 어디                                          |
| ---------------------- | --------------------------------------------- |
| 스킬 목록·폴더 관리    | 사이드바 **Extensions → Skills**              |
| 어시스턴트에 묶인 스킬 | **Assistants** → 편집 → **Skills** 탭         |
| 새로 만들어 두기       | 아래 **만들기 · 배포하기** (`skill-deployer`) |

### 헷갈리기 쉬운 점

| 말                                  | 실제                                                                                 |
| ----------------------------------- | ------------------------------------------------------------------------------------ |
| “시스템에 넣고 싶다”                | **내 계정 전체(글로벌)** 에 두면 됩니다. 앱 기본 제공 영역에 직접 넣지 마세요.       |
| “앱에 들어 있는 스킬을 고치고 싶다” | 앱이 다시 덮어쓸 수 있습니다. 같은 절차는 **내 스킬**로 만들어 배포하세요.           |
| “폴더만 복사하면 되나?”             | 가능은 하지만 위치가 틀리기 쉽습니다. **`skill-deployer`에 맡기는 것**을 권장합니다. |

---

## 쓰는 방법 (`@skill:`)

1. Chat 입력에 `@` → `@skill:` 선택
2. 이름 고른 뒤 할 일을 적습니다.

```
@skill:deep-research
경쟁사 A/B 최근 발표를 비교해 Markdown 보고서로 정리해줘.
```

파일은 `@file:경로`로 넣을 수 있습니다. 스킬을 막 배포했다면 목록은 **다음 메시지**부터 보이는 경우가 많습니다.

---

## 만들기 · 배포하기

직접 디스크 경로를 외울 필요는 없습니다. 채팅에서 두 스킬을 순서대로 쓰세요.

| 단계         | 호출                    | 하는 일                                       |
| ------------ | ----------------------- | --------------------------------------------- |
| 1. 작성·검증 | `@skill:skill-creator`  | 절차 초안을 만들고 형식·규칙을 검사           |
| 2. 설치      | `@skill:skill-deployer` | **누구에게 적용할지** 골라 올바른 위치에 설치 |

```
@skill:skill-creator
weekly-notes 스킬을 만들고 엄격 모드로 검증해줘.
```

```
@skill:skill-deployer
weekly-notes 를 이 프로젝트(워크스페이스)에 배포해줘.
```

- 어디에 둘지 모르겠으면 **이 프로젝트(워크스페이스)** 를 고르세요.
- **앱 기본 제공** 영역에는 배포하지 마세요. 앱이 정리할 수 있습니다.
- 어시스턴트 전용이면 deployer에 **어느 어시스턴트인지** 함께 알려 주세요.

---

## 앱에 들어 있는 스킬 (요약)

채팅 `@skill:` 자동완성이 가장 최신입니다. 자주 쓰는 이름만 모았습니다.

### 시작·환경

`setup-wizard`, `mcp-installer`, `agent-init`, `computer-diagnosis`

### 멀티 에이전트 · 오케스트레이션

하위 에이전트를 나누는 법은 **[서브 에이전트 · 오케스트레이션](sub-agents.md)** 을 보세요.

`delegate`, `teamwork`, `org`, `org-restructure`, `divide-conquer`, `hub-spoke`, `pipeline`, `consensus-delegation`, `gatekeeper`, `pair-programming`, `recruit`, `boost`

### 일정 · 웨이크

`schedule`, `loop` ([세션 루프 안내](loop.md)), `call-me-back` ([끝나면 이어서](call-me-back.md))

### 세션 · 컨텍스트

`context-recall` — 대화가 압축·요약된 뒤에도 이전 결정·내용을 다시 찾아 올 때

### 조사·문서

`deep-research`, `science-search`, `knowledge-distiller`, `to-md`, `docx`, `pptx`, `visualize`, `workspace-indexer`, `repo-wiki`, `soul-awakening`

`science-search` — 학술 논문 검색·DOI·유사 논문·인용 연결 (무료, API 키 불필요)

### 개발·연동·제작

`git-workflow`, `bench`, `fine-tune`, `email-integration`, `calendar-mgmt`, `telegram-cli`, `x-cli`, `ig-cli`, `skill-creator`, `skill-deployer`, `tool-creator`, `playbook-creator`

### 자주 하는 조합

| 하고 싶은 일        | 추천                                                 |
| ------------------- | ---------------------------------------------------- |
| 런타임 설치         | `@skill:setup-wizard` + App Wizard                   |
| MCP 추천 설치       | [Extensions](extensions.md)                          |
| MCP 커스텀/가져오기 | [커스텀 MCP](custom-mcp.md) · `@skill:mcp-installer` |
| 내 절차를 스킬로    | `skill-creator` → `skill-deployer`                   |
| 자식 세션·팀        | [서브 에이전트 가이드](sub-agents.md)                |
| 압축 전 대화 복구   | `@skill:context-recall`                              |
| 학술 논문·DOI       | `@skill:science-search`                              |
| 메일                | `@skill:email-integration`                           |
| 어시스턴트 설정     | [Assistants](assistants.md)                          |
| 반복 실행           | [Playbooks](playbooks.md) · [자동화](automation.md)  |

---

## 관련 문서

- [서브 에이전트 · 오케스트레이션](sub-agents.md)
- [Extensions (MCP)](extensions.md)
- [커스텀 MCP](custom-mcp.md)
- [5분 시작 가이드](../getting-started/5-minute-tutorial.md)
- [에이전트 첫 대화](../getting-started/first-agent.md)
- [문제 해결](troubleshooting.md)
