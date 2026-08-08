---
title: 문제 해결
---

# 문제 해결

> 증상 → 원인 → **실제 UI 이름** 기준 해결 방법.

---

## 1. API 키·모델

### 채팅이 안 되거나 요청이 바로 실패해요

**원인**: API 키가 없거나 잘못됨.

**해결**:

1. 사이드바 **Settings** → **AI & Models**
2. **Provider API Keys**에서 해당 공급자 카드의 **API Key** 확인
3. **Save Changes**
4. **Chat**에서 다시 메시지 전송

키 발급: [Anthropic](https://console.anthropic.com/) · [OpenAI](https://platform.openai.com/api-keys) · [Gemini](https://aistudio.google.com/) · [Groq](https://console.groq.com/keys)

> Settings에 「연결됨 / 검증 중」 상태 표시는 없습니다. 저장 후 실제 요청으로 확인하세요.

### `Invalid API key` / Authentication failed

1. **Settings → AI & Models → Provider API Keys**
2. 앞뒤 공백·잘린 키 확인 후 재입력
3. Provider 콘솔에서 새 키 발급
4. **Save Changes**

### `Rate limit exceeded`

잠시 후 재시도하거나, **Default LLM** / 세션의 **Model**을 다른 모델·공급자로 바꿉니다.

### 응답이 느리거나 비용이 커요

| 시도 | 위치 |
|------|------|
| 더 작은/빠른 모델 | Chat의 **Model** 또는 Settings **Default LLM** |
| 컨텍스트 줄이기 | **Settings → Chat Interface → Max Input Context** |
| 로컬 모델 | Custom OpenAI Provider로 Ollama 등 |

### 응답이 너무 무작위예요

**Settings → AI & Models → Model Preferences**에서 **Override temperature**를 켜고 **Temperature**를 낮춥니다 (예: 0.2–0.5).

---

## 2. 세션

### 세션을 못 찾겠어요

1. 앱 재시작
2. **History**(있는 경우)에서 검색
3. 삭제한 세션은 복구 불가

### 새 세션이 안 열려요

1. **Settings → AI & Models**에서 키·**Default LLM** 확인 후 **Save Changes**
2. 사이드바 **Chat** → **Built-in Assistants**에서 어시스턴트 카드 클릭
3. 초안(**New Session**)에서 메시지 전송

> **「+ New Session」** 버튼 흐름이 아닙니다.

### 세션이 오래 걸려요

에이전트가 도구·하위 작업을 수행 중일 수 있습니다. UI의 진행/일시정지 상태를 보고, 필요하면 세션을 중단한 뒤 더 작은 모델로 다시 시도하세요.

---

## 3. 도구·환경·MCP

### Python / Node / uv가 없다고 해요

Settings 탭이 아니라:

1. **Chat → Built-in Assistants → App Wizard**
2. 환경 점검을 요청합니다. App Wizard는 **setup-wizard**(별칭 **bootstrap**)로 플랫폼 감지·설치 안내를 합니다.

### MCP / 외부 도구가 안 붙어요

Settings에 **「MCP Servers」** 탭은 **없습니다.** 사이드바 **Extensions** (`/mcp-servers`)를 쓰세요.

1. **추천**: [Extensions → Recommended Extensions](extensions.md)
2. **직접/다른 에디터에서 가져오기**: [커스텀 MCP 설치](custom-mcp.md) 또는 `@skill:tool-installer`
3. Assistants에서 해당 MCP 허용 여부 확인
4. OS에서 `npx`/`uv` 등 command가 실행 가능한지 확인 (App Wizard)

### 에이전트가 도구를 안 써요

프롬프트에 도구 사용을 명시하거나, 어시스턴트에 필요한 builtin/MCP가 허용돼 있는지 확인합니다. 응답의 도구 배지로 호출 여부를 볼 수 있습니다.

### 파일/워크스페이스 도구 실패

세션 워크스페이스 경로·OS 권한을 확인합니다. **Settings → System** / **General**(스킬 디렉터리 등)도 점검합니다.

### 하위 에이전트가 파일을 못 찾아요 / Org에 안 보여요

- 자식은 부모 워크스페이스·로컬 스킬을 **자동 상속하지 않습니다.** [서브 에이전트 가이드](sub-agents.md)의 격리 규칙을 보세요. `@skill:delegate`로 핸드오프를 다시 적거나 공유 워크스페이스를 쓰세요.
- 사이드바 **Org**에는 **명시적 Org**만 나옵니다. 일반 위임 세션은 세션 히스토리의 하위 에이전트 표시를 확인하세요.

---

## 4. 앱

### 앱이 멈췄어요

1. 완전 종료 후 재실행
2. 지속 시 [GitHub Discussions](https://github.com/fritzprix/libr-agent/discussions)에 OS·버전·증상 공유 (**API 키 제외**)

개발 빌드에만 **Dev** 탭이 있을 수 있습니다. 일반 사용자에게는 필수가 아닙니다.

### UI가 깨져요

재시작 후 시도. 그래픽/디스플레이 이슈면 OS·드라이버를 점검합니다.

---

## 5. 자주 보는 메시지

| 메시지 | 조치 |
|--------|------|
| Invalid API key | **Provider API Keys** 재입력 |
| Rate limit exceeded | 대기 또는 모델 변경 |
| Model not found | **Default LLM** / 세션 **Model** 변경, **Refresh models** |
| Connection refused / Timeout | 네트워크·로컬 서버(Ollama)·MCP 프로세스 |
| Permission denied | 폴더/OS 권한 |
| Session not found | 재시작, History 확인 |

---

## 도움 요청 시

[Discussions](https://github.com/fritzprix/libr-agent/discussions)에 OS, LibrAgent 버전, Provider/모델 이름, 증상, 시도한 것을 적되 **API 키는 넣지 마세요.**

---

## 관련 문서

- [모델 연결하기](../getting-started/connecting-models.md)
- [5분 시작 가이드](../getting-started/5-minute-tutorial.md) — App Wizard / setup-wizard
- [에이전트 첫 대화](../getting-started/first-agent.md)
- [번들 스킬](skills.md)
- [서브 에이전트 · 오케스트레이션](sub-agents.md)
- [Extensions (MCP)](extensions.md)
- [커스텀 MCP](custom-mcp.md)

---

*사용자용입니다. 개발자 디버깅은 [getting-started.md](https://github.com/fritzprix/libr-agent/blob/main/docs/guides/getting-started.md)를 참고하세요.*
