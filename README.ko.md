# 🤖 LibrAgent

> **실제 도구를 쓰고, 병렬로 일하고, 당신의 통제 아래 머무는 AI 에이전트를 위한 로컬 우선 데스크톱 앱.**
> _어떤 LLM이든 연결하고, 어떤 MCP 서버든 붙인 뒤, 에이전트가 파일을 읽고, 셸을 실행하고, 웹을 탐색하고, 끝까지 자동화하게 하세요._

[English](./README.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)

LibrAgent는 Tauri + Rust + React로 만든 **로컬 우선 에이전트 워크스페이스**입니다. 단순한 채팅을 넘어서 실제 파일 접근, 셸 실행, 브라우저 자동화, MCP 확장성, 그리고 데모 한 번 돌고 무너지는 게 아니라 몇 시간짜리 작업도 버티는 멀티 에이전트 워크플로우를 위해 설계되었습니다.

클라우드 모델도, Ollama 같은 로컬 런타임도 연결할 수 있습니다. 이미 쓰는 도구에서 MCP 서버를 가져오고, 에이전트에게 코드 조사, 파일 편집, 명령 실행, 웹 탐색, 지식 축적, 하위 작업 위임까지 맡기세요. 작업 전체를 남의 클라우드 VM으로 넘기지 않고도 가능합니다.

