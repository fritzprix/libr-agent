# Release 빌드 이슈 해결 가이드

## 📋 발견된 문제들과 해결책

### 1. ❌ 문제: "Could not connect to localhost: Connection refused"

#### 원인

- `cargo build --release`로 빌드 시 프론트엔드가 번들에 포함되지 않음
- 바이너리가 실행될 때 정적 파일을 찾을 수 없어 localhost dev 서버에 연결을 시도함

#### ✅ 해결책

**올바른 빌드 명령 사용:**

```bash
# ❌ 잘못된 방법
cd src-tauri && cargo build --release

# ✅ 올바른 방법
pnpm tauri build
```

---

### 2. ❌ 문제: Release 빌드에서 하얀 화면만 표시됨

#### 원인 1: 세션 컨텍스트 초기화 타이밍 이슈

```
[webview][ERROR] No active session context.
Call switch_context with a sessionId before invoking this tool.
```

**문제점:**

- 앱 시작 시 `ResourceAttachmentContext`가 즉시 `listContent`를 호출
- 하지만 백엔드의 세션 컨텍스트는 아직 초기화되지 않은 상태
- 초기 MCP 호출 실패로 UI가 제대로 렌더링되지 않음

**수정 내용:**

1. `src/hooks/use-rust-mcp-server.ts`에서 세션 컨텍스트 에러를 WARN으로 다운그레이드:

```typescript
if (response.error) {
  // Session context errors are recoverable during initialization
  const isSessionContextError = response.error.code === -32002;

  if (isSessionContextError) {
    logger.warn(
      'Proxy: Tool execution failed (recoverable session context error)',
      {
        serverName: resolvedServerName,
        methodName,
        error: response.error,
        note: 'This error is expected during initialization and will be retried',
      },
    );
  } else {
    logger.error('Proxy: Tool execution failed', {
      serverName: resolvedServerName,
      methodName,
      error: response.error,
    });
  }

  throw new Error(
    `MCP tool execution failed: ${methodName} - ${response.error.message} (code: ${response.error.code})`,
  );
}
```

2. `src/context/ResourceAttachmentContext.tsx`에 에러 핸들링 및 자동 재시도 로직 추가:

```typescript
const { data: sessionFiles = [], mutate } = useSWR(
  currentSession?.id ? `session-files-${currentSession.id}` : null,
  async (key: string) => {
    const sessionId = key.replace('session-files-', '');
    if (sessionId && server) {
      try {
        // ... listContent 호출
        return files;
      } catch (error) {
        // 세션 컨텍스트가 준비되지 않은 경우 경고만 로그하고 빈 배열 반환
        logger.warn(
          'Session context not ready yet, will retry on next revalidation',
          { sessionId, error },
        );
        return [];
      }
    }
    return [];
  },
  {
    revalidateOnFocus: false,
    revalidateOnReconnect: true,
    dedupingInterval: 5000,
    shouldRetryOnError: true, // 에러 시 재시도
    errorRetryCount: 3, // 최대 3회 재시도
    errorRetryInterval: 1000, // 1초 간격으로 재시도
  },
);
```

**결과:**

- 앱 시작 시 ERROR 대신 WARN 로그만 출력됨
- SWR이 자동으로 재시도하여 세션이 준비되면 정상 작동
- UI 렌더링 차단이 해결되어 하얀 화면 문제 해결

#### 원인 2: 엄격한 CSP(Content Security Policy) 설정

**문제점:**

```json
{
  "security": {
    "csp": "default-src 'self'; connect-src ipc://localhost"
  }
}
```

- 너무 제한적인 CSP로 인해 인라인 스크립트/스타일 차단
- React 앱의 동적 콘텐츠 로딩 실패

**수정 내용:**
`src-tauri/tauri.conf.json`:

```json
{
  "security": {
    "csp": "default-src 'self' 'unsafe-inline' 'unsafe-eval' data: blob:; connect-src ipc://localhost https: wss:; img-src 'self' data: https:; style-src 'self' 'unsafe-inline'; font-src 'self' data:; script-src 'self' 'unsafe-inline' 'unsafe-eval'"
  }
}
```

#### 원인 3: WebView DevTools 비활성화

**문제점:**

