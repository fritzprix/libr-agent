---
title: 커스텀 MCP 설치
---

# 커스텀 MCP 설치

> 추천 목록에 없는 MCP를 **Add Extension**으로 직접 등록하거나, 에이전트에게 `@skill:tool-installer`로 맡기는 방법입니다.  
> 추천 프리셋만 쓰려면 [Extensions로 MCP 설치하기](extensions.md)를 보세요.

---

## 언제 커스텀이 필요한가

- npm/`npx` 패키지, GitHub의 MCP 서버
- 자체 `node`/`python`/`uvx` 명령
- 원격 HTTP/SSE MCP 엔드포인트
- Cursor / VS Code / Windsurf 등 다른 앱에 있던 MCP를 가져올 때

---

## 방법 A — UI: Add Extension

1. 사이드바 **Extensions** → **Tools**
2. **Add Extension** (`확장 기능 추가`)
3. 필드 입력 후 **Save**

### 공통 필드

- **Name** — 고유 id (예: `filesystem`, `my-search`). builtin 예약 이름은 사용 불가
- **Description** — 선택
- **Transport Type**
  - **stdio (Local Process)** — 로컬에서 프로세스 실행
  - **HTTP (Remote Server)** — URL로 접속

### stdio (로컬)

| 필드 | 의미 | 예 |
|------|------|----|
| **Command** | 실행 파일 | `npx`, `uvx`, `node`, `python` |
| **Arguments** | 공백 구분 인자 | `-y @modelcontextprotocol/server-filesystem /tmp` |
| **Environment Variables** | 프로세스에 넘길 키/값 | API 키 등 |

예시 — filesystem:

- Name: `filesystem`
- Transport: stdio
- Command: `npx`
- Arguments: `-y @modelcontextprotocol/server-filesystem /path/to/folder`

`npx`/`uv`가 없으면 먼저 `@skill:setup-wizard` 또는 **App Wizard**로 런타임을 설치하세요.

### HTTP (원격)

| 필드 | 의미 |
|------|------|
| **URL** | MCP 엔드포인트 전체 URL |
| **API Key / Token** (선택) | 있으면 `Authorization: Bearer …` 자동 추가 |
| **Custom Headers** (고급) | 추가 헤더 |
| **Enable SSE** | 스트리밍용. 상태 없는 HTTP면 끌 수 있음 |

### 저장 후

1. 목록에서 **Active** 확인  
2. 필요하면 Assistants에서 그 확장 허용  
3. 새 세션/다음 턴에서 도구 사용

편집·삭제는 카드의 **Edit** / **Delete**입니다.

---

## 방법 B — 에이전트: `@skill:tool-installer`

채팅에서:

```
@skill:tool-installer
npx로 @modelcontextprotocol/server-everything 를 LibrAgent에 등록해줘.
```

```
@skill:tool-installer
Cursor에 있는 MCP 설정을 읽어서 LibrAgent에 가져와줘.
```

이 스킬이 하는 일:

- 패키지·GitHub·JSON 설정으로 **직접 등록** (`tool__registerServer` 등)
- Cursor / VS Code / Windsurf / Claude Desktop 등 **설정 파일에서 가져와 비교·등록**

자세한 절차는 번들 스킬 `tool-installer` 본문을 따릅니다. ([스킬 카탈로그](skills.md))

---

## 어시스턴트에 연결하기

확장을 설치만 하고 어시스턴트에서 막혀 있으면 도구가 안 보입니다.

1. **Manage Assistants** (또는 Assistants)에서 프로필 편집  
2. MCP / 서버 선택 UI에서 해당 확장을 허용  
3. 그 어시스턴트로 세션을 다시 시작

(정확한 탭 라벨은 UI의 “Select which MCP servers this assistant can access” 계열입니다.)

---

## 문제 해결

| 증상 | 확인 |
|------|------|
| 저장은 됐는데 도구 없음 | Active 여부, 어시스턴트 MCP 허용, 세션 재시작 |
| `command not found` / npx 실패 | App Wizard / `@skill:setup-wizard` |
| 기동 시간 초과 | **Settings → Advanced → MCP Discovery Timeout** 증가 |
| 이름 저장 거부 | builtin 예약 이름과 충돌 — 다른 Name 사용 |
| API 키 필요 프리셋 | Extensions 폼의 env / Required Configuration |

---

## 관련 문서

- [Extensions로 MCP 설치하기](extensions.md) — 추천 프리셋 목록
- [5분 시작 가이드](../getting-started/5-minute-tutorial.md) — App Wizard
- [문제 해결](troubleshooting.md)
