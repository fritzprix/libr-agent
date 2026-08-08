---
title: Assistants
---

# Assistants

> 사이드바 **Assistants** (`/assistants`)에서 맞춤 AI 에이전트 설정을 만들고 관리합니다.  
> 세션을 시작하려면 **Chat**에서 어시스턴트 카드를 클릭합니다.

---

## 목록에서 할 수 있는 것

1. 사이드바 **Assistants** 열기  
2. **Create New Assistant**로 새 설정 만들기  
3. 카드에서 **Edit** / **Delete** (PROTECTED 표시된 빌트인은 삭제 불가)

**Chat**의 **Built-in Assistants** / **My Assistants**는 같은 설정을 세션 시작용으로 보여 줍니다.

---

## 편집 탭

**Create New Assistant** / **Edit Assistant** 화면에는 세 탭이 있습니다.

### General

| 필드 | 설명 |
|------|------|
| **Assistant Name** | 표시 이름 (필수) |
| **Description** | 짧은 설명 |
| **System Prompt** | 역할·행동 지시 (필수) |

저장: **Save**.

### Tools

- **Built-in Tools**: 코어 builtin은 항상 켜집니다. 선택(optional) builtin만 목록에서 조절합니다.
- **MCP Servers**: 이 어시스턴트가 쓸 수 있는 MCP를 고릅니다. 서버 자체 설치는 사이드바 **Extensions**에서 합니다. (Settings에 MCP 탭 없음)

### Skills

어시스턴트 scope 스킬을 붙이거나 관리합니다. 자세한 경로·우선순위는 [스킬 가이드](skills.md).

---

## 추천 흐름

| 하고 싶은 일 | 방법 |
|--------------|------|
| 바로 대화 | **Chat** → 빌트인/내 어시스턴트 카드 |
| 전문 역할 만들기 | **Assistants** → Create, 또는 `@skill:recruit` |
| 도구 정리 | `@skill:boost` 또는 Tools 탭에서 MCP/builtin 조정 |
| MCP 추가 후 허용 | [Extensions](extensions.md) → Assistants **Tools**에서 해당 서버 선택 |

---

## 관련

- [첫 대화](../getting-started/first-agent.md)
- [Extensions](extensions.md) · [커스텀 MCP](custom-mcp.md)
- [스킬](skills.md) · [서브 에이전트](sub-agents.md)
- [플레이북](playbooks.md)
