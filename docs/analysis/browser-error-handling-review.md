# Browser Error Handling Review: String Matching Analysis

## 검토 일시

2025-12-12

## 검토 대상

`src/features/tools/browser-tools/error-utils.ts`의 `handleBrowserError` 함수에서 사용하는 문자열 매칭 기반 에러 처리

## 1. 실제 에러 메시지 검증

### ✅ 검증된 에러 메시지

#### 1.1 세션 관련 에러

**소스**: `src-tauri/src/services/interactive_browser_server.rs`

```rust
// Line 188, 597, 644
.ok_or("Session not found")?

// Line 621
Ok("Session closed successfully".to_string())
```

**매칭 패턴**:

- ✅ `"session not found"` - 실제 백엔드에서 사용됨
- ❌ `"session closed"` - 실제로는 "Session closed successfully" (성공 메시지)
- ❌ `"invalid session"` - 백엔드에서 사용되지 않음

**문제점**:

1. "session closed"는 에러가 아니라 성공 메시지를 잘못 감지할 수 있음
2. "invalid session"은 실제로 발생하지 않는 가상의 에러

#### 1.2 셀렉터 관련 에러

**소스**: JavaScript 실행 결과 (interactive_browser_server.rs Line 260-269, 353-362)

```rust
// 엘리먼트를 찾지 못했을 때 반환되는 JSON
return JSON.stringify({
  ok: false,
  action: 'click',
  reason: 'not_found',
  selector: selector,
  timestamp: ts
});
```

**매칭 패턴**:

- ❌ `"element not found"` - 백엔드에서 이 문자열을 직접 사용하지 않음
- ❌ `"selector"` - 너무 일반적이어서 오탐 가능

**실제 동작**:

- 백엔드는 구조화된 JSON을 반환
- TypeScript에서 이를 파싱하여 처리해야 하지만, 현재는 문자열 매칭만 함

#### 1.3 네비게이션/네트워크 에러

**검증 결과**: ❌ 명시적인 에러 메시지 없음

**소스**: `src-tauri/src/services/interactive_browser_server.rs` Line 672

```rust
Err("Browser window not found".to_string())
```

**매칭 패턴**:

- ❌ `"navigation"` - 백엔드에서 사용하지 않음
- ❌ `"timeout"` - 백엔드에서 사용하지 않음
- ❌ `"network"` - 백엔드에서 사용하지 않음

**문제점**:

- 실제 네비게이션 에러는 Tauri/Webview 레벨에서 발생하며 명시적인 에러 메시지가 없음
- 타임아웃 관련 에러는 별도로 처리되지 않음

#### 1.4 콘텐츠 관련 에러

**소스**: `src-tauri/src/mcp/builtin/content_store/storage.rs` Line 475

```rust
return Err("No content found in specified line range".to_string());
```

**매칭 패턴**:

- ✅ `"no content found"` - 일부 사용됨 (하지만 브라우저 도구가 아닌 content_store에서)

**문제점**:

- 이 에러는 브라우저 도구가 아닌 content_store에서 발생
- 브라우저 도구에서는 실제로 발생하지 않을 수 있음

## 2. String Matching 방식의 위험성

### 2.1 취약점 분석

#### ❌ 1. 대소문자 의존성

```typescript
errorMessage.toLowerCase().includes('session not found');
```

- 백엔드에서 "Session Not Found"로 변경되면 감지 실패
- 일관성 없는 에러 메시지 형식에 취약

#### ❌ 2. 오탐(False Positive) 가능성

```typescript
// 예: 사용자가 입력한 URL에 "session not found"가 포함된 경우
navigateToUrl(sessionId, 'https://example.com/error?msg=session%20not%20found');
// → 정상 실행이지만 에러로 잘못 분류될 수 있음
```

#### ❌ 3. 부분 문자열 매칭의 위험

```typescript
errorMessage.toLowerCase().includes('selector');
```

- "Invalid CSS selector format" → 감지됨
- "Selector engine initialization failed" → 감지됨
- "The selector you provided is too complex" → 감지됨
- 모든 경우에 동일한 가이던스 제공 (부적절할 수 있음)

