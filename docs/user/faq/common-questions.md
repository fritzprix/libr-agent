---
title: 자주 묻는 질문
---

# 자주 묻는 질문 (FAQ)

> UI에 없는 이름(「LLM Provider」, Settings의 「MCP Servers」탭 등)은 쓰지 않습니다.  
> 메뉴는 사이드바 라벨 기준입니다.

---

## 시작하기

### LibrAgent는 어떻게 시작하나요?

1. [Releases](https://github.com/fritzprix/libr-agent/releases)에서 앱 설치·실행
2. 사이드바 **Settings** → 탭 **AI & Models** → **Provider API Keys**에 키 저장 (**Save Changes**)
3. 사이드바 **Chat** → 어시스턴트 카드 클릭 → **New Session**에서 메시지 전송

자세한 순서: [5분 시작 가이드](../getting-started/5-minute-tutorial.md)

### 에이전트에게 어떤 일을 시킬 수 있나요?

조사·글쓰기·코드·파일·웹 브라우징·예약 작업 등입니다. 사용 가능한 도구는 어시스턴트 설정과 **Extensions**(MCP)에 따라 달라집니다.

### 여러 어시스턴트를 만들 수 있나요?

네. 사이드바 **Assistants**에서 생성·편집합니다. 시스템 프롬프트, 기본 모델, builtin/MCP 허용 등을 고를 수 있습니다.  
바로 쓰려면 **Chat**의 **Built-in Assistants**(예: Libr Assistant, App Wizard)로 세션을 시작하세요.

---

## 세션

### 세션이 사라졌어요

대화는 저장됩니다. 사이드바 **History**(또는 **Bookmarked**)에서 찾으세요. **Chat**의 최근 세션 목록도 확인합니다.

### 세션을 삭제하면?

기록과 해당 세션 컨텍스트가 제거됩니다. 하위 에이전트가 있으면 **자식 포함 삭제** vs **유지**를 고를 수 있습니다. 복구는 되지 않습니다. → [세션 가이드](../guides/sessions.md), [서브 에이전트](../guides/sub-agents.md)

### 앱을 껐다가 이어서 할 수 있나요?

네. **History** 또는 최근 세션에서 다시 열면 이어집니다.

---

## 모델 · API 키

### 어떤 모델을 쓰나요?

Anthropic, OpenAI, Google Gemini, Groq, Fireworks, Cerebras, OpenRouter, Ollama(로컬) 등.  
키·기본 모델: **Settings → AI & Models**. 세션 중에는 Chat의 Provider/Model 피커로 **현재 세션만** 바꿀 수 있습니다. → [모델 연결하기](../getting-started/connecting-models.md)

### API 키는 어디에 저장되나요?

앱 로컬 데이터에 저장됩니다. **Settings → AI & Models → Provider API Keys**에서 관리하고, 변경 후 **Save Changes**를 누르세요. 키를 이슈/채팅에 붙이지 마세요.

### 무료·로컬 모델은?

Gemini/Groq 등 제공사 무료 티어, 또는 **Ollama** 등 로컬 엔드포인트를 Custom OpenAI 호환으로 붙일 수 있습니다. 키·Base URL은 동일 Settings 탭에서 설정합니다.

---

## 도구 · Extensions · 스킬

### MCP가 무엇인가요?

에이전트가 외부 도구와 붙는 프로토콜입니다. Settings에 **「MCP Servers」탭은 없습니다.** 사이드바 **Extensions** (`/mcp-servers`)에서 관리합니다.

- 추천 설치: [Extensions 가이드](../guides/extensions.md)
- 직접 추가: [커스텀 MCP](../guides/custom-mcp.md)

### 에이전트가 내 파일을 마음대로 지우나요?

위험 작업은 보통 **승인(또는 YOLO 모드)** 정책에 따릅니다. 중요한 삭제는 확인을 요구하는 흐름이 기본입니다. 세션 기록에 도구 호출이 남습니다.

### 브라우저를 쓰나요?

어시스턴트에 브라우징 builtin이 허용된 경우 가능합니다. Assistants에서 해당 기능을 확인하세요.

### `@skill:`은 뭔가요?

재사용 절차(플레이북성 문서)입니다. 입력에 `@` → `@skill:이름`. → [스킬 가이드](../guides/skills.md), [서브 에이전트·오케스트레이션](../guides/sub-agents.md)

---

## 자동화 · 플레이북 · Org

### 예약 작업은?

사이드바 **Scheduled Tasks**. 일회성 자식 세션 위임과는 다릅니다. → [서브 에이전트 가이드](../guides/sub-agents.md)의 Org vs schedule 구분

### 플레이북은?

사이드바 **Playbooks**. 반복 작업을 템플릿으로 저장해 재사용합니다.

### Org 메뉴는?

**명시적으로 만든 Org 팀**만 보입니다. 일반 위임(sub-agent) 계보만으로는 안 뜹니다. → [서브 에이전트](../guides/sub-agents.md)

---

## 문제가 났을 때

1. 화면 메시지 확인
2. **Settings → AI & Models** 키·Default LLM·**Save Changes**
3. **Extensions**에서 MCP 명령/런타임
4. [문제 해결](../guides/troubleshooting.md) · [에러 코드](./error-codes.md)

지원 요청: [Discussions](https://github.com/fritzprix/libr-agent/discussions) — **API 키 제외**, OS·앱 버전·Provider/모델·증상 포함.

---

## 다음에 해볼 것

| 항목               | 링크                                                                        |
| ------------------ | --------------------------------------------------------------------------- |
| 첫 대화            | [first-agent](../getting-started/first-agent.md)                            |
| Assistants         | [assistants](../guides/assistants.md)                                       |
| Playbooks / 자동화 | [playbooks](../guides/playbooks.md) · [automation](../guides/automation.md) |
| MCP                | [extensions](../guides/extensions.md)                                       |
| 멀티 에이전트      | [sub-agents](../guides/sub-agents.md)                                       |