**여기서 시작:** [최신 릴리스 다운로드](https://github.com/fritzprix/libr-agent/releases/latest) · [5분 온보딩으로 이동](#5-분-온보딩-경로) · [실전 시나리오 보기](#-실전-시나리오)

---

## 왜 LibrAgent인가?

대부분의 에이전트 제품은 아직도 짜증나는 트레이드오프를 강요합니다:

- **쉬운 UI, 약한 실행력**
- **강한 자동화, 형편없는 제품 경험**
- **편한 클라우드, 약한 프라이버시**
- **유연한 프레임워크, 그런데 스택은 전부 네가 조립**

LibrAgent는 사람들이 실제로 원하는 중간 지점을 노립니다:

- **로컬 우선 제어**: 파일, 워크스페이스, 세션, 브라우저 상태를 기본적으로 로컬에 유지
- **닫힌 플러그인 스토리 대신 MCP 기반의 개방형 확장성**
- **셸, 브라우저, 워크스페이스, 지식 도구를 통한 실제 실행력**
- **파워유저 깊이를 포기하지 않는 GUI**
- **한 명의 에이전트에서 여러 명의 팀으로 자연스럽게 확장되는 구조**

### LibrAgent 는 누구를 위한 것인가

- **솔로 개발자**: 실제로 읽고, 편집하고, 실행하고, 탐색하며 로컬에서 컨텍스트를 유지하는 에이전트를 원하는 사람
- **파워 유저와 운영자**: 로컬 모델, API 제공자, MCP 서버, 예약된 워크플로우로 자신만의 스택을 구성하고 싶은 사람
- **연구자와 분석가**: 브라우저 자동화, 지식 캡처, 반복 가능한 플레이북, 장시간 세션이 필요한 사람
- **개인정보 민감 팀**: 로컬 실행, 명시적 거버넌스, 단일 에이전트에서 조정된 조직으로의 경로를 원하는 팀

---

## 🎬 플랫폼 액션

![LibrAgent Demo](assets/demo_1280_4x_optimized.gif)

_단일 에이전트에서 조정된 군집까지 — 재귀적 위임, MCP 도구, 지속적 워크스페이스가 하나의 통합 서브스트레이트에._

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

### 3. 리서치를 반복 가능한 워크플로우로 바꾸기

- Browser + Knowledge 추가
- _"이 경쟁사 블로그 5개를 추적해서 매일 아침 요약해"_ 라고 요청
- 일회성 작업을 예약 파이프라인으로 전환

### 4. 한 명의 도우미에서 진짜 팀으로 가기

- `specialist-creator`로 전문 에이전트 생성
- `delegate`로 작업 분할
- 반복 협업을 `teamwork` 또는 `org` 워크스페이스로 구조화

---

## 데모 뒤에도 무너지지 않는 이유

### 1. 🔐 로컬 우선 보안 — 데이터는 기계에 남습니다

LibrAgent는 보안을 핵심 아키텍처 원칙으로 다룹니다:

- **세션 격리**: 모든 에이전트 세션이 전용 `MCPServiceProxy` 인스턴스를 받음 — 세션 간 데이터 누출 제로
- **내장 SecurityValidator**: 경로 탐색 공격과 커맨드 인젝션이 시스템 레벨에서 차단
- **클라우드 서브스트레이트 불필요**: 핵심 실행은 로컬에서 이뤄지며, 외부 연결도 주로 사용자가 선택한 클라우드 LLM 제공자와 원격 MCP/HTTP 서비스에 한정되고, 프로덕션에서는 새 릴리스를 확인하는 업데이트 체크가 있을 수 있습니다
- **완전 오프라인 지원**: [Ollama](https://ollama.ai) 와 페어로 완전히 에어갭된 에이전트 스택

#### 로컬에 남는 것 vs 기계 떠나는 것

- **항상 로컬**: 워크스페이스, 로컬 파일, 번들 스킬, 세션 상태, MCP 서버 구성, 브라우저 상태, 로컬 도구 실행
- **필요할 때만 떠남**: 명시적으로 구성한 클라우드 LLM 제공자나 원격 MCP/HTTP 서비스에 대한 요청, 그리고 프로덕션에서 새 릴리스를 확인하는 업데이트 체크
- **완전 오프라인 모드**: Ollama 나 다른 로컬 런타임과 로컬 MCP 서버를 사용하여 에어갭된 워크플로우

### 2. 🧩 MCP 네이티브 생태계 — 설계된 무한 확장성

MCP(Model Context Protocol)는 LibrAgent 확장성 모델의 기반이 되는 오픈 표준입니다. LibrAgent 는 이를 기능이 아닌 아키텍처 백본으로 다룹니다:

- **풀 트랜스포트 지원**: stdio, HTTP, SSE, OAuth 2.1 — 전체 스펙
- **12+ 내장 서버**: Planning, Knowledge(RAG), Browser Automation, Workspace, Shell Execution, Content Store 등
- **프리셋 카탈로그**: GitHub, Brave Search, Filesystem 등 인기 서버를 원클릭으로 설치
- **세션 격리 인스턴스**: 각 에이전트 세션이 독립적인 MCP 서버 상태 보유 — 병렬 에이전트 간 간섭 없음
- **어디서나 가져오기**: Cursor, VS Code, Claude Code, Windsurf에서 MCP 구성을 자동으로 마이그레이션

### 3. 🦾 프로덕션급 실행 서브스트레이트

대부분의 AI 도구는 데모에서는 인상적이지만 프로덕션에서는 취약합니다. LibrAgent 는 장시간 실제 작업을 위해 집요하게 엔지니어링되었습니다:

| 서브스트레이트 | 기능                                                                                  |
| -------------- | ------------------------------------------------------------------------------------- |
| **Workspace**  | 라인 정밀 편집, 멀티 파일 작업, 통합 검색, `@file`/`@skill`/`@playbook` 컨텍스트 주입 |
| **Shell**      | 고립 실행 AND 지속적 셸 — 비동기 프로세스 모니터링(`poll`, `read output`, `list`)     |
| **Browser**    | Playwright와 유사한 상호작용 모델을 갖춘 헤드리스 브라우저 자동화와 캐시 일관성 보장  |
| **Knowledge**  | 엔티티/관계 추출(v2), BM25 전체 텍스트 검색과 함께 그래프 기반 지식 관리              |

**신뢰성 엔지니어링 포함**: Context compaction, loop prevention, circuit breaker, stale-response guard 가 시간 단위 세션에서도 에이전트를 생산적으로 유지합니다.

### 4. 🤝 군집 → 팀 → 조직: 모든 규모의 멀티 에이전트

LibrAgent는 단독 실행에서 명시적 조직 조정까지 일관된 멀티 에이전트 스토리를 갖추고 있습니다:

- **`delegate`**: 부모 에이전트가 명시적 계보 추적과 함께 자식 세션을 생성, 브리핑, 모니터링
- **`teamwork`**: 한 커맨드로 전체 태스크포스 워크스페이스(agents.md, MISSION.md, KANBAN.md) 구성
- **`org`**: 내구성 조직 정체성, 루트 세션 재개, 조직 가시적 구성원 계보로 팀을 공식화
- **`schedule`**: CRON 기반 자동화 — 에이전트가 무감독으로, 스케줄에 따라, 워크스페이스 헌법과 함께 실행
- **Concurrency Gate**: 병렬 세션과 셸 프로세스에 하드 리밋을 적용해 데드락과 비용 폭주 방지

### 5. ⚡ 번들 스킬 — 빈 설치에서 완전 가동 군집까지 가장 빠른 경로

LibrAgent는 성장하는 **번들 스킬** 라이브러리와 함께 제공됩니다. 임의의 프롬프트가 아닙니다 — 모든 에이전트가 이름으로 호출할 수 있는 재사용 가능한 운영 절차입니다.

가장 중요한 day-one 스킬:

| 스킬                 | 기능                                                                  |
| -------------------- | --------------------------------------------------------------------- |
| `system-setup`       | 모든 플랫폼에서 누락된 런타임(Python, Node.js, uv) 감지 및 설치       |
| `mcp-installer`      | npm 패키지, GitHub URL, JSON 구성 블록에서 MCP 서버 등록              |
| `mcp-importer`       | Cursor, VS Code, Windsurf 등에서 기존 MCP 구성 가져오기               |
| `specialist-creator` | 역할 설명에서 풀 에이전트 구성(시스템 프롬프트, 모델, 도구) 설계      |
| `crew-constructor`   | 사용 가능한 도구 스캔 및 일치하는 specialist 팀 배치 생성             |
| `agent-tooling`      | 에이전트 감사, 능력 불일치 감지, 도구 할당 동적 재균형                |
| `delegate`           | 명시적 컨텍스트 전달 및 계보 추적과 함께 부모→자식 세션 인수인도 안내 |
| `teamwork`           | 조정된 멀티 에이전트 작업을 위한 공유 워크스페이스 헌법 scaffold      |
| `org`                | 내구성 조직 정체성 및 org-visible 구성원 계보 공식화                  |
| `schedule`           | 미감정 자동화를 위한 반복 예약 작업 그룹 생성 및 관리                 |
| `soul-awakening`     | 에이전트를 `SOUL.md` 페르소나에 고정 — 톤, 태도, 정체성               |

이것은 운영자 레이어에 불과합니다. LibrAgent는 도메인 스킬도 제공합니다:

- **지식 및 연구**: `deep-research-report`, `knowledge-distiller`
- **작업 흐름**: `document-to-markdown`, `docx`, `pptx`
- **스킬 및 워크플로우 저자**: `skill-creator`, `skill-deployer`, `playbook-creator`, `mcp-builder`
- **특수 작업**: `computer-diagnosis` 및 기타 집중 헬퍼

_참고: `bootstrap`은 이러한 스킬과 함께 자주 사용되는 내장 기능입니다. 번들 스킬은 재사용 가능한 운영 절차이며, 내장 기능과 MCP 도구는 그 하위 실행 기반입니다._

---

## 🌍 현실 세계 시나리오

### 솔로 개발자 — 자동화된 코드 리뷰

1. Workspace 도구로 로컬 리포지토리 연결
2. GitHub MCP 프리셋 설치 (원클릭)
3. 요청: _"PR #42 의 보안 이슈를 찾고 Markdown 보고서 생산"_
4. 에이전트가 코드를 읽고, 분석 실행, 향후 참조를 위해 Knowledge 서버에 발견 사항 저장

### 마케팅 — 경쟁 정보 자동 수집

1. Browser 도구로 5 개 경쟁사 블로그 구성
2. 에이전트에게: _"매일 아침 7시 경쟁사 브리프 예약해줘"_ — `schedule` 스킬로 반복 작업 그룹 설정
3. 에이전트가 탐색, 요약, Knowledge 스토어에 추가
4. 언제든 물어보기: _"지난 주 경쟁사 움직임을 요약해줘"_

### 엔지니어링 팀 — 오프라인 에이전트 스택

1. `ollama pull qwen3:14b` — API 키 없음, 클라우드 없음
2. Workspace + Shell 도구를 코드베이스에 연결
3. 민감한 IP 가 기계 밖으로 나가지 않음
4. 에이전트가 읽고, 수정하고, 테스트하고, 커밋 — 완전 로컬

### 파워 유저 — 멀티 에이전트 연구 파이프라인

1. `crew-constructor` 로 자동 생성: Researcher × 3, Analyst × 1, Writer × 1
2. 오케스트레이터가 `delegate` 스킬로 병렬 위임
3. 결과가 Content Store 의 단일 구조화된 보고서로 병합
4. `schedule` 로 전체 워크플로우를 주간 예약

---

## 📖 문서 및 가이드

- **[탐색 가이드](docs/guides/navigation-guide.md)**: Command & Control 허브 — `/assistants`(역할 정의) 및 `/playbooks`(워크플로우 청사진).
- **[아키텍처 가이드](docs/architecture/agent-workflow-architecture.md)**: 세션 격리, 오케스트레이션 엔진, Rust 기반 Think-Act-Observe 루프.
- **[내장 도구 가이드](docs/guides/builtin_tool_bp.md)**: 도구 설계 표준 및 MCP 응답 패턴.

---

## 📦 시작하기

[릴리스 페이지](https://github.com/fritzprix/libr-agent/releases/latest)에서 플랫폼별 최신 설치 프로그램을 다운로드하세요.

<!-- RELEASE_DOWNLOADS_START -->

- **Windows:** [`LibrAgent_0.8.2_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.2/LibrAgent_0.8.2_x64-setup.exe) · [`LibrAgent_0.8.2_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.2/LibrAgent_0.8.2_x64_en-US.msi)
- **macOS (Apple Silicon):** [`LibrAgent_0.8.2_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.2/LibrAgent_0.8.2_aarch64.dmg)
- **Linux:** [`LibrAgent_0.8.2_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.2/LibrAgent_0.8.2_amd64.AppImage) · [`LibrAgent_0.8.2_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.2/LibrAgent_0.8.2_amd64.deb) · [`LibrAgent-0.8.2-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.8.2/LibrAgent-0.8.2-1.x86_64.rpm)
- **전체 릴리스 자산:** [릴리스 페이지](https://github.com/fritzprix/libr-agent/releases/tag/v0.8.2)
<!-- RELEASE_DOWNLOADS_END -->

**개발자 설정:**

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

### 5 분 온보딩 경로

**1 단계 — 모델 연결** (Settings → LLM Providers)

- 클라우드: OpenAI / Anthropic / Gemini / Groq API 키 붙여넣기
- 로컬: `ollama pull qwen3:14b` 후 Settings 에서 Ollama 선택
- Cursor 나 VS Code 사용 중? 에이전트에게 알려주기: _"Cursor 에서 MCP 서버 가져와"_ → `mcp-importer` 가 처리

**2 단계 — MCP 도구 추가** (Extensions 사이드바)

- 프리셋 카탈로그 탐색 및 Install 클릭, 또는
- 에이전트에게 알려주기: _"Install @modelcontextprotocol/server-everything"_ → `mcp-installer` 가 자동 등록

**3 단계 — 첫 에이전트 생성**

- _"경쟁 정보 수집을 위한 리서처 에이전트 생성"_ → `specialist-creator`가 완전한 구성 설계
- _"현재 도구로 연구 팀 빌드"_ → `crew-constructor`가 팀 일괄 생성
- _"모든 에이전트 간 도구 할당 최적화"_ → `agent-tooling`이 감사하고 재균형 조정

**4 단계 — `delegate`로 병렬 작업**

- 어떤 에이전트에게든 자식 세션으로 하위 작업 위임 요청
- `delegate` 스킬이 컨텍스트 전달, 계보 추적, 결과 병합 관리

**5 단계 — 지속적 팀 빌드**

- `teamwork` → `agents.md`, `MISSION.md`, `KANBAN.md`와 함께 공유 워크스페이스 구성
- `org` → 내구성 정체성 및 org-root 세션 관리로 팀 공식화
- `schedule` → 무감독 CRON 기반 자동화를 에이전트가 생성 및 관리

### 복사-붙여넣기 첫 프롬프트

- _"Cursor에서 MCP 서버 가져와서 뭐가 추가됐는지 보여줘."_
- _"현재 도구로 경쟁 정보 수집을 위한 리서처 에이전트 생성."_
- _"GitHub MCP 프리셋 설치하고 코딩 에이전트에 연결."_
- _"리포지토리 분석을 자식 세션에 위임하고 요약 가져와."_
- _"이 리포지토리를 위한 teamwork 워크스페이스 준비하고, org-ready 스페셜리스트 팀 생성."_
- _"매일 아침 7시 경쟁사 일일 브리프 예약하고 공유 teamwork 워크스페이스에 유지."_

---

## LibrAgent가 특히 잘 맞는 경우

| 이런 걸 원한다면...                                             | LibrAgent가 강한 이유                                                                 |
| --------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| **로컬 AI 워크스테이션**                                        | 파일, 세션, 워크스페이스, 브라우저 상태가 기본적으로 로컬에 남습니다                  |
| **MCP 네이티브 데스크톱 제품**                                  | 얇은 래퍼가 아니라 MCP 서버를 설치, 가져오기, 관리하는 경험 자체가 제품 안에 있습니다 |
| **실제로 일하는 에이전트**                                      | Workspace, Shell, Browser, Knowledge 도구가 장시간 실행을 전제로 설계되었습니다       |
| **프레임워크를 먼저 짜지 않아도 되는 멀티 에이전트 워크플로우** | `delegate`, `teamwork`, `org`, `schedule`가 이미 제품 안에 들어 있습니다              |
| **GUI 사용성과 파워유저 깊이의 균형**                           | 데스크톱 UI를 쓰면서도 확장성과 통제력을 잃지 않습니다                                |

---

## 설계 철학

- **로컬 우선**: 데이터, API 키, 에이전트 "souls"이 전적으로 당신의 통제 하에 있습니다. 클라우드 인프라가 불필요합니다.
- **모델보다 하니스**: 실행 환경 — 도구, 세션 상태, 위임, 거버넌스 — 이 개별 모델보다 중요합니다. LibrAgent는 어떤 모델이든 최대 성능을 발휘하도록 설계되었습니다.
- **기능보다 안정성**: CHANGELOG는 세션 격리, compaction, 루프 방지, stale-response guard 등 런타임 안정성에 대한 집요한 집중을 반영합니다 — 단순한 기능 추가가 아닙니다.
- **인프라로서의 MCP**: 플러그인 시스템이 아닙니다. 전체 도구 생태계가 MCP를 핵심 상호운용성 레이어로 구조화되어 있습니다.
- **오픈 표준**: MIT 라이선스. MCP, 오픈소스 상호운용성, 사용자 데이터 주권에 완전히 헌신합니다.

---

## 기여 및 라이선스

LibrAgent 는 MIT 라이선스로 오픈소스로 구축되었습니다. 기여 환영 — 새 번들 스킬, MCP 통합, 버그 수정, 아키텍처 개선 모두.

- 📖 [기여 가이드](CONTRIBUTING.md)
- 🐛 [이슈 트래커](https://github.com/fritzprix/libr-agent/issues)
- 💬 [토론](https://github.com/fritzprix/libr-agent/discussions)

**라이선스**: MIT