#### ❌ 4. 다국어/현지화 불가능

- 에러 메시지가 한국어나 다른 언어로 변경되면 매칭 실패
- 국제화(i18n) 불가능

#### ❌ 5. 백엔드 변경에 취약

```rust
// 백엔드에서 에러 메시지 개선
- .ok_or("Session not found")?
+ .ok_or("Browser session with ID '{session_id}' could not be found")?
```

→ 프론트엔드의 에러 처리 로직이 깨짐

### 2.2 실제 위험 시나리오

#### 시나리오 1: 성공 메시지를 에러로 오인

```typescript
// 백엔드에서 반환
Ok('Session closed successfully'.to_string());

// handleBrowserError에서
if (errorMessage.toLowerCase().includes('session closed')) {
  // 성공 메시지를 에러로 처리!
  guidance = 'The browser session might have been closed...';
}
```

#### 시나리오 2: 구조화된 에러 무시

```rust
// 백엔드에서 상세한 JSON 에러 반환
return JSON.stringify({
  ok: false,
  action: 'click',
  reason: 'not_found',
  selector: selector,
  diagnostics: {
    visible: false,
    disabled: true,
    // ... 더 많은 진단 정보
  }
});

// 프론트엔드에서는 단순 문자열 매칭만 수행
// → 진단 정보를 활용하지 못함
```

## 3. 권장 개선 방안

### 3.1 구조화된 에러 타입 사용

#### 백엔드 (Rust)

```rust
#[derive(Debug, Serialize, Deserialize)]
pub enum BrowserError {
    SessionNotFound { session_id: String },
    SessionClosed { session_id: String },
    ElementNotFound { selector: String, diagnostics: ElementDiagnostics },
    NavigationFailed { url: String, reason: String },
    NetworkError { details: String },
    Timeout { operation: String, duration_ms: u64 },
}

impl BrowserError {
    pub fn to_error_code(&self) -> &'static str {
        match self {
            BrowserError::SessionNotFound { .. } => "SESSION_NOT_FOUND",
            BrowserError::SessionClosed { .. } => "SESSION_CLOSED",
            BrowserError::ElementNotFound { .. } => "ELEMENT_NOT_FOUND",
            BrowserError::NavigationFailed { .. } => "NAVIGATION_FAILED",
            BrowserError::NetworkError { .. } => "NETWORK_ERROR",
            BrowserError::Timeout { .. } => "TIMEOUT",
        }
    }
}
```

#### 프론트엔드 (TypeScript)

```typescript
interface BrowserErrorResponse {
  code:
    | 'SESSION_NOT_FOUND'
    | 'SESSION_CLOSED'
    | 'ELEMENT_NOT_FOUND'
    | 'NAVIGATION_FAILED'
    | 'NETWORK_ERROR'
    | 'TIMEOUT';
  message: string;
  context?: Record<string, unknown>;
}

export function handleBrowserError(
  error: unknown,
  context: { toolName: string; sessionId?: string; selector?: string },
): MCPResponse<unknown> {
  // Try to parse structured error
  let errorCode: string | undefined;
  let errorMessage: string;

  try {
    const parsed = JSON.parse(String(error)) as BrowserErrorResponse;
    errorCode = parsed.code;
    errorMessage = parsed.message;
  } catch {
    // Fallback to string error
    errorMessage = error instanceof Error ? error.message : String(error);
  }

  logger.error(`Error in ${context.toolName}`, { error, context, errorCode });

  let guidance = '';

  switch (errorCode) {
    case 'SESSION_NOT_FOUND':
    case 'SESSION_CLOSED':
      guidance = 'The browser session might have been closed...';
      break;
    case 'ELEMENT_NOT_FOUND':
      guidance = `The element with selector "${context.selector}"...`;
      break;
    // ... 다른 케이스들
    default:
      // Fallback to string matching (backward compatibility)
      guidance = getGuidanceFromStringMatching(errorMessage, context);
  }

  return createMCPErrorResponse(
    `✗ ${context.toolName} failed: ${errorMessage}\n\nGuidance: ${guidance}`,
  );
}
```