- Release 빌드에서 개발자 도구가 비활성화되어 디버깅 불가능
- 프론트엔드 에러를 확인할 방법이 없음

**수정 내용:**
`src-tauri/tauri.conf.json`에 `devtools` 추가:

```json
{
  "app": {
    "windows": [
      {
        "title": "SynapticFlow",
        // ... 기타 설정
        "devtools": true // ← Release 빌드에서도 DevTools 활성화
      }
    ]
  }
}
```

---

## 🔧 전체 수정 사항

### 1. `src-tauri/tauri.conf.json`

```json
{
  "app": {
    "windows": [
      {
        "devtools": true // 추가
      }
    ],
    "security": {
      "csp": "default-src 'self' 'unsafe-inline' 'unsafe-eval' data: blob:; connect-src ipc://localhost https: wss:; img-src 'self' data: https:; style-src 'self' 'unsafe-inline'; font-src 'self' data:; script-src 'self' 'unsafe-inline' 'unsafe-eval'" // 수정
    }
  }
}
```

### 2. `src/context/ResourceAttachmentContext.tsx`

- 세션 컨텍스트 초기화 전 MCP 호출 시 에러 핸들링 추가
- SWR 재시도 로직 구성 (최대 3회, 1초 간격)

---

## 🚀 Release 빌드 체크리스트

### 빌드 전

- [ ] 최신 코드로 업데이트: `git pull`
- [ ] 의존성 설치: `pnpm install`
- [ ] 프론트엔드 빌드 테스트: `pnpm build`
- [ ] TypeScript 컴파일 확인: `pnpm tsc`

### 빌드 실행

- [ ] 올바른 명령 사용: `pnpm tauri build` (NOT `cargo build`)
- [ ] 빌드 로그 확인: 에러 없이 완료되는지 확인
- [ ] 생성된 패키지 확인:
  - Linux: `.deb`, `.rpm`, `.AppImage`
  - Windows: `.msi`, `.exe`
  - macOS: `.dmg`, `.app.tar.gz`

### 빌드 후 테스트

- [ ] 번들 파일 실행 테스트
- [ ] 로그 확인: `RUST_LOG=debug ./your-app`
- [ ] 주요 기능 확인:
  - [ ] 앱 시작 (하얀 화면 없음)
  - [ ] 세션 생성/전환
  - [ ] MCP 도구 사용
  - [ ] 파일 업로드/첨부
  - [ ] 채팅 기능

### DevTools로 디버깅

1. 앱 실행 후 `F12` 또는 `Ctrl+Shift+I` (Linux/Windows) / `Cmd+Option+I` (macOS)
2. Console 탭에서 에러 확인
3. Network 탭에서 실패한 요청 확인
4. Sources 탭에서 소스맵 디버깅

---

## 📊 일반적인 Release 빌드 문제 해결

### 문제: "Failed to load resource" 에러

**원인**: CSP 설정이 너무 제한적
**해결**: CSP에 필요한 소스 추가

### 문제: "Permission denied" 에러

**원인**: 파일 시스템 권한 문제
**해결**: `assetProtocol.scope` 확인

### 문제: 앱이 즉시 종료됨

**원인**: Panic 또는 초기화 실패
**해결**:

```bash
RUST_BACKTRACE=full RUST_LOG=trace ./your-app 2>&1 | tee app.log
```

### 문제: 특정 기능만 작동 안함

**원인**: 환경 변수 누락
**해결**:

- Dev: `.env` 파일 사용
- Release: 시스템 환경 변수 설정

---

## 🎯 결론

Release 빌드 문제의 주요 원인:

1. **잘못된 빌드 방식** - `cargo build` 대신 `tauri build` 사용
2. **초기화 타이밍** - 세션 컨텍스트가 준비되기 전 API 호출
3. **제한적인 CSP** - React 앱에 필요한 권한 부족
4. **디버깅 불가** - DevTools 비활성화

**핵심 교훈:**

- 항상 `pnpm tauri build`를 사용하여 완전한 앱 빌드
- 초기화 순서와 타이밍에 주의
- Release 빌드에서도 DevTools 활성화하여 디버깅 가능하게 유지
- CSP는 보안과 기능의 균형을 맞춰야 함
