# Windows 릴리스 실행 시 백색(White) 화면 문제 분석 요청서

## 1. 증상 (Symptoms)

Windows 환경에서 릴리스 빌드된 실행 파일(.exe)을 실행하면 앱 창이 뜨지만 내용이 전혀 렌더링되지 않고 순수한 흰색 화면만 표시됨. CPU 사용량은 낮고 프로세스는 종료되지 않음. Linux / macOS 빌드에서는 동일한 소스 기준 정상 렌더링.

## 2. 영향 (Impact)

- 초기 화면 자체가 나오지 않아 모든 기능(Agent, MCP Tool, 설정 UI) 접근 불가
- 문제 재현 환경에서 사용자 로그 수집 곤란 → 원인 가시성 낮음
- 출시 차질 및 QA 중단 상태

## 3. 재현 조건 (Reproduction)

1. `pnpm build` 후 `tauri build` 로 Windows용 릴리스 패키지 생성
2. 생성된 `.exe` 실행
3. 흰색 창만 표시 (개발모드 `pnpm tauri dev` 에서는 정상 동작)

## 4. 관련 참고 이슈

- <https://github.com/tauri-apps/tauri/issues/7118> 과 유사: WebView 초기 로딩 실패 / GPU / CSP / WebView2 Runtime 문제 가능성

## 5. 현재까지 1차 분석 요약

프론트엔드 초기 부트스트랩(JS 번들) 또는 Web Worker/WASM 로딩이 Windows 릴리스 환경에서 **조기 실패**하나 오류가 노출되지 않아 흰 화면 상태로 정지하는 것으로 판단. 주요 의심 포인트는 다음 3가지 축: (A) 리소스 접근 정책(CSP/경로) (B) WebView2 / GPU 환경 (C) Web Worker 초기화 경로 및 번들링 차이.

## 6. 우선순위별 의심 원인 목록

| 우선순위 | 원인                                                           | 분류          |
| -------- | -------------------------------------------------------------- | ------------- |
| P1       | Web Worker 번들 경로/생성 패턴 불일치 (`?worker` 사용 코드)    | 프론트/빌드   |
| P1       | CSP에 `blob:` / `worker-src` 미포함 → Worker/WASM 차단         | 보안 정책     |
| P2       | WebView2 Runtime 미설치(또는 구버전)로 초기 JS 실행 실패       | 플랫폼        |
| P2       | GPU 가속 드라이버 문제로 WebView 렌더링 Freeze                 | 플랫폼/그래픽 |
| P3       | `frontendDist` 경로(`../dist`) 해석 오류 혹은 빌드 산출물 손상 | 빌드 경로     |
| P3       | Logger 초기화 이전 실패 → 에러 삼켜짐 (로그 레벨 Info)         | 관측성        |
| P3       | WASM + top-level-await 플러그인 조합이 CSP 차단으로 중단       | 번들/CSP      |

## 7. 각 원인별 근거 (Evidence)

### 7.1 Web Worker 생성 패턴

- 파일: `src/context/WebMCPContext.tsx`, `src/lib/web-mcp/mcp-proxy.ts`
- Worker를 `?worker` 쿼리로 임포트 후 `new MCPWorker()` 형태로 사용. Vite 환경에서는 임포트 타입이 Worker **인스턴스 생성자** 또는 URL을 반환하는데 릴리스 번들 시 차이 발생 가능.
- Worker 초기화 실패 시 React 마운트 이전 런타임 에러 → 흰 화면 & 조용한 실패.

### 7.2 CSP 제한

- 파일: `src-tauri/tauri.conf.json` 의 `security.csp`
- 현재 `script-src` / `default-src` 에 `blob:` / `worker-src` 명시 없음. ES Module Worker, WASM, top-level-await 로더가 내부적으로 blob URL, eval-like 동작을 필요로 하는 경우 차단 가능.

### 7.3 WebView2 Runtime

- Windows 릴리스에서 WebView2 미설치 혹은 손상 시 흰 화면 빈 WebView 발생 사례 다수 보고.
- 개발자 PC에서는 dev 모드 정상 (Runtime 존재) vs QA/테스트 PC에서 문제 가능.

### 7.4 GPU 드라이버 / 하드웨어 가속

- 일부 Windows 장치에서 WebView2 GPU 초기화 실패 시 빈 화면 → `--disable-gpu` 인자 주입으로 회복 사례 존재.

### 7.5 Dist 경로 문제

- `tauri.conf.json` 설정: `frontendDist: "../dist"` → 상대 경로 해석이 빌드/실행 위치에 따라 실패 시 WebView가 빈 문서 로드.

### 7.6 초기 로깅 미수집

- Rust 로거는 Info 레벨. 프론트엔드 Logger는 React 진입 후 초기화. 그 이전 단계에서 발생한 오류는 파일 로그/콘솔 미출력 가능.