### 3.2 에러 코드 기반 처리 (단기 해결책)

백엔드를 변경할 수 없는 경우:

```typescript
export function handleBrowserError(
  error: unknown,
  context: { toolName: string; sessionId?: string; selector?: string },
): MCPResponse<unknown> {
  const errorMessage = error instanceof Error ? error.message : String(error);
  const { toolName, selector } = context;

  logger.error(`Error in ${toolName}`, { error, context });

  let guidance = '';

  // Exact string matching (더 안전함)
  if (errorMessage === 'Session not found') {
    guidance = 'The browser session does not exist...';
  } else if (errorMessage === 'Browser window not found') {
    guidance = 'The browser window was closed...';
  } else if (errorMessage.startsWith('No content found')) {
    guidance = 'No content was extracted...';
  }
  // Contains matching은 최후의 수단으로만 사용
  else if (selector && errorMessage.includes('not_found')) {
    guidance = `The element with selector "${selector}"...`;
  } else {
    guidance = 'Please check the tool parameters and try again.';
  }

  return createMCPErrorResponse(
    `✗ ${toolName} failed: ${errorMessage}\n\nGuidance: ${guidance}`,
  );
}
```

### 3.3 타입 안전성 개선

```typescript
// 정확한 에러 메시지 상수 정의
const KNOWN_ERRORS = {
  SESSION_NOT_FOUND: 'Session not found',
  BROWSER_WINDOW_NOT_FOUND: 'Browser window not found',
  // ... 백엔드와 동기화된 에러 메시지
} as const;

export function handleBrowserError(/* ... */) {
  // 정확한 매칭
  if (errorMessage === KNOWN_ERRORS.SESSION_NOT_FOUND) {
    // ...
  }
}
```

## 4. 즉시 수정이 필요한 부분

### 4.1 잘못된 패턴 제거

```typescript
// ❌ 제거할 부분
else if (
  errorMessage.toLowerCase().includes('session closed') ||  // 성공 메시지를 에러로 오인
  errorMessage.toLowerCase().includes('invalid session')    // 실제로 사용되지 않음
) {
  // ...
}
```

### 4.2 개선된 패턴

```typescript
// ✅ 개선안
// 정확한 매칭만 사용
if (errorMessage === 'Session not found') {
  guidance =
    'The browser session does not exist. Please use `listSessions` to verify active sessions or `createSession` to start a new one.';
}
// 구조화된 응답 파싱
else if (errorMessage.includes('"reason":"not_found"')) {
  try {
    const parsed = JSON.parse(errorMessage);
    if (parsed.reason === 'not_found') {
      guidance = `The element with selector "${parsed.selector}" could not be found...`;
    }
  } catch {
    // Fallback
  }
}
```

## 5. 결론 및 권장사항

### 즉시 조치 필요 ⚠️

1. **"session closed" 패턴 제거** - 성공 메시지를 에러로 오인
2. **"invalid session" 패턴 제거** - 실제로 사용되지 않음
3. **정확한 문자열 매칭 사용** - `===` 연산자 사용

### 단기 개선 (1-2주)

1. **에러 메시지 상수화** - 백엔드와 프론트엔드 동기화
2. **구조화된 JSON 응답 파싱** - 진단 정보 활용
3. **테스트 케이스 추가** - 각 에러 시나리오별 단위 테스트

### 장기 개선 (1-2개월)

1. **타입 안전한 에러 처리 시스템** - Rust enum ↔ TypeScript interface
2. **에러 코드 기반 처리** - 문자열 매칭 완전 제거
3. **국제화 지원** - 에러 메시지 다국어 지원

## 6. 참고 자료

- 백엔드 에러 소스: `src-tauri/src/services/interactive_browser_server.rs`
- 프론트엔드 에러 처리: `src/features/tools/browser-tools/error-utils.ts`
- 관련 이슈: 문자열 매칭의 취약성과 구조화된 에러 처리 필요성
