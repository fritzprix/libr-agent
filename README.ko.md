<p align="center">
  <img src="./src-tauri/icons/128x128.png" width="96" alt="LibrAgent" />
</p>

# LibrAgent

> **직접 운영하는 에이전트 실행 환경 — 모델은 고르고, 툴은 원클릭으로, 조율 패턴은 고르기만 하면 됩니다.**
> _벤더 하네스 없음. JSON 숙제 없음. 결과는 내 머신의 파일로 남습니다._

[English](./README.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![Version](https://img.shields.io/github/v/release/fritzprix/libr-agent)](https://github.com/fritzprix/libr-agent/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/React-18.3-61DAFB?logo=react)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178C6?logo=typescript)](https://www.typescriptlang.org)

![히어로 데모 — 툴은 앱처럼 설치하고, 모델은 내가 고르고, 결과는 파일로 남깁니다.](./assets/hero-demo-60s.gif)

[HD WebM](./assets/hero-demo-60s.webm) · _툴은 앱처럼 설치하고, 모델은 내가 고르고, 결과는 파일로 남깁니다._

---

## LibrAgent가 다른 점

대부분의 에이전트 하네스는 MCP JSON을 고치고, 터미널에서 살고, 조율을 코드로 조립한다고 가정합니다(또는 한 벤더 스택에 가둡니다).

LibrAgent는 같은 일을 위한 **데스크톱 제품**입니다:

| …대신 | 이렇게 |
| ----- | ------ |
| MCP 설정을 손수 편집 | **Extensions** — 원클릭 프리셋(GitHub, Brave Search, Filesystem, …)과 Cursor / VS Code / Claude Code / Windsurf에서 import |
| “멀티에이전트 있어요” | 번들 스킬로 된 **이름 있는 조율 패턴** — `pipeline`, `hub-spoke`, `divide-conquer`, `consensus-delegation`, … |
| 한 제공자의 모델 + 툴 | **내** LLM(API 키 또는 [Ollama](https://ollama.com))과 **내** MCP 스택 — MIT, 로컬 우선 |

[최신 릴리스 다운로드](https://github.com/fritzprix/libr-agent/releases/latest) · [5분 온보딩](#5분-온보딩-경로) · [히어로 데모 스펙](docs/contributing/hero-demo-spec.md)

---

## 처음 10분 안에 할 수 있는 일

### 1. 원클릭 툴, 그리고 산출물

- **Extensions**에서 프리셋 설치(예: GitHub) — JSON 없음
- **Workspace**를 실제 폴더에 연결
- 요청: _"이 저장소에서 신규 기여자에게 가장 큰 리스크를 찾아 `DELIVERABLE.md`로 저장해"_

### 2. 원클릭 워크플로 레시피 배포

- Chat 홈 또는 [Scheduled Tasks](docs/user/guides/scheduled-tasks.md)에서 **Morning Briefing** 레시피 실행
- Hacker News + Yahoo Finance 프리셋 설치, 어시스턴트 구성, 매일 오전 9시 스케줄
- 일어나면 기술·시장 브리핑이 준비되어 있음 — 무인 실행

### 3. 조율 패턴 고르기 (프레임워크 조립 없음)

- 작업 형태를 말하거나 스킬 이름으로 붙이기:
  - _"@skill:pipeline — 조사, 초안, 리뷰 순으로; 최종 보고서 하나만"_
  - _"@skill:divide-conquer — 독립 조각으로 나눠 합쳐줘"_
- 패턴은 제품화된 스킬입니다 — 직접 배선하는 SDK가 아닙니다. [서브에이전트 & 오케스트레이션](docs/user/guides/sub-agents.md) 참고.

### 4. 모델 선택의 자유

- 클라우드: OpenAI / Anthropic / Gemini / Groq API 키
- 로컬: `ollama pull qwen3:14b` 후 Ollama 선택 — 하네스는 동일

---

## 제품의 세 가지 약속

1. **하네스 숙제 없는 표면** — GUI, Extensions 원클릭, 레시피, 인앱 승인, `@skill:` — “먼저 설정과 셸을 열라”가 아님.
2. **제품으로서의 오케스트레이션** — 스킬로 Sequential / Hub-and-spoke / Swarm 스타일 선택; 필요하면 `teamwork` → `org`, `schedule`로 확장 — LangGraph/CrewAI를 조립하지 않음.
3. **제공자·스택 자유** — 지원 LLM 아무거나, MCP 인프라, 기존 IDE MCP import, MIT, 로컬 워크스페이스·브라우저 상태 기본.

**잘 맞는 사람:** JSON 없이 하네스 깊이를 원하는 운영자·파워유저; 단일 벤더 에이전트 스택을 거부하는 개발자; 브라우저·지식·스케줄이 한 제품에 필요한 연구자.

---

## 조율 패턴 (번들 스킬)

**작업의 형태**에 맞춰 모델을 고른 뒤, 채팅에서 실행합니다:

| 스킬 | 패턴 | 이럴 때 |
| ---- | ---- | ------- |
| `pipeline` | 순차 단계 | 출력이 다음 단계 입력 (조사 → 초안 → 리뷰) |
| `hub-spoke` | 허브–스포크 | 한 코디네이터가 여러 워커를 통합 |
| `divide-conquer` | 병렬 분할 | 독립 조각 후 병합 |
| `consensus-delegation` | 다관점 | 같은 질문을 여러 전문가에게 → 조정 |
| `gatekeeper` / `pair-programming` | 리뷰 루프 | 엄격 리뷰 또는 페어 코딩 |
| `delegate` | 가벼운 핸드오프 | 자식 세션 하나, 계보 추적 |
| `teamwork` → `org` | 지속 팀 | 공유 헌장 + Org UI |
| `schedule` / `loop` / `call-me-back` | 시간·이벤트 | Cron, 세션 내 지연, 프로세스/웹훅 재개 |

선택 휴리스틱: [framework-selection](src-tauri/bundled_skills/teamwork/references/framework-selection.md) · 전체: [Sub-agents](docs/user/guides/sub-agents.md).

기타 데이원 스킬: `setup-wizard`, `tool-installer`, `playbook-creator` 등 — **[Bundled Skills](docs/user/guides/skills.md)**.

---

## MCP 플랫폼 (파워유저 깊이 유지)

- 전송: stdio, HTTP, SSE, OAuth 2.1
- 15+ 내장 서버 (Workspace, Shell, Browser, Knowledge, Planning, Scheduled Tasks, …)
- 원클릭 프리셋 + 에이전트 지원 설치 (`tool-installer`)
- 세션별 툴 격리; 경로/명령 가드; 자동화용 YOLO / unsafe 모드

### 실행 기판

| 기판 | 능력 |
| ---- | ---- |
| **Workspace** | 라인 단위 편집, 다중 파일, `@file` / `@skill` / `@playbook` |
| **Shell** | 격리·지속 셸, 비동기 프로세스 모니터링 |
| **Browser** | 격리 브라우저 사이드카; 선택적 저장 로그인 프로필 |
| **Knowledge** | 지식 그래프 + BM25 |
| **Export** | Markdown 보고서 및 ATIF 궤적 ([session export](docs/user/guides/session-export.md)) |

긴 세션은 컨텍스트 압축, 루프 방지, 회로 차단기, stale-response 가드로 유지됩니다.

---

## 실제 시나리오

### 운영자 — 빈 앱에서 매일 브리핑까지

1. **Morning Briefing** 레시피 실행 (프리셋 + 어시스턴트 + 오전 9시)
2. **Run now**로 한 번 검증
3. 터미널 없이 보고서가 쌓이도록 두기

### 솔로 개발자 — 설정 파일 대신 프리셋

1. Extensions → GitHub MCP 프리셋 설치
2. Workspace로 로컬 저장소 연결
3. 디스크에 남는 Markdown 보안/리뷰 보고서 요청

### 프레임워크 없는 파워유저 — 이름 있는 조율

1. 작업 형태에 `@skill:pipeline` (또는 `hub-spoke` / `divide-conquer`)
2. 해당 패턴으로 에이전트 조율
3. 워크스페이스에 병합된 산출물 하나 — 유지할 오케스트레이션 라이브러리 없음

### 프라이버시 팀 — 같은 제품, 로컬 모델

1. `ollama pull qwen3:14b`
2. Workspace + Shell은 머신에 유지
3. 나중에 클라우드 키로 바꿔도 — 하네스가 벤더를 강요하지 않음

---

## 문서

- **[User Guide](docs/user/README.md)** — 설치, 첫 채팅, 모델, 스킬 ([docs site](https://fritzprix.github.io/libr-agent/))
- **[Hero Demo Spec](docs/contributing/hero-demo-spec.md)** — 제품 데모 (EN/KO/ZH 자막)
- **[Product Messaging Guide](docs/contributing/product-messaging-guide.md)** — 포지셔닝과 카피
- **[Recipes](docs/user/guides/recipes.md)** · **[Scheduled Tasks](docs/user/guides/scheduled-tasks.md)** · **[Sub-agents](docs/user/guides/sub-agents.md)** · **[Skills](docs/user/guides/skills.md)**
- **[HTTP API](docs/api/http_api.md)** — 원격 제어와 프로그래밍 승인
- **[Architecture](docs/architecture/agent-workflow-architecture.md)** — 세션 격리와 Think-Act-Observe

---

## 시작하기

**[Releases 페이지](https://github.com/fritzprix/libr-agent/releases/latest)**에서 최신 설치 파일을 받으세요.

<!-- RELEASE_DOWNLOADS_START -->
- **Windows:** [`LibrAgent_0.9.22_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_x64-setup.exe) · [`LibrAgent_0.9.22_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_x64_en-US.msi)
- **macOS (Apple Silicon):** [`LibrAgent_0.9.22_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_aarch64.dmg)
- **Linux:** [`LibrAgent_0.9.22_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_amd64.AppImage) · [`LibrAgent_0.9.22_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_amd64.deb) · [`LibrAgent-0.9.22-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent-0.9.22-1.x86_64.rpm)
- **전체 릴리스 자산:** [릴리스 페이지](https://github.com/fritzprix/libr-agent/releases/tag/v0.9.22)
<!-- RELEASE_DOWNLOADS_END -->

### 5분 온보딩 경로

**1단계 — 모델 연결** (Settings → LLM Providers)

- 클라우드: OpenAI / Anthropic / Gemini / Groq API 키
- 로컬: `ollama pull qwen3:14b` 후 Settings에서 Ollama 선택

**2단계 — JSON 없이 툴 추가**

- Extensions → 프리셋 설치(예: GitHub), **또는**
- 에이전트에게: _"Cursor에서 내 MCP 서버를 import해"_

**3단계 — 워크스페이스 연결 후 남길 파일 요청**

- Workspace를 실제 프로젝트 폴더에
- _"이 워크스페이스를 리뷰하고 결과를 `DELIVERABLE.md`에 남겨."_

**다음 — 조율과 자동화**

- _"@skill:pipeline — 조사, 초안, 리뷰; 최종 보고서 하나."_
- _"이 저장소용 teamwork 워크스페이스를 준비해."_
- _"매일 오전 7시 경쟁사 브리프 스케줄을 만들어."_ (또는 Morning Briefing 레시피)

### 복사해 쓸 첫 프롬프트

- _"Cursor에서 내 MCP 서버를 import하고 뭐가 추가됐는지 보여줘."_
- _"GitHub MCP 프리셋을 설치하고 코딩 에이전트에 붙여."_
- _"이 워크스페이스를 리뷰한 뒤 `DELIVERABLE.md`에 남겨."_
- _"@skill:pipeline — 이 주제를 조사하고 요약을 초안한 뒤 리뷰; 최종 보고서를 저장."_
- _"매일 오전 7시 경쟁사 브리프 스케줄을 만들어."_

### 개발자 설정

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## LibrAgent가 잘 맞는 경우

| 원하는 것 | LibrAgent가 강한 이유 |
| --------- | --------------------- |
| **하네스 숙제 없는 깊이** | Extensions 프리셋, 레시피, `@skill:` 패턴, 승인 — JSON 우선 온보딩이 아님 |
| **프레임워크 조립 없는 조율** | `pipeline`, `hub-spoke`, `divide-conquer`, `consensus-delegation`, `teamwork` / `org`, `schedule`가 제품으로 제공 |
| **벤더 에이전트 스택으로부터의 자유** | 모델·MCP는 사용자가; MIT; 기본 로컬 우선 |
| **실제 실행 기판** | Workspace, shell, browser, knowledge, playbook, 장기 세션 가드 |
| **MCP 네이티브 데스크톱 제품** | 프리셋, import, 15+ 내장 — 얇은 채팅 래퍼가 아님 |

---

## 설계 철학

- **키트보다 제품**: 조립하지 않아도 하네스를 쓸 수 있다.
- **스킬로서의 오케스트레이션**: 조율 패턴은 이름 있고 선택·문서화된다 — 샘플 레포에만 있지 않다.
- **스택의 자유**: 모델과 툴은 사용자 선택; 특정 AI 벤더를 요구하지 않는다.
- **로컬 우선**: 워크스페이스·세션·스킬·브라우저 상태는 사용자 통제. 클라우드 LLM / 원격 MCP는 선택 시에만.
- **모델보다 하네스**: 툴, 세션 상태, 위임, 거버넌스가 단일 모델보다 중요하다.
- **기능보다 안정**: 격리, 압축, 루프 방지 — 기능 추격 전에.
- **열린 표준**: MIT. 상호운용 층으로서의 MCP.

---

## 기여 & 라이선스

LibrAgent는 MIT이며 공개적으로 만들어집니다. 번들 스킬, MCP 통합, 버그 수정, 아키텍처 개선을 환영합니다.

- [Contributing Guide](CONTRIBUTING.md)
- [Issue Tracker](https://github.com/fritzprix/libr-agent/issues) [![Good First Issues](https://img.shields.io/github/issues/fritzprix/libr-agent/good%20first%20issue)](https://github.com/fritzprix/libr-agent/issues?q=is%3Aissue+is%3Aopen+label%3A%22good%20first%20issue%22)
- [Discussions](https://github.com/fritzprix/libr-agent/discussions)
- 벤치마크 (Harbor / Terminal-Bench): [Harbor guide](benchmarks/harbor/README.md) (`pnpm bench:diverse`, `pnpm bench:terminal`, …)

**License**: MIT
