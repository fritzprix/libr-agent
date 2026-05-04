# Product Messaging Guide

이 문서는 **제품 포지셔닝 및 PR 메시지 가이드**다.  
목적은 기능을 나열하는 것이 아니라, LibrAgent를 **왜 지금 중요한 제품인지** 설득력 있게 설명할 수 있도록 돕는 것이다.

---

## 1. Core Positioning

**LibrAgent는 “좋은 모델”을 붙이는 앱이 아니라, 모델을 실제로 일하게 만드는 local-first Agent Harness이자 MCP-native Agent Operating System이다.**

짧게 말하면:

- 채팅 UI에서 끝나지 않는다.
- MCP로 외부 도구를 붙일 수 있다.
- Workspace / Browser / Knowledge / Skills를 하나의 런타임 안에서 묶는다.
- 단일 agent에서 끝나지 않고 `delegate`, `teamwork`, `org`, `schedule`을 통해 swarm과 조직 운영으로 확장된다.

---

## 2. Manifesto

### 문제 인식

AI 제품은 아직도 세 가지 함정에 자주 묶인다.

1. **좋은 모델 = 좋은 제품**이라는 착각
2. **툴은 많지만 시스템이 없다**는 문제
3. **개별 agent는 있어도 운영체제는 없다**는 문제

모델이 좋아도, 실행 하니스가 허술하면 agent는 금방 맥락을 잃고 헛돌며, 협업과 장기 실행을 버티지 못한다.

### LibrAgent의 답

LibrAgent는 이 문제를 **하니스 중심 설계**로 푼다.

- Rust 중심 orchestration
- MCP-first 구조
- Workspace / Browser / Shell / Knowledge 기반 실전 실행
- `delegate` / `teamwork` / `org` / `schedule` 기반 multi-agent coordination
- local-first 보안 감각과 운영 통제

---

## 3. Most Compelling Message

## **“LibrAgent는 채팅 앱이 아니라, Agent를 위한 실행 운영체제다.”**

이게 제일 강한 메시지다.

경쟁 제품은 대체로 아래 중 하나에 갇힌다:

- 좋은 코딩 보조 도구지만 범용 agent 운영에는 약함
- 강한 프레임워크지만 직접 조립해야 함
- 강한 클라우드 agent지만 데이터 통제권이 약함
- 자유로운 오픈소스 플랫폼이지만 운영 일관성과 거버넌스가 흔들림

LibrAgent는 여기서 드물게 동시에:

- **제품**이고,
- **하니스**고,
- **MCP 플랫폼**이고,
- **Swarm orchestration layer**다.

---

## 4. Competitive Framing

### 시장 전체 포지션

시장은 이미 모델 경쟁에서 **Agent Harness 경쟁**으로 이동했다.  
LibrAgent는 이 흐름에 맞는 제품이다.

- 모델 자체보다 **실행 루프**
- 단발 프롬프트보다 **지속 세션**
- 툴 연결보다 **도구 운영 체계**
- 단일 agent보다 **delegation / org coordination**

### 대표 경쟁군과의 차이

| 경쟁군 | 강점 | 한계 | LibrAgent의 우위 |
| --- | --- | --- | --- |
| Cursor / Claude Code 류 | 코딩 생산성 | 개발 워크플로우 중심 | 코딩을 포함하되 MCP/지식/브라우저/워크스페이스/스케줄/Swarm까지 감 |
| LangGraph / CrewAI / Pydantic AI 류 | 프레임워크 유연성 | 직접 조립 부담 | 바로 실행 가능한 제품 경험 제공 |
| OpenClaw 류 | 자유도와 확장성 | 보안/거버넌스/운영 일관성 리스크 | local-first 감각과 세션 격리, validation, artifact isolation 강화 |
| Operator / Mariner 류 | 자동화 상상력 | 클라우드/브라우저 중심 | 실제 로컬 작업 환경과 더 밀착 |

### 메시지로 뽑아낼 핵심 우위

1. **하니스 완성도가 높다**
2. **MCP를 연결 기능이 아니라 생태계 운영 계층으로 다룬다**
3. **단일 agent에서 swarm/org까지 가는 성장 경로가 자연스럽다**
4. **사용자가 자기만의 agent stack을 통제할 수 있다**

