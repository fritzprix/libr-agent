<p align="center">
  <img src="./src-tauri/icons/128x128.png" width="96" alt="LibrAgent" />
</p>

# LibrAgent

> **사용자가 직접 제어하는 로컬 AI 에이전트 데스크톱 앱입니다.**
> 원하는 LLM을 연결하고, 클릭 한 번으로 도구를 설치하며, 검증된 다중 에이전트 협업 패턴으로 작업을 자동화합니다.
> _특정 벤더 종속이나 복잡한 JSON 설정 없이, 모든 작업 결과물을 내 PC에 로컬 파일로 안전하게 저장합니다._

[English](./README.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![Version](https://img.shields.io/github/v/release/fritzprix/libr-agent)](https://github.com/fritzprix/libr-agent/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/React-18.3-61DAFB?logo=react)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178C6?logo=typescript)](https://www.typescriptlang.org)

![히어로 데모 — 앱처럼 도구를 설치하고, 원하는 모델을 선택하며, 결과물을 파일로 보관합니다.](./assets/hero-demo-60s.gif)

[HD WebM 영상 보기](./assets/hero-demo-60s.webm) · _앱처럼 도구를 설치하고, 원하는 모델을 선택하며, 결과물을 파일로 보관합니다._

---

## LibrAgent가 특별한 이유

대부분의 AI 에이전트 프레임워크는 사용자가 터미널 환경에 익숙하고, 복잡한 JSON 설정을 직접 다루며, 코드로 오케스트레이션을 직접 구현해야 한다고 전제합니다. 또는 특정 클라우드 벤더의 서비스에 강하게 종속됩니다.

LibrAgent는 이러한 작업을 간소화한 **완성형 데스크톱 제품**입니다:

| 기존 방식 | LibrAgent 방식 |
| --------- | -------------- |
| 수동으로 복잡한 MCP JSON 설정 작성 | **원클릭 확장 도구** — GitHub, Brave Search, 파일시스템 등의 프리셋 지원 및 Cursor / VS Code / Claude Code 설정 자동 가져오기 |
| 코드로 직접 구현해야 하는 멀티 에이전트 | **검증된 협업 패턴 내장** — `pipeline`, `hub-spoke`, `divide-conquer`, `consensus-delegation` 등 기본 제공 스킬 |
| 특정 업체의 모델 및 도구에 종속 | **자유로운 스택 선택** — 상용 LLM API부터 로컬 [Ollama](https://ollama.com)까지 자유롭게 연동하는 MIT 라이선스 로컬 우선 앱 |

[최신 릴리스 다운로드](https://github.com/fritzprix/libr-agent/releases/latest) · [5분 온보딩 가이드](#5분-온보딩-경로) · [데모 사양서](docs/contributing/hero-demo-spec.md)

---

## 10분 만에 경험하는 핵심 기능

### 1. 원클릭 도구 설치와 파일 생성

- **Extensions** 메뉴에서 GitHub 등의 도구 프리셋을 원클릭으로 설치합니다 (JSON 설정 불필요).
- **Workspace**에서 작업할 로컬 프로젝트 폴더를 지정합니다.
- 다음과 같이 요청합니다: _"이 저장소를 분석하여 신규 기여자가 주의해야 할 핵심 위험 요소를 `DELIVERABLE.md` 파일로 정리해줘"_

### 2. 자동화 워크플로 레시피 실행

- 채팅 홈 또는 [스케줄 작업 가이드](docs/user/guides/scheduled-tasks.md)에서 **Morning Briefing** 레시피를 실행합니다.
- Hacker News 및 Yahoo Finance 도구가 자동으로 연결되며, 매일 오전 9시에 실행되도록 예약됩니다.
- 출근길에 밤사이 취합된 IT 기술 및 시장 동향 요약 보고서를 확인할 수 있습니다.

### 3. 코드 없이 선택하는 에이전트 협업 패턴

- 복잡한 프레임워크 코드를 작성할 필요 없이, 원하는 협업 방식을 대화로 지정합니다:
  - _"@skill:pipeline — 먼저 자료를 조사하고, 초안을 작성한 뒤 검토해서 최종 보고서 한 편으로 정리해줘"_
  - _"@skill:divide-conquer — 이 작업을 독립된 하위 작업으로 나누어 병렬로 진행하고 결과를 종합해줘"_
- 검증된 협업 패턴이 스킬 형태로 내장되어 있습니다. 자세한 내용은 [서브에이전트 및 오케스트레이션 가이드](docs/user/guides/sub-agents.md)를 참고하십시오.

### 4. 자유로운 모델 선택

- **클라우드 모델**: OpenAI, Anthropic, Gemini, Groq 등의 API 키를 입력하여 즉시 사용합니다.
- **로컬 모델**: `ollama pull qwen3:14b` 실행 후 설정에서 Ollama를 선택하면 외부 통신 없이 완전한 로컬 환경으로 구동됩니다.

---

## 제품의 세 가지 핵심 가치

1. **설정 과정 없는 완성형 UI** — 직관적인 GUI, 원클릭 도구 설치, 자동화 레시피, 인앱 실행 승인 시스템을 통해 터미널이나 설정 파일 없이 즉시 사용 가능합니다.
2. **별도 프레임워크가 필요 없는 오케스트레이션** — 순차 처리(Sequential), 허브 앤 스포크(Hub-and-spoke), 스웜(Swarm) 패턴을 스킬로 선택하고, 장기 프로젝트를 위한 `teamwork` 및 정기 실행 `schedule`로 손쉽게 확장합니다.
3. **완전한 모델 및 데이터 독립성** — 오픈 표준인 MCP(Model Context Protocol)를 기반으로 동작하며, 모든 작업 공간과 브라우저 세션 데이터는 기본적으로 사용자의 로컬 환경에만 보관됩니다.

**추천 대상:** 복잡한 설정 없이 강력한 에이전트를 데스크톱에서 사용하려는 실무자, 단일 벤더 종속을 피하려는 개발자, 브라우저 자동화와 지식 베이스 및 예약 실행 기능이 모두 필요한 연구자.

---

## 내장 협업 패턴 (번들 스킬)

작업의 특성에 맞는 협업 패턴을 선택하여 채팅창에서 바로 호출할 수 있습니다:

| 스킬명 | 협업 패턴 | 권장 작업 유형 |
| ------ | --------- | -------------- |
| `pipeline` | 순차 단계별 처리 | 이전 단계의 결과물을 다음 단계로 전달 (조사 → 초안 작성 → 최종 검토) |
| `hub-spoke` | 허브 앤 스포크 | 하나의 조율자 에이전트가 여러 전문 작업 에이전트를 총괄 관리 |
| `divide-conquer` | 병렬 분할 처리 | 독립적인 하위 작업들로 쪼개어 병렬 수행 후 결과 종합 |
| `consensus-delegation` | 다관점 비교 분석 | 동일한 질문을 여러 전문 에이전트에 질의한 뒤 의견 수렴 및 조율 |
| `gatekeeper` / `pair-programming` | 코드 검토 루프 | 엄격한 품질 검토 또는 두 에이전트 간의 페어 프로그래밍 |
| `delegate` | 단순 작업 위임 | 독립된 하위 세션에 특정 작업을 위임하고 진행 상태 추적 |
| `teamwork` → `org` | 영구 프로젝트 팀 | 공유 헌장과 조직도(Org UI)를 기반으로 협력하는 팀 구성 |
| `schedule` / `loop` / `call-me-back` | 시간 및 이벤트 기반 | 정기 크론(Cron) 예약, 세션 내 지연 실행, 외부 이벤트 기반 재개 |

상세 가이드: [패턴 선택 가이드](src-tauri/bundled_skills/teamwork/references/framework-selection.md) · [서브에이전트 가이드](docs/user/guides/sub-agents.md)

기타 기본 제공 스킬: `setup-wizard`, `tool-installer`, `playbook-creator` 등 — **[번들 스킬 목록](docs/user/guides/skills.md)**.

---

## 강력한 MCP 실행 인프라

- **다양한 전송 프로토콜**: stdio, HTTP, SSE, OAuth 2.1 지원
- **15개 이상의 내장 서버**: Workspace, Shell, Browser, Knowledge, Planning, Scheduled Tasks 등 완비
- **원클릭 프리셋 및 자동 설치 지원**: `tool-installer`를 통한 대화형 도구 설치
- **안전한 격리 환경**: 세션별 도구 격리, 파일 경로 및 셸 명령어 실행 제한 가드, 자동화 전용 안전 실행 모드 지원

### 핵심 실행 인프라

| 실행 영역 | 세부 기능 |
| --------- | --------- |
| **Workspace** | 라인 단위 정밀 코드 편집, 다중 파일 일괄 작업, `@file` / `@skill` / `@playbook` 컨텍스트 주입 |
| **Shell** | 격리된 영구 셸 환경 및 백그라운드 프로세스 비동기 모니터링 |
| **Browser** | 격리된 브라우저 사이드카 및 일상 Chrome 세션을 연동하는 브리지 지원 |
| **Knowledge** | 지식 그래프 저장소 및 고속 BM25 하이브리드 검색 |
| **Export** | Markdown 보고서 출력 및 표준 ATIF 세션 궤적 내보내기 지원 ([세션 내보내기](docs/user/guides/session-export.md)) |

컨텍스트 자동 압축, 무한 루프 감지기, 서킷 브레이커, 응답 유효성 검증 가드를 내장하여 장시간 실행되는 세션도 성능 저하 없이 안정적으로 유지됩니다.

---

## 실제 활용 시나리오

### 업무 자동화 담당자 — 아침 브리핑 자동 생성

1. **Morning Briefing** 레시피를 실행합니다 (도구 설치, 에이전트 구성, 매일 09:00 실행 예약).
2. **Run now**를 클릭하여 정상 작동 여부를 확인합니다.
3. 터미널을 열지 않고도 매일 아침 자동으로 생성된 시장 브리핑 보고서를 확인합니다.

### 1인 개발자 — 설정 파일 없는 신속한 도구 연동

1. Extensions 메뉴에서 GitHub MCP 프리셋을 설치합니다.
2. Workspace에서 로컬 코드 저장소를 연결합니다.
3. 에이전트에게 보안 및 코드 검토 보고서를 작성하여 로컬 디스크에 저장하도록 요청합니다.

### 비개발자 파워유저 — 협업 템플릿 기반 작업 위임

1. 대화창에 `@skill:pipeline` (또는 `hub-spoke`, `divide-conquer`)을 입력하여 작업 구조를 지정합니다.
2. 에이전트들이 해당 패턴에 맞춰 협력하여 작업을 수행합니다.
3. 외부 라이브러리 유지보수 없이 하나의 통합된 최종 결과물을 워크스페이스에 생성합니다.

### 보안에 민감한 팀 — 외부 유출 없는 100% 로컬 구동

1. `ollama pull qwen3:14b`를 실행합니다.
2. Workspace와 Shell의 모든 작업 데이터가 로컬 PC 내에서만 처리됩니다.
3. 필요 시 언제든지 클라우드 모델로 전환할 수 있으며, 전체 사용 워크플로는 동일하게 유지됩니다.

---

## 공식 문서 안내

- **[사용자 가이드 (User Guide)](docs/user/README.md)** — 설치, 첫 대화, 모델 설정, 스킬 활용법 ([웹사이트 보기](https://fritzprix.github.io/libr-agent/))
- **[데모 영상 사양서 (Hero Demo Spec)](docs/contributing/hero-demo-spec.md)** — 공식 제품 데모 사양 (한국어/영어/중국어 자막)
- **[제품 메시징 가이드](docs/contributing/product-messaging-guide.md)** — 제품 포지셔닝 및 설명 가이드라인
- **[레시피 가이드](docs/user/guides/recipes.md)** · **[스케줄 작업](docs/user/guides/scheduled-tasks.md)** · **[서브에이전트](docs/user/guides/sub-agents.md)** · **[스킬 가이드](docs/user/guides/skills.md)**
- **[HTTP API 명세](docs/api/http_api.md)** — 원격 제어 및 프로그램 방식의 승인 인터페이스
- **[아키텍처 문서](docs/architecture/agent-workflow-architecture.md)** — 세션 격리 및 Think-Act-Observe 실행 루프

---

## 시작하기

**[Releases 페이지](https://github.com/fritzprix/libr-agent/releases/latest)**에서 최신 설치 파일을 다운로드하십시오.

<!-- RELEASE_DOWNLOADS_START -->
- **Windows:** [`LibrAgent_0.9.22_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_x64-setup.exe) · [`LibrAgent_0.9.22_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_x64_en-US.msi)
- **macOS (Apple Silicon):** [`LibrAgent_0.9.22_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_aarch64.dmg)
- **Linux:** [`LibrAgent_0.9.22_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_amd64.AppImage) · [`LibrAgent_0.9.22_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent_0.9.22_amd64.deb) · [`LibrAgent-0.9.22-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.22/LibrAgent-0.9.22-1.x86_64.rpm)
- **전체 릴리스 목록:** [GitHub Releases](https://github.com/fritzprix/libr-agent/releases/tag/v0.9.22)
<!-- RELEASE_DOWNLOADS_END -->

### 5분 온보딩 경로

**1단계 — 모델 연결** (Settings → LLM Providers)

- 클라우드 모델: OpenAI, Anthropic, Gemini, Groq 등의 API 키를 입력합니다.
- 로컬 모델: `ollama pull qwen3:14b` 실행 후 설정에서 Ollama를 선택합니다.

**2단계 — 도구 연결 (JSON 편집 불필요)**

- Extensions 메뉴에서 GitHub 등의 프리셋을 설치하거나,
- 에이전트에게 다음과 같이 요청합니다: _"Cursor에서 사용하는 내 MCP 서버 설정을 가져와줘"_

**3단계 — 워크스페이스 연결 및 결과물 생성**

- Workspace에서 작업할 로컬 프로젝트 폴더를 지정합니다.
- 다음과 같이 요청합니다: _"이 워크스페이스를 분석하고 핵심 내용을 `DELIVERABLE.md` 파일로 정리해줘"_

**다음 단계 — 협업 패턴 및 자동화 확장**

- _"@skill:pipeline — 자료 조사, 초안 작성, 검토 순으로 진행하여 최종 보고서 1편으로 저장해줘"_
- _"이 저장소를 분석하기 위한 teamwork 워크스페이스를 구성해줘"_
- _"매일 오전 7시에 경쟁사 동향을 요약하는 스케줄 작업을 만들어줘"_ (또는 Morning Briefing 레시피 실행)

### 바로 복사해서 사용하는 실전 프롬프트

- _"Cursor에서 사용하는 내 MCP 서버 설정을 가져와서 어떤 도구가 추가되었는지 보여줘."_
- _"GitHub MCP 프리셋을 설치하고 코딩 에이전트에 연결해줘."_
- _"이 워크스페이스를 검토하고 발견된 개선점을 `DELIVERABLE.md`에 작성해줘."_
- _"@skill:pipeline — 이 주제를 심층 조사하고, 요약 초안을 작성한 뒤 검토해서 최종 보고서로 저장해줘."_
- _"매일 오전 7시에 시장 동향 브리핑을 실행하도록 예약해줘."_

### 개발자 실행 환경 구성

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## 설계 철학

- **조립 키트가 아닌 완성형 제품**: 복잡한 환경 설정 없이 설치 즉시 업무에 활용할 수 있습니다.
- **스킬로 정의되는 오케스트레이션**: 다중 에이전트 협업 패턴이 명확한 이름과 사양을 가진 스킬로 체계화되어 있습니다.
- **자유로운 기술 스택**: 모델과 도구 선택권은 사용자에게 있으며, 특정 AI 업체의 종속을 강제하지 않습니다.
- **로컬 우선 원칙 (Local First)**: 모든 작업 공간, 세션 기록, 브라우저 상태는 사용자의 로컬 PC에 안전하게 보관됩니다.
- **모델보다 중요한 실행 환경**: 훌륭한 도구 연동, 세션 상태 관리, 정밀한 권한 제어가 단일 모델의 성능보다 더 안정적인 결과를 만듭니다.
- **기능 추가보다 시스템 안정성 우선**: 컨텍스트 격리, 자동 압축, 루프 방지 등 시스템 안정성을 최우선으로 검증합니다.
- **개방형 표준 준수**: MIT 라이선스 기반의 오픈소스이며, MCP(Model Context Protocol)를 상호운용 표준으로 채택합니다.

---

## 기여 및 라이선스

LibrAgent는 MIT 라이선스 기반의 오픈소스 프로젝트입니다. 번들 스킬 개발, MCP 도구 연동, 버그 수정 등 모든 기여를 환영합니다.

- [기여 가이드 (Contributing Guide)](CONTRIBUTING.md)
- [이슈 트래커 (Issue Tracker)](https://github.com/fritzprix/libr-agent/issues) [![Good First Issues](https://img.shields.io/github/issues/fritzprix/libr-agent/good%20first%20issue)](https://github.com/fritzprix/libr-agent/issues?q=is%3Aissue+is%3Aopen+label%3A%22good%20first%20issue%22)
- [커뮤니티 토론 (Discussions)](https://github.com/fritzprix/libr-agent/discussions)
- 벤치마크 (Harbor / Terminal-Bench): [Harbor 가이드](benchmarks/harbor/README.md) (`pnpm bench:diverse`, `pnpm bench:terminal` 등)

**License**: MIT
