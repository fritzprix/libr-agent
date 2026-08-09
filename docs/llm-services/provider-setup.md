# LLM Provider 설정 가이드

> 모든 LLM 공급자의 API 키 설정과 모델 선택 방법을 통합합니다.

---

## 지원 Provider

LibrAgent는 다음 LLM 공급자를 지원합니다:

| Provider          | SDK                    | 특징                                  |
| ----------------- | ---------------------- | ------------------------------------- |
| **Anthropic**     | `anthropic`            | Claude 모델, 강력한 추론 및 도구 사용 |
| **OpenAI**        | `openai`               | GPT-4o 등, 광범위한 모델 라인업       |
| **Google Gemini** | `@anthropic-ai/gemini` | Gemini 1.5 Pro/Flash, 멀티모달        |
| **Ollama**        | OpenAI 호환            | 로컬 실행, 프라이버시 보호            |
| **Groq**          | OpenAI 호환            | 초고속 추론, Llama/Mixtral            |
| **Fireworks AI**  | OpenAI 호환            | 다양한 오픈소스 모델                  |
| **Cerebras**      | OpenAI 호환            | WaLM 기반 초고속 추론                 |
| **OpenRouter**    | OpenAI 호환            | 여러 Provider 단일 엔드포인트         |

---

## API 키 설정

### 1. Settings 열기

사이드바 **Settings** → **AI & Models** 탭

### 2. Provider API Keys

각 Provider 카드에서 **API Key**에 키를 입력합니다:

| Provider      | 발급 링크                                                            |
| ------------- | -------------------------------------------------------------------- |
| Anthropic     | [console.anthropic.com](https://console.anthropic.com/)              |
| OpenAI        | [platform.openai.com/api-keys](https://platform.openai.com/api-keys) |
| Google Gemini | [aistudio.google.com](https://aistudio.google.com/)                  |
| Groq          | [console.groq.com/keys](https://console.groq.com/keys)               |
| Fireworks AI  | [fireworks.ai](https://fireworks.ai/)                                |
| Cerebras      | [cloud.cerebras.ai](https://cloud.cerebras.ai/)                      |
| OpenRouter    | [openrouter.ai/keys](https://openrouter.ai/keys)                     |

### 3. Custom OpenAI 호환 Provider

OpenRouter, Ollama 등 OpenAI 호환 엔드포인트는 **Custom OpenAI Providers**에서 추가합니다:

| 항목      | 예시 (OpenRouter)              |
| --------- | ------------------------------ |
| 표시 이름 | OpenRouter                     |
| Base URL  | `https://openrouter.ai/api/v1` |
| API Key   | OpenRouter 키                  |

---

## 모델 설정

### Model Preferences (AI & Models 탭)

| 필드                     | 설명                                                     |
| ------------------------ | -------------------------------------------------------- |
| **Default LLM**          | 새 세션에 사용되는 기본 Provider/모델                    |
| **Fallback LLM**         | 기본 모델이 실패할 때 사용하는 대체 모델                 |
| **Override temperature** | 켜면 Temperature를 직접 지정 (끄면 Provider 기본값 사용) |

### 세션 중 모델 변경

Chat 화면에서 **Provider** / **Model** 피커를 사용해 세션 중에도 변경 가능합니다. 이 변경은 **현재 세션에만** 적용됩니다.

---

## Provider별 모델 예시

| Provider  | 모델              | 용도                  |
| --------- | ----------------- | --------------------- |
| Anthropic | Claude 3.5 Sonnet | 일반 작업, 코드 생성  |
| Anthropic | Claude 3 Opus     | 복잡한 추론, 분석     |
| OpenAI    | GPT-4o            | 다목적, 멀티모달      |
| OpenAI    | GPT-4o Mini       | 경량 작업, 빠른 응답  |
| Google    | Gemini 1.5 Pro    | 긴 컨텍스트, 멀티모달 |
| Google    | Gemini 1.5 Flash  | 빠른 응답, 비용 효율  |
| Ollama    | Llama 3.1 8B      | 로컬 실행, 프라이버시 |
| Groq      | Llama 3 70B       | 초고속 추론           |

---

## 문제 해결

| 증상                  | 확인                                                        |
| --------------------- | ----------------------------------------------------------- |
| 요청이 실패함         | Provider API Keys의 키·Base URL, **Save Changes** 여부 확인 |
| 모델 목록이 비어 있음 | 키 저장 후 모델 피커 **Refresh models**                     |
| 다른 공급자로 전환    | 해당 카드에 키를 넣고 **Default LLM** 변경                  |
| Rate limit exceeded   | 대기 후 재시도 또는 다른 모델/Provider로 전환               |

---

## 관련 문서

- [모델 연결하기](../user/getting-started/connecting-models.md)
- [문제 해결](../user/guides/troubleshooting.md)
- [개발자 설정 가이드](../guides/getting-started-dev.md)

---

_기존 `docs/llm-services/`의 10개 SDK README 파일은 [아카이브](./_archive/)로 이동되었습니다._