---

## 5. Onboarding Story

설득력 있는 소개는 결국 “그래서 뭘 먼저 하면 되는데?”에 답해야 한다.

### 1. 모델부터 붙인다

- Local LLM: Ollama
- API Key 기반 모델: OpenAI / Anthropic / Gemini 등

### 2. MCP를 붙인다

- preset 카탈로그에서 추가하거나
- agent에게 설치/가져오기를 맡긴다 (`mcp-installer`, `mcp-importer`)

### 3. bundled skill로 초기 구성을 가속한다

- `system-setup`
- `mcp-installer`
- `mcp-importer`
- `specialist-creator`

### 4. 도구를 기반으로 agent를 만든다

- `crew-constructor`: specialist 팀 자동 생성
- `agent-tooling`: 기존 agent 도구 최적화

### 5. delegate로 swarm을 만들고, 필요하면 org로 올린다

- 병렬 분업: `delegate`
- shared constitution: `teamwork`
- durable org identity: `org`
- recurring automation: `schedule`

---

## 6. Real Usage Stories

### Solo developer

- 로컬 repo 연결
- GitHub MCP preset 설치
- 코드 분석 / 보안 리뷰 / 문서 초안 작성

### Operator / researcher

- Browser + Search + Knowledge 연결
- 일정 기반 조사 자동화
- 결과 누적과 재활용

### Team workflow

- specialist agent 생성
- `delegate`로 병렬 작업 분산
- `org`로 더 명시적인 운영 구조 형성

### Offline / privacy-sensitive setup

- Ollama + local MCP + local workspace
- 민감 데이터가 클라우드로 나가지 않도록 통제

---

## 7. Copy-Ready Lines

### 짧은 소개

**LibrAgent는 MCP와 로컬 실행 환경을 바탕으로, 단일 AI를 실제로 일하는 agent 팀으로 확장하는 플랫폼입니다.**

### 더 강한 소개

**대부분의 AI 앱은 좋은 모델을 붙인 채팅창에서 끝납니다. LibrAgent는 다릅니다. 모델, 도구, 워크스페이스, 브라우저, 지식, 세션, 위임, 조직 운영까지 하나의 Agent Harness로 묶어, AI가 실제로 장기 작업을 수행하도록 만듭니다.**

### 한 줄 포지션

**LibrAgent의 핵심 경쟁력은 모델이 아니라 하니스에 있습니다.**

### 온보딩 유도 문장

**Ollama든 API Key든 모델 하나 붙이고, MCP 몇 개 추가한 뒤, bundled skill로 첫 agent를 깨우세요. 거기서부터는 delegate로 Swarm, org로 팀 운영까지 자연스럽게 확장됩니다.**

---

## 8. Recommended Narrative Order

PR / 발표 / 소개 글에서는 이 순서가 제일 좋다.

1. **문제 정의**: 시장은 모델보다 하니스가 중요해졌다
2. **정체성 선언**: LibrAgent는 AI 앱이 아니라 Agent Operating System이다
3. **핵심 매력**: MCP + local-first + workspace + delegation + org가 한 제품에 있다
4. **경쟁 우위**: 프레임워크도 아니고, 단순 코딩툴도 아니고, 무질서한 런타임도 아니다
5. **실사용 그림**: LLM 연결 → MCP 추가 → bundled skill → specialist 생성 → delegate → org
6. **결론**: LibrAgent는 에이전트를 “쓰는” 도구가 아니라 “운영하는” 기반이다

---

## 9. Final Take

LibrAgent를 소개할 때 “기능이 많다”로 가면 약하다.  
대신 이렇게 가야 한다:

> **LibrAgent는 하니스의 시대에 맞는 제품이다.**  
> 좋은 모델 하나를 더 붙이는 게 아니라, Agent가 실제로 일하고, 협업하고, 확장하고, 운영될 수 있도록 만드는 실행 기반이다.

그리고 가장 매력적인 지점은 이것이다:

> **로컬에서 시작할 수 있고, MCP로 확장할 수 있고, Agent 하나에서 Swarm과 Org까지 자연스럽게 커진다.**
