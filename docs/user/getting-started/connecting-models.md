---
title: 모델 연결하기
---

# 모델 연결하기

> 사이드바 **Settings → AI & Models**에서 API 키와 기본 모델을 설정합니다.

---

## 이 가이드에서 배우는 것

1. Settings 페이지 여는 방법
2. **Provider API Keys**에 키 넣기
3. **Default LLM** / **Fallback LLM** 고르기
4. 커스텀 OpenAI 호환 Provider 추가
5. 연결이 안 될 때 확인할 것

---

## 1. Settings 열기

왼쪽 **사이드바 → Settings**를 누릅니다. 전체 페이지(`/settings`)로 열리며, 저장할 때까지 열려 있습니다.

상단 액션: **Discard** · **Close** · **Save Changes**

### 탭 이름 (실제 UI)

| 탭                 | 용도                                |
| ------------------ | ----------------------------------- |
| **General**        | 언어, 스킬 디렉터리 등              |
| **AI & Models**    | API 키, 기본/대체 모델, Temperature |
| **Chat Interface** | 채팅 UI·컨텍스트 관련               |
| **System**         | 시스템 옵션                         |
| **Advanced**       | 고급 (셸 런타임 PATH 등)            |
| **Experimental**   | 실험 기능                           |

> **없는 이름**: 「LLM Provider」섹션, 「AI Models」(단독), 「Preferred Model」, Settings를 여는 “우측 상단 톱니” — 현재 UI와 맞지 않습니다.

![Settings → AI & Models](../assets/screenshots/getting-started/settings-ai-models.png)

---

## 2. Provider API Keys

1. **AI & Models** 탭을 엽니다.
2. **Provider API Keys** 섹션에서 공급자 카드를 고릅니다.  
   예: OpenAI, Anthropic, Google Gemini, Ollama, Groq, Fireworks AI, Cerebras, OpenRouter
3. **API Key**에 키를 붙여넣습니다. (일부는 **Base URL**도 있습니다.)
4. **Save Changes**를 누릅니다.

키는 앱 로컬에 저장됩니다. 채팅에 키를 붙여 넣지 마세요.

### 키 발급

| Provider      | 발급                                                                 |
| ------------- | -------------------------------------------------------------------- |
| Anthropic     | [console.anthropic.com](https://console.anthropic.com/)              |
| OpenAI        | [platform.openai.com/api-keys](https://platform.openai.com/api-keys) |
| Google Gemini | [aistudio.google.com](https://aistudio.google.com/)                  |
| Groq          | [console.groq.com/keys](https://console.groq.com/keys)               |

---

## 3. Model Preferences

같은 **AI & Models** 탭의 **Model Preferences**:

| 필드                     | 의미                                                    |
| ------------------------ | ------------------------------------------------------- |
| **Default LLM**          | 새 세션에 쓰일 기본 Provider/모델                       |
| **Fallback LLM**         | 기본 모델이 실패할 때 쓰는 대체                         |
| **Override temperature** | 켜면 **Temperature**를 직접 지정 (끄면 Provider 기본값) |

세션 중에도 Chat 화면의 **Provider** / **Model** 피커로 바꿀 수 있습니다. 그 변경은 **현재 세션**에만 적용됩니다.

---

## 4. Custom OpenAI Providers

OpenRouter·로컬(Ollama 등) OpenAI 호환 엔드포인트는 **Custom OpenAI Providers**에서 **Add Custom OpenAI Provider**로 추가합니다.

예시 (OpenRouter):

| 항목      | 예                             |
| --------- | ------------------------------ |
| 표시 이름 | OpenRouter                     |
| Base URL  | `https://openrouter.ai/api/v1` |
| API Key   | OpenRouter 키                  |

예시 (Ollama):

| 항목     | 예                          |
| -------- | --------------------------- |
| Base URL | `http://localhost:11434/v1` |
| API Key  | (비우거나 `ollama`)         |

로컬 서버는 미리 실행되어 있어야 합니다.

---

## 5. 문제 해결

| 증상                  | 확인                                                       |
| --------------------- | ---------------------------------------------------------- |
| 요청이 실패함         | **Provider API Keys**의 키·Base URL, **Save Changes** 여부 |
| 모델 목록이 비어 있음 | 키 저장 후 모델 피커 **Refresh models**                    |
| 다른 공급자로 전환    | 해당 카드에 키를 넣고 **Default LLM** 변경                 |
| Python/Node 등 런타임 | Settings가 아니라 **Chat → App Wizard** + **setup-wizard** |

연결 상태 뱃지(「연결됨 / 검증 중」) UI는 Settings에 없습니다. 저장 후 Chat에서 실제로 메시지를 보내 확인하세요.

자세한 증상별 가이드: [문제 해결](../guides/troubleshooting.md)

---

## 다음 단계

- [5분 시작 가이드](5-minute-tutorial.md) — App Wizard / setup-wizard 소개 포함
- [에이전트 첫 대화](first-agent.md)

---

_사용자용 가이드입니다. 개발자용 설정은 [getting-started.md](https://github.com/fritzprix/libr-agent/blob/main/docs/guides/getting-started.md)를 참고하세요._
