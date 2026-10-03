---
title: 브라우저 자동화 (Browser Sidecar)
---

# 브라우저 자동화 (Browser Sidecar)

LibrAgent는 에이전트가 웹사이트를 탐색하고, 필요한 정보를 검색·수집하며, 화면 스크린샷을 분석할 수 있도록 강력한 브라우저 자동화 기능을 제공합니다.

---

## 🛡️ 완벽한 프로세스 격리와 안정성

웹 브라우징 중 복잡한 웹페이지나 스크립트 오류가 발생하더라도 사용자의 작업 환경을 안전하게 보호합니다:

- **독립된 샌드박스 실행**: 브라우저 자동화는 LibrAgent 데스크톱 앱 본체와 분리된 독립 프로세스에서 안전하게 실행됩니다. 웹페이지가 충돌하거나 과도한 메모리를 사용해도 앱 본체는 안전합니다.
- **고정 에이전트 프로필**: `browser__createSession`의 `browser="sidecar"`(기본값)는 이 기기의 LibrAgent 에이전트 브라우저 프로필을 재사용합니다. 그 브라우저 *안에서* 한 로그인은 이후 세션에도 유지되며, 설정 → 시스템 → **에이전트 브라우저** → 에이전트 브라우저 데이터 지우기로 삭제할 수 있습니다. 일상 Chrome과는 별개입니다. 브라우저 sidecar가 떠 있는 동안 여러 에이전트 채팅은 같은 쿠키 jar를 공유합니다.

---

## 일상 Chrome (MV3 확장 브리지)

**일상 Chrome**에 이미 로그인된 사이트를 쓰려면, 로컬 MV3 확장(Load unpacked 또는 Chrome Web Store)과 loopback WebSocket 브리지(`ws://127.0.0.1:3847/extension-bridge`, 기본값)로 탭을 제어할 수 있습니다.

| `createSession`의 `browser` | 의미 |
| --- | --- |
| **`sidecar`** (기본) | Sticky CDP 에이전트 브라우저 — 일상 Chrome과 별개 |
| **`userChrome`** | 확장 브리지로 일상 Chrome — **미연결이면 에러**; sidecar로 조용히 넘어가지 않음 |

에이전트는 create 시점에 대상을 골라야 합니다. `userChrome` 실패 시 sidecar로 자동 전환하지 않아 읽기 대상이 섞이지 않습니다.

**에이전트당 active 브라우저 세션 1개(SSOT):** 각 에이전트 채팅은 active 브라우저 세션을 하나만 둡니다. `browser__createSession`은 이전 세션을 닫고 교체합니다(`sidecar` ↔ `userChrome` 전환 포함). 다른 브라우저 도구는 항상 그 active만 대상으로 하며, `sessionId` 인자나 일상 Chrome+에이전트 브라우저 동시 유지는 없습니다. 백엔드를 바꾸려면 create를 다시 호출하세요.

- 설치: `chrome-extension/README.md` 참고, 또는 설정 → 시스템 → **에이전트 브라우저** → Chrome 확장 프로그램 브리지(상태 우선 카드)에서 경로 복사. Chrome Web Store 패키징은 `chrome-extension/STORE.md`를 참고하세요.
- 한 번 설치하면(Load unpacked 또는 Store) LibrAgent 재시작 시 확장이 자동으로 다시 붙습니다. 툴바 팝업·설정 상태를 보면 되고, 확장 **파일**을 바꾼 뒤에만 `chrome://extensions`에서 **새로고침**이 필요합니다(개발자 업데이트).
- `browser="userChrome"`이고 Connected일 때 대부분의 브라우저 도구(navigate, content, click/input, evaluateJS, 뷰포트 스크린샷)가 일상 Chrome으로 동작합니다. `getConsoleLogs`만 sticky sidecar 전용입니다. 확장 스크린샷은 뷰포트만 지원합니다(`fullPage` 무시).
- 인증: `LIBRAGENT_EXTENSION_BRIDGE_TOKEN`이 없으면 고정 개발 토큰 `libragent-dev`. 프로덕션에서는 회전 토큰을 사용할 예정입니다.

---

## 🌐 주요 기능

| 기능                                                              | 설명                                                                      |
| ----------------------------------------------------------------- | ------------------------------------------------------------------------- |
| **페이지 방문 및 탐색** (`browser__navigateToUrl`)                | 대상 웹사이트 URL에 접속하여 페이지 로딩 완료를 감지하고 본문 확인        |
| **화면 캡처** (`browser__takeScreenshot`)                         | 웹페이지 화면 또는 전체 페이지(Full Page)를 이미지로 캡처하여 시각적 검증 |
| **콘솔 및 상태 모니터링**                                         | 웹페이지 내부 에러 및 네트워크 상태를 파악하여 오류 분석 지원             |
| **인터랙션 지원** (`browser__clickElement`, `browser__inputText`) | 링크 클릭, 스크롤, 텍스트 입력 등 자연스러운 웹 서핑 수행                 |

---

## 🎯 대표적인 활용 시나리오

1. **최신 기술 문서 및 릴리즈 노트 조사**:
   - 실시간 웹페이지를 직접 방문하여 최신 라이브러리 문서나 뉴스를 요약합니다.
2. **웹 UI 개발 결과 검증**:
   - 로컬 개발 서버(`http://localhost:3000`)에 접속하여 UI가 의도대로 렌더링되었는지 스크린샷으로 확인합니다.
3. **정기 브리핑 자동화**:
   - 예약 작업(Scheduled Tasks)과 결합하여 매일 특정 대시보드나 주요 사이트를 방문하여 일일 요약을 받아봅니다.

---

## 💡 유의 사항

- 첫 웹 브라우징 실행 시 백그라운드 브라우저 엔진이 초기화되므로 최초 1회에 한해 수 초의 준비 시간이 소요될 수 있습니다.