### 7.7 WASM + top-level-await

- `vite.config.ts` 플러그인: `vite-plugin-wasm`, `vite-plugin-top-level-await`
- 해당 플러그인 로딩 중 CSP 제약 또는 WebView2 구버전 기능 누락 시 모듈 체인이 끊어질 수 있음.

## 8. 즉시 수행 권장 진단 액션 (Diagnostic Checklist)

| 번호 | 액션                                                                                                                                           | 기대 결과             |
| ---- | ---------------------------------------------------------------------------------------------------------------------------------------------- | --------------------- |
| D1   | 릴리스 빌드 후 `dist/` 내용 확인 (index.html, hashed assets 존재)                                                                              | 경로 문제 배제        |
| D2   | `tauri.conf.json` CSP에 임시로 `script-src 'self' tauri: 'unsafe-inline' 'unsafe-eval' blob: data:` / `worker-src 'self' blob:` 추가 후 재빌드 | CSP 차단 여부 확인    |
| D3   | Worker import를 URL 기반(`import workerUrl from './mcp-worker.ts?worker&url'; new Worker(workerUrl,{type:'module'})`)으로 교체 테스트          | Worker 번들 문제 검증 |
| D4   | Rust 시작 시 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--disable-gpu"` 환경변수 주입                                                             | GPU 문제 여부         |
| D5   | 문제 PC에 WebView2 Evergreen 설치/업데이트 수행                                                                                                | Runtime 문제 여부     |
| D6   | 로그 레벨 Trace로 상향 및 `index.html`에 `window.onerror` / `unhandledrejection` 핸들러 삽입                                                   | 초기 에러 캡처        |
| D7   | 빌드 산출물 내 `http://localhost:1420` 잔존 문자열 grep                                                                                        | Dev URL 누락 여부     |

## 9. 제안 패치 세트 (Staged Fix Proposals)

### Patch A (진단용 임시)

1. CSP 확장 (blob:, worker-src)
2. WebView2 GPU 비활성화 환경변수 추가
3. 로그 레벨 Trace + 초기 오류 핸들러 삽입

### Patch B (구조 개선)

1. Worker 생성 로직을 URL import 방식으로 통일 (`?worker&url`)
2. 실패 시 재시도 + 에러 토스트 표시
3. 초기 부트스트랩 헬스체크 (`window.__BOOT_OK = true` 기록 후 React 진입 확인)

### Patch C (관측성)

1. 프론트 부트스트랩 매우 초기 단계에서 Tauri invoke로 `bootstrap_log` 커맨드 호출하여 Rust log에 기록
2. 릴리스 빌드 시 자동 로그 디렉터리 zip 내보내기 스크립트 추가

## 10. 로그/에러 수집 개선 제안

- `index.html` 최상단 inline script: `console.log('[bootstrap] start'); window.addEventListener('error', ...);`
- Rust 측: Builder setup 직후 "WebView started" Trace 출력
- 실패 지점 시나리오별 구분 문자열: `[worker-init-fail]`, `[wasm-load-fail]`, `[csp-block]`

## 11. 추가 필요 정보 (요청)

개발팀/QA로부터 아래 정보를 요청합니다:

1. 문제 발생 PC의 WebView2 Runtime 버전 (레지스트리 또는 설치 관리자 스크린샷)
2. GPU 모델 / 드라이버 버전
3. 릴리스 실행 직후 로그 디렉터리 내용(`AppData/Local/.../libragent.log`) 첨부
4. `dist/` 폴더 내용(파일 목록) 캡처
5. (가능하면) 임시 Trace 빌드 적용 후 캡처된 초기 콘솔 로그

## 12. 결론 (Summary)

현상은 **초기 웹 자산 로딩 실패** 또는 **WebView2 환경 이슈**가 가장 유력. 1차로 Worker/CSP/GPU/WebView2 4가지 축을 빠르게 분리 진단하면 원인 협소화 가능. 위 표(D1~D7)를 순차 적용한 결과 공유 요청.

## 13. 액션 요청 (Action Items for Dev Team)

- [ ] Patch A 적용한 테스트 빌드 생성 및 QA 전달
- [ ] D1~D7 체크리스트 실행 후 결과 회신
- [ ] Worker import 패턴 교체 PoC 분기(branch) 생성 (`feat/windows-white-screen-diagnostic`)
- [ ] CSP 최소 허용치 재산정 후 보안팀 검토
- [ ] WebView2 Runtime 설치 스텝 README Windows 섹션에 명문화

---

문의/추가 요구 사항은 이 문서 댓글로 이어서 진행 바랍니다.

> 최초 보고 내용 (원본):
>
> - when the release built binary is executed, only white window displayed
> - issue <https://github.com/tauri-apps/tauri/issues/7118> seems relevant
> - only Windows build (not linux or mac)
> - 관련해서 별다른 error log가 없음 → 원인 분석 어려움
