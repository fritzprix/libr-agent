# 🤖 LibrAgent

> **Your PC. Your data. Your AI agents that actually work.**
> Connect any LLM, add any tool, and let agents read files, run shells, browse the web, and automate real tasks — all locally.

---

## 한 줄 소개

**LibrAgent**는 Tauri + Rust + React로 만든 로컬 우선 AI 에이전트 워크스페이스입니다. 채팅을 넘어 파일 편집, 쉘 실행, 브라우저 자동화, MCP 확장을 모두 내 PC에서 실행합니다.

---

## 왜 LibrAgent인가?

대부분의 AI 에이전트 제품은 다음 중 하나를 강요합니다:

|                                             | 문제점                       |
| ------------------------------------------- | ---------------------------- |
| 쉬운 UI, 약한 실행                          | 데모용이고 실제 작업은 못 함 |
| 강력한 자동화, 제품 polish 부재             | 전문가만 쓰기도 귀찮음       |
| 클라우드 편의, 약한 프라이버시              | 내 코드·데이터가 외부로 나감 |
| 프레임워크 유연성, 하지만 내가 다 쌓아야 함 | 인프라 구축에 시간 다 써버림 |

LibrAgent는 **그 중간**을 줍니다:

- ✅ **로컬 우선** — 파일, 워크스페이스, 세션, 브라우저 상태 모두 내*machine*
- ✅ **오픈 확장** — MCP 표준으로 무한 확장, 폐쇄적 플러그인 아님
- ✅ **진짜 실행** — 쉘, 브라우저, 워크스페이스, 지식 도구 모두 실제 동작
- ✅ **사람이 쓰는 GUI** — 파워유저 깊이, 일반인 진입장벽 낮음
- ✅ **단일 → 다수 에이전트** — 한 assistant로는 부족할 때 swarm으로 확장

---

## 핵심 기능 7가지

| 기능                | 설명                                                                                          |
| ------------------- | --------------------------------------------------------------------------------------------- |
| 📁 **Workspace**    | 라인 단정밀 편집, 멀티파일 작업, 검색, `@file`/`@skill` 컨텍스트 주입                         |
| 💻 **Shell**        | 고립 실행 + 지속 셸, 비동기 프로세스 모니터링 (`poll`, `read`, `list`)                        |
| 🌐 **Browser**      | Headless 브라우저 자동화, Playwright급 상호작용, 캐시 일관성 보장                             |
| 🧠 **Knowledge**    | 그래프 기반 지식 관리 (엔티티/관계 추출), BM25 전체 텍스트 검색                               |
| 🧩 **MCP 네이티브** | stdio/HTTP/SSE/OAuth 2.1 완전 지원, 15+ 내장 서버, 1클릭 프리셋 설치                          |
| 🤝 **멀티에이전트** | `delegate`(자녀 세션), `teamwork`(공유 워크스페이스), `org`(공식 팀), `schedule`(CRON 자동화) |
| 🎯 **번들 스킬**    | `system-setup`, `mcp-installer`, `deep-research`, `git-workflow` 등 30+ 재사용 절차           |

---

## 사용 시나리오 3가지

### 1. 개발자 — 로컬 코드 리뷰 자동화

1. 로컬 리포지토리 연결
2. GitHub MCP 프리셋 설치 (1클릭)
3. `"PR #42의 보안 이슈 찾아서 마크다운 리포트 써줘"`
4. 에이전트가 코드 읽기, 분석, 지식 저장 — 전체 과정 로컬

### 2. 연구자 — 경쟁사 모니터링 오토메이션

1. Browser 도구로 5개 경쟁사 블로그 설정
2. `"매일 아침 7시에 경쟁사 요약 보내줘"`
3. 에이전트가 매일 브라우저 방문, 요약, 지식 저장소 추가
4. `"지난 주 경쟁사 동향 요약해줘"` — 언제든지 질문

### 3. 팀 — 오프라인 에이전트 스택

1. `ollama pull qwen3:14b` — API 키 없음, 클라우드 없음
2. Workspace + Shell 도구 연결
3. 민감한 IP가 절대 외부로 나가지 않음
4. 에이전트가 읽기, 수정, 테스트, 커밋 — 완전 로컬

---

## 기술 스택

| 레이어         | 기술                                                |
| -------------- | --------------------------------------------------- |
| **백엔드**     | Rust, Tokio, SeaORM, rmcp (MCP 클라이언트)          |
| **프론트엔드** | React 18, TypeScript 5.6, Tailwind CSS 4, shadcn/ui |
| **데스크톱**   | Tauri 2.x (크로스플랫폼)                            |
| **빌드**       | Vite 6, PNPM 9                                      |

---

## 다운로드

- **GitHub Releases:** https://github.com/fritzprix/libr-agent/releases/latest
- **License:** MIT
- **OS:** Windows (x64), macOS (Apple Silicon), Linux

---

## 빠른 시작 (5분)

```bash
# 1. 설치 (Windows/macOS/Linux 인스톨러 다운로드)
# 2. 앱 실행 → LLM 공급자 연결 (OpenAI, Ollama 등)
# 3. MCP 서버 추가 (GitHub, Brave Search 등 1클릭 프리셋)
# 4. 에이전트에게 작업 위임 — 끝
```

---

_LibrAgent — AI가 네 일을 대신 하게 하되, 데이터는 네 것이게._
