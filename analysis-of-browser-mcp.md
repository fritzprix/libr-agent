# Browser Agent 도구 문제점 분석

## 현재 이슈

- WebView (Vite) -> invoke를 통해 Tauri 2.0 Command 호출
  - `create_browser_session`: 새로운 browser session을 생성하여 페이지로 이동
  - `get_page_content`: page를 가져오기 위해서 get_page_content 호출

- interactive_browser_server.rs에서 처리
  - `create_browser_session`: 새로운 WebViewWindow를 생성하고 session을 등록
  - `execute_script`: JS 코드를 WebView에 주입하고, 결과를 oneshot 채널로 받음

- WebView (JS) -> invoke를 통해 Script 실행 결과 보내기
  - `browser_script_result` (Rust command, browser_commands.rs): JS에서 window.**TAURI**.core.invoke로 Rust에 결과 전달
  - `handle_script_result` (interactive_browser_server.rs): result_waiters에서 채널을 찾아 결과를 전달

### 문제 현상

- `execute_script`가 rx.await에서 block되어 있음
- Tauri Command는 single threaded event loop에서 실행되므로 `browser_script_result`는 queue에서 대기상태가 되고
- 결국 `execute_script`는 결과를 받지 못해 timeout 됨
- timeout 이후 `browser_script_result`가 실행되고 이때 이미 채널이 사라진 상태가 됨

---

## 상세 동작 흐름 (코드 기반 분석)

### 1. Command 호출 및 세션 관리

- `create_browser_session` (browser_commands.rs, browser_agent_server.rs)
  - Rust에서 새로운 브라우저 세션을 생성 (`InteractiveBrowserServer::create_browser_session`)
  - WebViewWindowBuilder로 새로운 창 생성, session 등록

- `get_page_content` (browser_commands.rs, browser_agent_server.rs)
  - Rust에서 JS 코드 (`document.documentElement.outerHTML`)를 실행하도록 요청
  - `execute_script`를 통해 WebView에 JS 주입

### 2. JS 실행 및 결과 전달

- `execute_script` (interactive_browser_server.rs)
  - oneshot 채널 생성 후 result_waiters에 등록
  - JS 코드 주입: window.**TAURI**.core.invoke('browser_script_result', { payload })
  - Rust에서 eval 후, 채널에서 결과를 기다림 (timeout 5초)

- JS에서 실행 결과를 Rust로 전달
  - JS 내부에서 window.**TAURI**.core.invoke('browser_script_result', { payload }) 호출
  - Rust의 `browser_script_result` command가 호출됨 (browser_commands.rs)
  - `handle_script_result`에서 result_waiters에서 채널을 찾아 결과를 전달

### 3. MCP Tool 호출 (browser_agent_server.rs)

- MCP 서버에서 각 브라우저 관련 기능을 tool로 래핑
  - 예: `handle_extract_data`, `handle_crawl_current_page`, `handle_click_element` 등
  - 내부적으로 모두 InteractiveBrowserServer의 메서드 호출
  - 결과를 받아 파일 저장, JSON 변환, 상대경로 처리 등 추가 로직 수행

---

## 추가 분석

### 1. 이벤트 루프/동기화 문제

- 현재 구조는 Rust에서 JS 실행 결과를 oneshot 채널로 기다림
- Tauri의 Command는 single-threaded event loop에서 실행되므로, JS에서 invoke된 Rust Command(`browser_script_result`)가 바로 실행되지 못하고 queue에서 대기
- 결과적으로 Rust 쪽에서 timeout 발생 → JS에서 결과가 도착해도 이미 채널이 닫혀있음

---

## 개선 시도 및 결과

### 효과가 없었던 시도들

- **Recursive Tauri Commands**  
  `execute_script()` → `window.eval()` → `invoke('browser_script_result')`  
  → Deadlock 발생: 서로의 응답을 기다림

- **Event-based Communication**  
  `window.__TAURI__.event.emit('script_result', payload)`  
  → 동적으로 생성된 윈도우에서는 event emit이 허용되지 않아 실패

- **Fire-and-forget Invoke**  
  `window.__TAURI__.core.invoke('handle_script_result', payload)`  
  → 같은 event loop에서 처리되어 deadlock 발생

- **Polling with DOM Storage**  
  JS에서 결과를 DOM에 저장하고 Rust가 polling  
  → 결국 Rust가 값을 가져오려면 eval/invoke를 사용해야 하므로 deadlock 구조 반복

- **HTTP Server Approach**  
  Tauri 앱 내부에 HTTP 서버를 두고 JS에서 fetch로 결과 전송  
  → WebView의 CSP/CORS 정책으로 인해 fetch 요청이 차단되어 실패

---

## 새로운 접근 방법

### Phase 1 - BuiltInToolContext.tsx의 개선

- BuiltInToolContext.tsx에 Rust Backend에서 제공되는 BuiltIn MCP 뿐 아니라 일종의 Hook을 추가하여 도구를 확장할 수 있도록 변경
- 기본적으로 Interface는 기존 BuiltIn MCP와 호환되도록 해야 함

### Phase 2 - Rust Backend의 수정 (Block Free, Polling 방식)

- 기존에 Tauri Command 실행을 Block하던 부분을 제거하고 결과를 확인하기 위한 poll_result같은 command를 추가
- handle_script_result를 통해 들어온 결과를 poll_result를 통해 가져갈 수 있도록 수정
- session id 외 추가로 request의 polling을 위한 고유 값이 필요

### Phase 3 - 이러한 Browser Invoke를 추상화할 Hook

### Phase 4 - Phase 3의 구현된 Hook을 MCP 표준화된 방식으로 Provision하기 위한 Wrapper Hook

참고: 코드의 복잡도에 따라 Phase 3, 4를 병합해도됨

관련 코드들

- BuitlInToolContext.tsx
- tauri-mcp-client.ts
- interactive_browser_server.rs
- browser_commands.rs
- lib.rs
- main.rs
- mcp.rs
