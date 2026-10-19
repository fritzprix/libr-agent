---
title: LibrAgent — 로컬 AI 에이전트 데스크톱 운영 환경
description: '로컬 에이전트 운영 환경 LibrAgent (libr-agent) — 모델 선택, 앱처럼 MCP 도구 설치, 멀티 에이전트 협업 패턴 실행. 설치·모델 연결·채팅·자동화 가이드.'
---

# LibrAgent 사용자 문서

> **모델을 선택하고, 앱처럼 도구를 설치하고, 협업 패턴을 실행하는 로컬 AI 에이전트 운영 환경.**
> 복잡한 JSON 설정이나 특정 벤더 종속 없이 결과물이 내 머신의 파일로 완성됩니다.

언어: **한국어** · [English](/en/) · [**최신 릴리스 다운로드 (Windows / macOS / Linux)**](https://github.com/fritzprix/libr-agent/releases/latest) · [GitHub 저장소](https://github.com/fritzprix/libr-agent)

---

## 왜 LibrAgent인가요?

- **수동 설정 없는 직관적 UI**: JSON 편집이나 터미널 없이 **Extensions**에서 원클릭으로 MCP 프리셋(GitHub, Brave Search 등)을 설치하고 Cursor/VS Code 설정을 가져옵니다.
- **내장된 멀티 에이전트 오케스트레이션**: `@skill:pipeline`, `hub-spoke`, `divide-conquer`, `consensus-delegation`, `teamwork` 등 검증된 협업 패턴을 채팅에서 바로 실행합니다.
- **모델 및 스택 자유도 (Local-First, MIT)**: OpenAI, Anthropic, Gemini, Groq API 키는 물론 로컬 **Ollama** 모델을 동일한 데스크톱 환경에서 연결합니다.

---

## 시작하기

- [**5분 시작 가이드**](getting-started/5-minute-tutorial.md) — 설치, **AI & Models**, Chat, **App Wizard**
- [**에이전트 첫 대화**](getting-started/first-agent.md)
- [**모델 연결하기**](getting-started/connecting-models.md)

## 가이드

- [**Assistants**](guides/assistants.md) — 맞춤 에이전트 (General / Tools / Skills)
- [**Playbooks**](guides/playbooks.md) — 성공한 작업을 다시 실행
- [**Solution Recipes**](guides/recipes.md) — 원클릭 솔루션 레시피 (프리셋+스케줄)
- [**예약 작업 (Scheduled Tasks)**](guides/scheduled-tasks.md) · [**자동화 개요**](guides/automation.md) · [**세션 루프 (loop)**](guides/loop.md) · [**끝나면 이어서 (call-me-back)**](guides/call-me-back.md)
- [**세션 관리**](guides/sessions.md) · [**세션 내보내기**](guides/session-export.md)
- [**스킬**](guides/skills.md) · [**서브 에이전트**](guides/sub-agents.md)
- [**Extensions (MCP 도구)**](guides/extensions.md) · [**커스텀 MCP**](guides/custom-mcp.md)
- [**브라우저 자동화**](guides/browser-sidecar.md) (전용 브라우저 · [일상 Chrome 확장](https://chromewebstore.google.com/detail/libragent-browser-bridge/eponcjengahlpkgbaolhemhmckopmfbe)) · [**내장 도구**](guides/builtin-tools.md)
- [**네비게이션 가이드**](guides/navigation-guide.md) · [**문제 해결**](guides/troubleshooting.md)

## FAQ · 시나리오

- [**FAQ**](faq/common-questions.md) · [**에러 코드**](faq/error-codes.md)
- [자동화](scenarios/automation.md) · [코드 리뷰](scenarios/code-review.md) · [조사](scenarios/research.md) · [파일](scenarios/file-management.md) · [웹](scenarios/web-browsing.md)

### 온보딩 한줄

1. **Settings → AI & Models → Provider API Keys** → **Save Changes**
2. **Chat**에서 어시스턴트 카드로 세션 시작
3. MCP는 **Extensions**
4. 반복 작업은 **Playbooks** / **Scheduled Tasks**

---

## 팁

- Settings 탭 이름: **AI & Models**. MCP: 사이드바 **Extensions**.
- 개발자: [docs/guides/getting-started-dev.md](https://github.com/fritzprix/libr-agent/blob/main/docs/guides/getting-started-dev.md)
- 질문: [Discussions](https://github.com/fritzprix/libr-agent/discussions)
