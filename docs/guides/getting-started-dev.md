# Getting Started — Developer Setup

> LibrAgent의 개발 환경 구축 가이드. 일반 사용자는 [사용자 문서](../user/README.md)를 참고하세요.

---

## Prerequisites

| 항목                    | 버전 / 설명                                              |
| ----------------------- | -------------------------------------------------------- |
| **Rust**                | [rustup.rs](https://rustup.rs/) — `rustc --version` 확인 |
| **Node.js**             | 20+ (LTS)                                                |
| **pnpm**                | 9.15.9 (corepack로 pinned)                               |
| **System deps (Linux)** | 아래 패키지 목록 참조                                    |

### Linux 시스템 의존성

Debian/Ubuntu:

```bash
sudo apt-get update && sudo apt-get install -y \
  libglib2.0-dev libgtk-3-dev libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev libwebkit2gtk-4.1-dev \
  build-essential curl wget file libxdo-dev \
  libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

> **WebKit 관련**: 컨테이너/헤드리스 환경에서 `webkit2gtk` 오류 발생 시, 실제 데스크탑 세션에서 실행하세요. 소프트웨어 렌더링 플래그 강제 사용은 추천하지 않습니다.
>
> **Source**: [`src-tauri/src/lib.rs`](../../src-tauri/src/lib.rs) (Lines 188–250)

---

## Installation

```bash
# 1. Clone
git clone https://github.com/fritzprix/libr-agent.git
cd libr-agent

# 2. pnpm pinned 활성화 (처음 한 번만)
corepack enable
corepack prepare pnpm@9.15.9 --activate

# 3. 의존성 설치
pnpm install --frozen-lockfile
```

> API 키는 `.env` 파일이 아닌 **앱 내 Settings**에서 관리합니다. 개발 시에도 `.env`가 필요하지 않습니다.

---

## Running

| 명령어             | 용도                                      |
| ------------------ | ----------------------------------------- |
| `pnpm tauri dev`   | 전체 Tauri 데스크톱 앱 (백엔드 포함, HMR) |
| `pnpm dev`         | 프론트엔드 Vite dev 서버만                |
| `pnpm build`       | 프론트엔드 프로덕션 빌드                  |
| `pnpm tauri build` | 프로덕션 데스크톱 앱 번들                 |

---

## Code Quality & Validation

```bash
# 단일 검증 명령어 (CI 파이프라인과 동일)
pnpm refactor:validate

# 개별 명령어
pnpm lint          # ESLint
pnpm format        # Prettier 포맷 체크
pnpm rust:fmt      # rustfmt 체크
pnpm rust:clippy   # Rust 린터
pnpm dead-code     # 미사용 코드를 찾습니다 (unimported)
```

> PR 제출 전 반드시 `pnpm refactor:validate`를 실행하세요.

---

## Testing

| 유형         | 위치               | 실행 방법                                  |
| ------------ | ------------------ | ------------------------------------------ |
| **Frontend** | `src/`             | `pnpm test:run` (Vitest)                   |
| **Backend**  | `src-tauri/tests/` | `cargo test --tests` (CI 통합 테스트 전용) |

> **주의**: Rust의 `#[cfg(test)]` 블록은 CI에서 실행되지 않습니다. 테스트는 반드시 `src-tauri/tests/`에 통합 테스트로 작성하세요.

---

## Architecture Overview

```
src/
├── app/              # 앱 진입점, 루트 레이아웃, 전역 프로바이더
├── components/       # 공유 UI 컴포넌트
├── features/         # 기능별 컴포넌트, 훅, 로직
├── hooks/            # 재사용 가능한 훅
├── lib/              # 서비스 레이어, 비즈니스 로직, API
│   └── backend/      # Tauri 명령어 래퍼 (safeInvoke)
├── models/           # TypeScript 타입/인터페이스
└── context/          # React Context 프로바이더

src-tauri/src/
├── agent/            # 에이전트 오케스트레이션
├── commands/         # Tauri 명령어 핸들러
├── mcp/              # MCP 통합 (내장/외부 서버)
├── models/           # 데이터 모델
├── repositories/     # 데이터 접근 계층
└── main.rs           # 진입점
```

---

## Key Patterns

### Backend → Frontend 통신

- 모든 Tauri 명령어는 `src/lib/backend/`의 `safeInvoke()` 래퍼를 통해 호출
- 중앙 로깅: `getLogger('ComponentName')` (console.\* 금지)
- 에러 처리: `Result<T, E>` + 구조화된 에러 객체

### 타입 안전성

- **`any` 사용 금지** — `unknown` + 타입 가드 또는 Zod 스키마 사용
- JSON.parse → 반드시 Zod 검증
- 백엔드 응답 → 타입 가드로 검증

### 에이전트 아키텍처

- 각 세션마다 고립된 `MCPServiceProxy` 할당
- 세션별 `HttpSessionManager` / `SessionMCPManager`
- Rust 백엔드가 Think-Act-Observe 루프 전담 (프론트엔드는 수동적 반응형)

---

## Security Notes

- Tauri allowlist + capability 시스템 사용 (CSP 불필요)
- **CSP를 `tauri.conf.json`에 추가하지 마세요** — 릴리즈 빌드에서 화면이 흐려집니다
- 모든 프론트엔드→백엔드 입력 검증
- MCP 서버 통신: 프로토콜 검증 + 샌드박스 실행

---

## Related

- [문제 해결 (개발자용)](./troubleshooting-dev.md)
- [네비게이션 가이드 (개발자용)](./navigation-guide-dev.md)
- [프로젝트 가이드 (agents.md)](../../agents.md)
