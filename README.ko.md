# 🤖 LibrAgent

> **상태 유지가 가능한 가벼운 자율형 AI 에이전트 플랫폼.**

[English](./README.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)

LibrAgent는 상호작용 간의 맥락을 유지하도록 설계된 로컬 우선(local-first) 에이전트 실행 도구입니다. 기존의 일회성 클라이언트와 달리 브라우저 탭과 터미널 세션을 턴 사이에도 유지하여, 에이전트가 영구적인 작업 공간 내에서 더 자연스럽게 일할 수 있게 돕습니다.

**MCP (Model Context Protocol)** 및 **Skills**와 같은 공개 표준을 지원하여 높은 확장성을 제공합니다.

---

## 왜 만들었나?

LibrAgent의 목표는 누구나 쉽게 자율형 에이전트를 사용하는 환경을 만드는 것입니다. 많은 최신 도구들이 여전히 터미널 명령어와 복잡한 JSON 설정 뒤에 머물러 있어, 대다수의 사용자가 이 기술의 혜택을 누리는 데 장벽이 되고 있습니다. LibrAgent는 이 기술적 격차를 해소하여 전문가가 아니더라도 누구나 로컬 환경에서 자신만의 에이전트를 구동하고 관리할 수 있도록 돕고자 합니다.

---

## 🎬 Demo

![LibrAgent Demo](assets/demo_1280_4x_optimized.gif)

_실시간 브라우저 제어와 셸 명령을 단일 워크플로우에서 실행하는 에이전트릭 루프._

---

## 주요 기능

### 1. 영구 작업 공간 (Persistent Workspace)

에이전트는 매 턴마다 새로운 프로세스를 띄우는 대신, 기존 환경에서 연속적으로 작업합니다.

- **Live Webview**: Tauri 기반의 실시간 브라우저 자동화. 세션과 쿠키가 턴 간에 유지됩니다.
- **Unified Terminal**: 워크스페이스와 상태를 공유하며 샌드박스 처리된 통합 셸 (Python/Node.js 지원).

### 2. 멀티 에이전트 오케스트레이션

LibrAgent는 에이전트가 전문화된 하위 에이전트에게 작업을 위임할 수 있게 합니다.

- **Assistants**: 전용 시스템 프롬프트와 도구 설정을 가진 에이전트 프로필 관리.
- **Swarm Intelligence**: 부모 에이전트가 서브 에이전트를 생성하고 지시하며 결과를 수집하여 복잡한 문제를 해결합니다.

### 3. 확장성

플랫폼은 커뮤니티 표준을 통해 확장되도록 설계되었습니다.

- **Extensions (MCP)**: Model Context Protocol 완벽 지원. 모든 MCP 서버에 즉시 연결 가능합니다.
- **원클릭 프리셋**: GitHub, Brave Search 등 검증된 MCP 서버 카탈로그를 UI에서 즉시 설치할 수 있습니다.
- **Skills & Playbooks**: 재사용 가능한 행동 스니펫과 정형화된 워크플로우 템플릿.

### 4. 자율성 및 스케줄링

- **YOLO Mode**: 매번 승인받지 않고 민감한 도구를 자율적으로 실행하도록 선택할 수 있습니다.
- **Scheduled Tasks**: 크론(Cron) 기반 예약 자동화. 지정된 워크스페이스에서의 작업 및 자동 복구를 지원합니다.

### 5. 문맥 지능 및 지표

- **@mention**: 파일, 스킬, 플레이북을 채팅창에 즉시 주입.
- **멀티모달**: OpenAI, Anthropic, Gemini 모델에 대한 이미지 및 오디오 처리 지원.
- **모니터링**: 실시간 TPS 지표 및 프롬프트 캐싱(Anthropic/Gemini) 히트율 표시.

---

## 📦 설치

[Release](https://github.com/fritzprix/libr-agent/releases/latest) 페이지에서 Windows, macOS, Linux용 최신 패키지를 다운로드하세요.

**소스에서 빌드:**

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## 설계 철학

- **로컬 우선**: 당신의 데이터와 API 키는 당신의 컴퓨터에 머뭅니다.
- **Tauri + Rust**: 보안(메모리 안전성), 성능, 그리고 작은 바이너리 크기를 위해 선택했습니다.
- **SQLite (SeaORM)**: 세션과 설정의 견고한 로컬 영속성을 위해 사용됩니다.

---

## 기여 및 라이선스

기여는 언제나 환영합니다. `CONTRIBUTING.md`를 참고해 주세요.

**라이선스**: MIT
