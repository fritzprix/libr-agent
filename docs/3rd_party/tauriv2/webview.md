# Tauri v2 Webview API와 Multi Window 활용 정리

## 핵심 요약

- **Webview API**: Tauri v2에서는 Webview와 WebviewWindow 클래스를 통해 웹뷰 생성·제어가 가능하며, 다양한 창 관리 기능을 지원합니다.
- **Multiwindow**: 여러 개의 webview 창(윈도우)을 label로 생성·제어하는 공식적 방법을 제공함.
- **실제 코드 예제**까지 함께 최신 문서(2025 기준)로 제공.

---

## Webview API 주요 내용과 실전 예제

### Webview 생성 & 제어

```typescript
import { Webview } from '@tauri-apps/api/webview';
import { appWindow } from '@tauri-apps/api/window';

// Webview 생성 예시
const webview = new Webview(appWindow, 'my-webview', {
  url: 'https://example.com',
  width: 800,
  height: 600,
  x: 100,
  y: 100,
});
await webview.show();
```

#### 주요 메서드

- `show()`: 웹뷰 표시
- `hide()`: 웹뷰 숨기기
- `close()`: 웹뷰 종료
- `clearAllBrowsingData()`: 웹뷰의 브라우저 데이터 삭제
- `setSize(size)`, `setPosition(pos)`: 위치/크기 지정 (Object형/px 단위)
- `setFocus()`: 포커스 제어
- `setBackgroundColor(color)`: 배경색 지정
- `setZoom(scaleFactor)`: 확대/축소
- `listen(event, handler)`: 이벤트 수신 리스너
- `emit(event, payload)`: 이벤트 발생

#### 이벤트 예제

```typescript
webview.listen('custom-event', (payload) => {
  console.log(payload);
});
webview.emit('custom-event', { foo: 'bar' });
```

---

## Multiwindow(다중창) 활용 예제

### 1. WebviewWindow를 이용한 다중창 생성

```typescript
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

// 새로운 창 생성
const win = new WebviewWindow('external-agent-window', {
  title: 'AI Agent 창',
  url: 'https://your-url-or-local.html',
  width: 400,
  height: 600,
  resizable: true,
  decorations: true,
});

// 창 생성 이벤트 handling
win.once('tauri://created', () => {
  console.log('WebviewWindow 창 생성 완료');
});
win.once('tauri://error', (e) => {
  console.error('창 생성 에러', e);
});
```

### 2. 기존 window 접근 및 제어

```typescript
const myWin = await WebviewWindow.getByLabel('external-agent-window');
if (myWin) {
  await myWin.show();
  await myWin.setFocus();
}
```

### 3. 모든 Window 목록 출력

```typescript
import { getAllWebviewWindows } from '@tauri-apps/api/webviewWindow';

const winList = getAllWebviewWindows();
winList.forEach((win) => {
  console.log('Window label:', win.label);
});
```

### 4. 창 관리 메서드

다양한 창 제어 메서드 제공 (상속 포함):

- `maximize()`, `minimize()`, `unmaximize()`, `unminimize()`
- `setPosition()`, `setSize()`
- `setAlwaysOnTop(bool)`, `setAlwaysOnBottom(bool)`
- `setTitle(string)`
- `destroy()`, `close()`
- `listen(event, handler)`, `once(event, handler)` (윈도우 이벤트 리스너)
- `requestUserAttention()` (알림 요청, 플랫폼별 지원)

---

## 창별 capability/권한 분리 및 고급 설정

- 각 Window별 capability (.json) 파일로 권한/기능 제한 가능.
- 창별 파일 접근, 알림, 시스템 API 등 granular하게 제어 가능.

---

## 참고

- Webview와 WebviewWindow 모두 label 기반으로 창·웹뷰를 분기 관리
- 이벤트 중심(message passing 아키텍처)으로 창 간 통신 가능
- Tauri v2에서는 모든 Webview/Window API가 TypeScript/Javascript와 Rust 양쪽에서 접근/제어됨[1][2]

---

## 실전 개발 팁

- **여러 창 독립 제어**: 각 창에 별도 웹뷰를 띄우고 Agent나 MCP로 연결 가능
- **Vue/React 등 프론트 구성과 병행 가능** (단, Webview는 DOM에 직접 렌더링은 불가능, native overlay 방식임)
- **권한 분리, 이벤트 처리 등으로 강력한 UI/자동화 시스템 구축 가능**

---

## 대표 코드 샘플

```typescript
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

// 새 창 생성 및 이벤트 처리
const win = new WebviewWindow('my-second-window', {
  url: 'index.html',
  width: 600,
  height: 400,
  resizable: true,
});
win.once('tauri://created', () => console.log('생성됨'));
win.once('tauri://error', (err) => console.error('에러발생', err));

// 모든 창 리스트 확인
import { getAllWebviewWindows } from '@tauri-apps/api/webviewWindow';
const winList = getAllWebviewWindows();
winList.forEach((win) => console.log(win.label));
```

---

## 결론

Tauri v2 공식 문서 기준으로 다양한 Webview 옵션과 Multiwindow 기능이 제공되며, 실전 코드 예제와 함께 자유로운 창·웹뷰 관리가 가능합니다. 고급 이벤트, 창 제어, 권한 분리까지 전방위적인 데스크톱 애플리케이션 자동화와 Agent 인터페이스 개발에 활용할 수 있습니다.[2][1]

[1] <https://v2.tauri.app/reference/javascript/api/namespacewebview/>
[2] <https://v2.tauri.app/reference/javascript/api/namespacewebviewwindow/>
[3] <https://stackoverflow.com/questions/77775315/how-to-create-mulitwindows-in-tauri-rust-react-typescript-html-css>
[4] <https://v2.tauri.app/reference/javascript/api/namespacewindow/>
[5] <https://tauri.app/ko/v1/guides/features/multiwindow/>
[6] <https://classic.yarnpkg.com/en/package/@tauri-apps/api>
[7] <https://stackoverflow.com/questions/79602787/how-to-properly-embed-a-webview-inside-a-vue-component-in-a-tauri-2-vite-appli>
[8] <https://v2.tauri.app/learn/security/capabilities-for-windows-and-platforms/>
[9] <https://v2.tauri.app/reference/javascript/api/>
[10] <https://v2.tauri.app/concept/architecture/>
[11] <https://v2.tauri.app/blog/tauri-20/>
