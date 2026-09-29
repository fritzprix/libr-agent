# 🤖 LibrAgent

> **실제 도구를 쓰고, 병렬로 일하고, 당신의 통제 아래 머무는 AI 에이전트를 위한 로컬 우선 데스크톱 앱.**
> _어떤 LLM이든 연결하고, 어떤 MCP 서버든 붙인 뒤, 에이전트가 파일을 읽고, 셸을 실행하고, 웹을 탐색하고, 작업을 내 머신에서 끝내게 하세요._

[English](./README.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![Version](https://img.shields.io/github/v/release/fritzprix/libr-agent)](https://github.com/fritzprix/libr-agent/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/React-18.3-61DAFB?logo=react)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178C6?logo=typescript)](https://www.typescriptlang.org)

---

## 🎬 실행 스토리

**LibrAgent는 채팅 앱이 아닙니다. 에이전트를 위한 실행 환경입니다.**

**한 세션. 한 목표. 내 머신에 남는 산출물 하나.**

1. 모델 연결 (API 키 또는 [Ollama](https://ollama.com))
2. **Workspace**에 실제 프로젝트 폴더 연결
3. 에이전트에게 **읽고, 실행하고, 파일로 남기라**고 요청 — 말풍선 속 제안으로 끝내지 않기

[최신 릴리스 다운로드](https://github.com/fritzprix/libr-agent/releases/latest) · [5분 온보딩](#5분-온보딩-경로) · [히어로 데모 스펙](docs/contributing/hero-demo-spec.md)

---

## 처음 10분 안에 할 수 있는 일

### 1. 실제 도구로 저장소 리뷰하기

- Workspace 도구로 로컬 저장소 연결
- GitHub MCP 프리셋 추가
- _"PR #42의 보안 이슈를 찾아서 보고서로 저장해"_ 라고 요청

### 2. 완전한 로컬 에이전트 스택 만들기

- `ollama pull qwen3:14b` 실행
- Workspace + Shell 연결
- 코드를 클라우드 VM으로 보내지 않고 에이전트가 읽고, 수정하고, 테스트하고, 반복하게 만들기

### 3. 원클릭 워크플로우 레시피 배포하기

- Chat 홈 또는 [예약 작업](docs/user/guides/scheduled-tasks.md)에서 **Morning Briefing** 레시피 실행
- Hacker News + Yahoo Finance 프리셋 설치, 전용 어시스턴트 구성, 매일 오전 9시 일정 등록
- 매일 아침 합성된 기술·시장 브리핑을 무인으로 전달

### 4. 리서치를 반복 가능한 워크플로우로 바꾸기

- Browser + Knowledge 추가
- _"이 경쟁사 블로그 5개를 추적해서 매일 아침 요약해"_ 라고 요청
- [예약 작업](docs/user/guides/scheduled-tasks.md)으로 일회성 작업을 자동화 파이프라인으로 전환

---

## 왜 LibrAgent인가?

대부분의 에이전트 제품은 트레이드오프를 강요합니다: 쉬운 UI에 약한 실행력, 강한 자동화에 빈약한 제품 경험, 편한 클라우드에 약한 프라이버시, 또는 전부 직접 조립해야 하는 프레임워크.

LibrAgent는 사람들이 실제로 원하는 중간 지점을 노립니다:

- **로컬 우선 제어**: 파일, 워크스페이스, 세션, 브라우저 상태
- **MCP 기반 개방형 확장성** (닫힌 플러그인 스토리가 아님)
- **셸·브라우저·워크스페이스·지식 도구를 통한 실제 실행력**
- **파워유저 깊이를 포기하지 않는 GUI**
- **한 명의 에이전트에서 여러 명의 팀으로 확장되는 경로**

**잘 맞는 사람:** 솔로 개발자, 파워 유저/운영자, 연구자·분석가, 로컬 실행과 거버넌스가 필요한 팀.

---

## 데모 뒤에도 무너지지 않는 이유

### 로컬 우선 보안

- **세션 격리**: 에이전트 세션마다 독립 도구 런타임 — 세션 간 데이터 누출 없음
- **경로·명령 가드**: 경로 탐색·커맨드 인젝션을 시스템 경계에서 차단
- **핵심 작업은 로컬**; 클라우드 LLM / 원격 MCP는 직접 구성할 때만 (선택적 업데이트 확인 포함)
- **완전 오프라인**: [Ollama](https://ollama.com) + 로컬 MCP

| 항상 로컬 | 선택할 때만 나감 |
| --------- | ---------------- |
| 워크스페이스, 파일, 스킬, 세션 상태, 브라우저 상태, 로컬 도구 | 클라우드 LLM, 원격 MCP/HTTP, 릴리스 업데이트 확인 |

### MCP 네이티브 플랫폼

- 트랜스포트: stdio, HTTP, SSE, OAuth 2.1
- 15+ 내장 서버 (Planning, Knowledge, Browser, Workspace, Shell, Content Store, …)
- 원클릭 프리셋 (GitHub, Brave Search, Filesystem, …)
- Cursor / VS Code / Claude Code / Windsurf에서 MCP 구성 가져오기

### 실행 기반

| 기반          | 기능                                                                                          |
| ------------- | --------------------------------------------------------------------------------------------- |
| **Workspace** | 라인 정밀 편집, 멀티 파일 작업, `@file` / `@skill` / `@playbook` 컨텍스트                     |
| **Shell**     | 격리·지속 셸과 비동기 프로세스 모니터링                                                       |
| **Browser**   | 격리된 브라우저 사이드카 (Playwright 스타일 상호작용)                                         |
| **Knowledge** | 그래프 지식 + BM25 검색                                                                       |
| **Export**    | Markdown 보고서와 ATIF 궤적 내보내기 ([세션 내보내기](docs/user/guides/session-export.md)) |

장시간 세션은 context compaction, 루프 방지, circuit breaker, stale-response guard로 유지됩니다.

### Day-one 스킬

에이전트가 이름으로 호출하는 재사용 운영 절차:

| 스킬             | 역할                                                                    |
| ---------------- | ----------------------------------------------------------------------- |
| `setup-wizard`   | 누락 런타임(Python, Node.js, uv) 감지·설치                              |
| `tool-installer` | MCP 서버 등록 또는 Cursor / VS Code / Windsurf 구성 가져오기            |
| `schedule`       | 반복 예약 작업 그룹 생성                                                |
| `delegate`       | 부모→자식 세션 인수인도와 계보 추적                                     |
| `teamwork`       | 공유 멀티 에이전트 워크스페이스 헌법 scaffold                           |

전체 목록: **[번들 스킬 가이드](docs/user/guides/skills.md)**.

---

## 에이전트 하나가 부족할 때

첫 산출물이 동작한 뒤, 프레임워크를 조립하지 않고 확장합니다:

- **`delegate`** — 자식 세션 생성·모니터링
- **`teamwork`** — 공유 워크스페이스 (`agents.md`, `MISSION.md`, `KANBAN.md`)
- **`org`** — 내구성 있는 팀 정체성·계층
- **`schedule`** — 워크스페이스 헌법과 함께 CRON 자동화

동시성 제한으로 병렬 세션·셸의 비용 폭주를 막습니다.

---

## 실전 시나리오

### 솔로 개발자 — 자동 코드 리뷰

1. Workspace로 로컬 리포 연결
2. GitHub MCP 프리셋 설치
3. _"PR #42 보안 이슈를 찾아 Markdown 보고서로 남겨"_ 
4. 에이전트가 읽고 분석한 뒤, 열어볼 수 있는 보고서를 남김

### 마케팅 — 경쟁 정보 자동 수집

1. Browser로 경쟁사 블로그 연결
2. _"매일 아침 7시 경쟁사 브리프 예약해줘"_
3. 에이전트가 탐색·요약 후 Knowledge에 저장
4. 언제든 _"지난주 경쟁사 움직임 요약해줘"_

### 엔지니어링 팀 — 오프라인 에이전트 스택

1. `ollama pull qwen3:14b`
2. Workspace + Shell 연결
3. 민감 코드는 머신 안에 유지
4. 읽고, 수정하고, 테스트하고, 커밋 — 완전 로컬

### 파워 유저 — 멀티 에이전트 연구 파이프라인

1. `teamwork`으로 역할·공유 문서 scaffold
2. `delegate`로 병렬 리서치
3. Content Store에 단일 보고서로 병합
4. `schedule`로 주간 실행

---

## 문서

- **[사용자 가이드](docs/user/README.md)** — 설치, 첫 채팅, 모델, 스킬 ([문서 사이트](https://fritzprix.github.io/libr-agent/))
- **[히어로 데모 스펙](docs/contributing/hero-demo-spec.md)** — 60초 제품 스토리 기준 (EN/KO/ZH 자막)
- **[레시피](docs/user/guides/recipes.md)** · **[예약 작업](docs/user/guides/scheduled-tasks.md)** · **[브라우저](docs/user/guides/browser-sidecar.md)** · **[스킬](docs/user/guides/skills.md)**
- **[HTTP API](docs/api/http_api.md)** — 원격 제어·프로그래매틱 승인
- **[아키텍처](docs/architecture/agent-workflow-architecture.md)** — 세션 격리와 Think-Act-Observe

---

## 시작하기

[릴리스 페이지](https://github.com/fritzprix/libr-agent/releases/latest)에서 플랫폼별 최신 설치 프로그램을 다운로드하세요.

<!-- RELEASE_DOWNLOADS_START -->
- **Windows:** [`LibrAgent_0.9.20_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.20/LibrAgent_0.9.20_x64-setup.exe) · [`LibrAgent_0.9.20_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.20/LibrAgent_0.9.20_x64_en-US.msi)
- **macOS (Apple Silicon):** [`LibrAgent_0.9.20_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.20/LibrAgent_0.9.20_aarch64.dmg)
- **Linux:** [`LibrAgent_0.9.20_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.20/LibrAgent_0.9.20_amd64.AppImage) · [`LibrAgent_0.9.20_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.20/LibrAgent_0.9.20_amd64.deb) · [`LibrAgent-0.9.20-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.20/LibrAgent-0.9.20-1.x86_64.rpm)
- **전체 릴리스 자산:** [릴리스 페이지](https://github.com/fritzprix/libr-agent/releases/tag/v0.9.20)
<!-- RELEASE_DOWNLOADS_END -->

### 5분 온보딩 경로

**1단계 — 모델 연결** (Settings → LLM Providers)

- 클라우드: OpenAI / Anthropic / Gemini / Groq API 키 붙여넣기
- 로컬: `ollama pull qwen3:14b` 후 Settings에서 Ollama 선택

**2단계 — 워크스페이스와 도구 연결**

- Workspace에 실제 프로젝트 폴더 지정
- 선택: Extensions에서 프리셋(예: GitHub) 설치, 또는 _"Cursor에서 MCP 서버 가져와"_

**3단계 — 산출물 요청**

- _"이 리포에서 신규 기여자에게 가장 큰 리스크를 찾아 워크스페이스에 `DELIVERABLE.md`로 저장해."_
- 디스크에서 열 수 있는 결과물을 우선 — 채팅 속 제안만으로 끝내지 않기

**다음 (에이전트가 하나 이상 필요할 때)**

- _"리포 분석을 자식 세션에 위임하고 요약 가져와."_
- _"이 리포용 teamwork 워크스페이스 준비해."_
- _"매일 아침 7시 경쟁사 브리프 예약해."_

### 복사-붙여넣기 첫 프롬프트

- _"Cursor에서 MCP 서버 가져와서 뭐가 추가됐는지 보여줘."_
- _"GitHub MCP 프리셋 설치하고 코딩 에이전트에 연결."_
- _"이 워크스페이스를 리뷰한 뒤 결과를 `DELIVERABLE.md`에 써."_
- _"현재 도구로 경쟁 정보용 리서처 에이전트 만들어."_
- _"매일 아침 7시 경쟁사 브리프 예약해."_

### 개발자 설정

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## LibrAgent가 특히 잘 맞는 경우

| 이런 걸 원한다면...                                             | LibrAgent가 강한 이유                                                                 |
| --------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| **로컬 AI 워크스테이션**                                        | 파일·세션·워크스페이스·브라우저 상태가 기본적으로 로컬에 남음                         |
| **MCP 네이티브 데스크톱 제품**                                  | 얇은 래퍼가 아니라 설치·가져오기·관리가 제품 안에 있음                                |
| **실제로 일하는 에이전트**                                      | Workspace·Shell·Browser·Knowledge가 장시간 실행 전제                                  |
| **프레임워크를 먼저 짜지 않아도 되는 멀티 에이전트**            | `delegate`, `teamwork`, `org`, `schedule`가 이미 제품 안에 있음                       |
| **GUI 사용성과 파워유저 깊이의 균형**                           | 데스크톱 UI를 쓰면서도 확장성과 통제력을 잃지 않음                                    |

---

## 설계 철학

- **로컬 우선**: 데이터, 키, 에이전트 페르소나는 당신의 통제 아래. 클라우드 기판 불필요.
- **모델보다 하니스**: 도구·세션·위임·거버넌스가 개별 모델보다 중요.
- **기능보다 안정성**: 격리·compaction·루프 방지 등 런타임 정확성이 우선.
- **인프라로서의 MCP**: 도구 생태계의 상호운용성 레이어.
- **오픈 표준**: MIT. MCP·오픈소스 상호운용성·데이터 주권.

---

## 기여 및 라이선스

LibrAgent는 MIT 라이선스 오픈소스입니다. 번들 스킬, MCP 통합, 버그 수정, 아키텍처 개선 모두 환영합니다.

- 📖 [기여 가이드](CONTRIBUTING.md)
- 🐛 [이슈 트래커](https://github.com/fritzprix/libr-agent/issues) [![Good First Issues](https://img.shields.io/github/issues/fritzprix/libr-agent/good%20first%20issue)](https://github.com/fritzprix/libr-agent/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22)
- 💬 [토론](https://github.com/fritzprix/libr-agent/discussions)
- 🧪 벤치마크 (Harbor / Terminal-Bench): [Harbor 가이드](benchmarks/harbor/README.md) (`pnpm bench:diverse`, `pnpm bench:terminal`, …)

**라이선스**: MIT
