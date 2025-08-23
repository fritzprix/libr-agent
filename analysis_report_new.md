# Tauri Single-Threaded Event Loop Deadlock Analysis Report

## 문제 개요

Tauri의 **single-threaded event loop** 구조로 인해 Interactive Browser Server (IBS)의 `execute_script` 기능에서 근본적인 deadlock 문제가 발생하고 있습니다. 이는 JavaScript 실행 결과를 Rust로 가져오는 모든 시도에서 회피 불가능한 구조적 문제입니다.

## 근본 원인 분석

### 1. Tauri의 Event Loop 구조
- Tauri는 **single-threaded event loop**을 사용
- WebView의 JavaScript 실행과 Rust command 처리가 **같은 스레드**에서 발생
- 한 번에 **하나의 작업만** 처리 가능

### 2. Deadlock 발생 메커니즘
```
[Rust] execute_script 호출
   ↓
[Rust] window.eval(script) 실행
   ↓
[JS] 스크립트 실행 후 결과를 Rust로 전송 시도
   ↓
[JS] invoke('command') 또는 event.emit() 호출
   ↓
[Event Loop] 이미 execute_script가 실행 중이므로 대기
   ↓
[Rust] execute_script는 JS 결과를 기다리며 대기
   ↓
💀 DEADLOCK: 서로를 기다리는 상황
```

## 시도했던 해결 방법들과 실패 원인

### 1. ❌ Recursive Tauri Commands
```rust
execute_script() -> window.eval() -> invoke('browser_script_result')
```
**실패 원인**: `browser_script_result` 명령이 `execute_script`가 끝나기를 기다리지만, `execute_script`는 `browser_script_result`의 응답을 기다림

### 2. ❌ Event-based Communication
```javascript
window.__TAURI__.event.emit('script_result', payload)
```
**실패 원인**: 
- Event emission이 특정 윈도우에서만 허용됨 (`event.emit not allowed on window`)
- Browser 윈도우는 동적으로 생성되어 허용 목록에 없음

### 3. ❌ Fire-and-forget Invoke
```javascript
window.__TAURI__.core.invoke('handle_script_result', payload).catch(...)
```
**실패 원인**: `invoke`를 사용하는 한 결국 같은 event loop에서 처리되어 deadlock 발생

### 4. ❌ Polling with DOM Storage
```javascript
window._tauri_result_123 = { result: '...', completed: true }
```
**실패 원인**: DOM에 저장된 값을 읽으려면 결국 `eval` → `invoke`를 통해 Rust로 가져와야 함

## 현재 상황의 근본적 한계

### Tauri의 구조적 제약사항
1. **Single-threaded Event Loop**: 모든 UI 작업과 command 처리가 하나의 스레드
2. **Security Restrictions**: 동적으로 생성된 윈도우에서 event emission 불가
3. **IPC Limitations**: JavaScript에서 Rust로 데이터를 전송하는 모든 방법이 event loop를 통함

### 불가능한 우회 방법들
- ✗ Multi-threading: Tauri의 WebView는 main thread에서만 조작 가능
- ✗ Async/await: JavaScript의 async 작업도 결국 같은 event loop 사용
- ✗ Worker Threads: Web Workers는 Tauri API 접근 불가
- ✗ External HTTP: CORS 및 보안 제약

## 대안적 해결 방안

### 1. 🔄 External Process Communication
별도의 프로세스를 통한 브라우저 제어:
```rust
// Puppeteer, Playwright, 또는 Selenium 같은 외부 도구 사용
// 장점: Tauri event loop와 독립적
// 단점: 추가 의존성, 복잡성 증가
```

### 2. 🔄 File System IPC
파일을 통한 JavaScript-Rust 통신:
```javascript
// JavaScript가 임시 파일에 결과 저장
// Rust가 file watcher로 변경 감지
```

### 3. 🔄 HTTP Server Approach
내장 HTTP 서버를 통한 통신:
```rust
// 로컬 HTTP 서버 (예: localhost:random_port)
// JavaScript가 fetch로 결과 POST
// 장점: 완전히 독립적인 통신 채널
```

### 4. 🔄 Native Browser Integration
시스템 브라우저와의 직접 통합:
```rust
// Chrome DevTools Protocol 사용
// WebDriver 프로토콜 사용
```

## 권장 해결책

### 단기적 해결책: HTTP Server 방식
1. Tauri 앱 내부에 작은 HTTP 서버 구동
2. JavaScript에서 `fetch()`를 사용해 결과 전송
3. Tauri event loop와 완전히 분리된 통신 채널

### 장기적 해결책: 외부 브라우저 도구 사용
1. Puppeteer 또는 Playwright 통합
2. 더 안정적이고 기능이 풍부한 브라우저 자동화
3. Tauri의 제약사항에서 완전히 자유로움

## 결론

현재의 timeout 문제는 **Tauri의 근본적인 아키텍처 한계**로 인한 것이며, JavaScript 실행 결과를 동기적으로 가져오는 것은 **구조적으로 불가능**합니다. 

해결을 위해서는:
1. **완전히 다른 통신 메커니즘** 도입 (HTTP, File System 등)
2. **외부 브라우저 자동화 도구** 사용
3. **비동기 처리 방식으로 전환** (결과를 즉시 받지 않는 방식)

이 중에서 **HTTP Server 방식**이 가장 현실적이고 구현 가능한 해결책으로 판단됩니다.