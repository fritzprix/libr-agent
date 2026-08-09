# Troubleshooting — Developer Guide

> LibrAgent 개발 중 발생할 수 있는 문제와 해결 방법. 일반 사용자는 [사용자 문제 해결](../user/guides/troubleshooting.md)을 참고하세요.

---

## Linux / WebKit

### WebKit 크래시 또는 화면 blank

**증상**: 앱 창이 blank이거나, 콘솔에 `webkit2gtk` 관련 오류

**원인**: WebKit 라이브러리 누수 또는 호환성 문제

**해결**:

1. 필수 패키지 설치:
   ```bash
   sudo apt-get install -y libwebkit2gtk-4.1-dev
   ```
2. 실제 데스크탑 세션에서 실행 (컨테이너/헤드리스 환경은 WebKit 렌더링 오류를 앱 코드만으로 고칠 수 없음)
3. 소프트웨어 렌더링 플래그 강제 사용 금지 — 렌더링 성능이 더 나빠질 수 있음

> **Source**: [`src-tauri/src/lib.rs`](../../src-tauri/src/lib.rs) (Lines 188–250)

---

## MCP Server Issues

### MCP 서버 연결 실패

**증상**: 앱에서 MCP 서버 연결 실패 보고

**원인**: 서버 프로세스 시작 실패 또는 구성 오류

**해결**:

1. **구성 확인**: UI에서 `command`와 `args`가 정확하고 OS `PATH`에서 실행 가능한지 확인
2. **포트 충돌**: HTTP/WebSocket 서버 사용 시 포트가 이미 사용 중인지 확인
3. **로그 확인**: 앱 로그에서 서버 프로세스 관련 에러 메시지 확인

> **Source**: [`src-tauri/src/mcp.rs`](../../src-tauri/src/mcp.rs) (Lines 200–250)

### MCP 서버 시작 타임아웃

**증상**: `npx` 기반 서버가 30초 내에 시작되지 않음

**해결**:

1. **Settings → Advanced → System & Performance → MCP Server Startup Timeout**
2. 기본값 30초에서 120초까지 조정 가능
3. 환경 변수 `LIBRAGENT_MCP_STARTUP_TIMEOUT_SECONDS`도 지원

> **Source**: [`src-tauri/src/config.rs`](../../src-tauri/src/config.rs), [`src/features/settings/components/SystemPerformanceSettings.tsx`](../../src/features/settings/components/SystemPerformanceSettings.tsx)

---

## Tool Call Issues

### 도구 호출 실패

**증상**: 에이전트가 도구를 호출하지만 에러 발생

**해결**:

1. **도구 스키마 검증**: `validate_tool_schema` 명령어 또는 도구 정의 확인
2. **AI 생성 인자 확인**: 메시지 내 `tool_calls` 객체에서 AI가 생성한 인자 확인
3. **도구 출력 확인**: `ToolOutputBubble`에서 에러 메시지 확인

> **Source**: [`src/features/agent/components/AgentToolCallDetails.tsx`](../../src/features/agent/components/AgentToolCallDetails.tsx)

---

## Type Mismatches

### 런타임 타입 불일치

**증상**: 프론트엔드에서 백엔드 데이터 처리 중 타입 관련 런타임 에러

**원인**: TypeScript 인터페이스와 Rust 구조체 필드명/타입 불일치

**해결**:

1. **정의 비교**: Rust 구조체와 대응 TypeScript 인터페이스의 필드명/타입 정밀 비교
2. **`types.md` 참조**: [`docs/api/types.md`](../api/types.md) — 표준 참조 문서
3. **Serde 호환성 확인**: Rust `#[serde(rename = "...")]` 어트리뷰트가 직렬화 필드명을 변경하는지 확인

> **Source**: [`src/lib/backend/messages.ts`](../../src/lib/backend/messages.ts)

---

## Session / Agent

### 세션이 응답을 안 함

**원인**:

- Rust 백엔드의 Think-Act-Observe 루프가 블록됨
- 하위 세션이 고착됨

**해결**:

1. `agent__checkSession(sessionId)`로 하위 세션 상태 확인
2. 고착 확인 시 `agent__stopSession(sessionId)` 후 재시도
3. 지속 시 Tauri 콘솔 로그 확인 (Rust stderr)

---

## Build Errors

### `pnpm build` 실패

1. TypeScript 에러 확인: `pnpm build 2>&1 | head -30`
2. ESLint 에러: `pnpm lint`
3. Vite 캐시 문제: `rm -rf node_modules/.vite && pnpm build`

### `cargo clippy` 실패

1. `cargo clippy -- -D warnings` — 경고도 에러로 처리
2. 새 코드 추가 시 `#[allow(...)]` 어트리뷰트 남용 금지

---

## Debugging Tips

### Tauri 콘솔 로그

```bash
# Tauri dev 모드에서 Rust stderr 확인
pnpm tauri dev 2>&1 | grep -i error
```

### 프론트엔드 디버깅

```bash
# Vite dev 서버에서 브라우저 DevTools 사용
pnpm dev
# 또는
pnpm tauri dev  # 데스크톱 앱 내 Chrome DevTools
```

### 세션 추적 파일

에이전트 세션의 `.trace.json` 파일은 에이전트 동작을 이해하는 데 사용됩니다. 분석에는 [trace-analyzer 스킬](https://github.com/fritzprix/libr-agent/.agents/skills/trace-analyzer/SKILL.md)을 사용하세요.

---

## Related

- [개발자 시작 가이드](./getting-started-dev.md)
- [프로젝트 가이드 (agents.md)](../../agents.md)
- [GitHub Discussions](https://github.com/fritzprix/libr-agent/discussions)
