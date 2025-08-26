# 계획: `src/context/WebMCPContext.tsx` 축소 — 오직 `useWebMCPServer`만 남기기

목표: `WebMCPContext.tsx`에서 public API를 최소화하여, 소비자(컴포넌트/훅)는 타입 안전한 서버 프록시(`useWebMCPServer`)만 사용하도록 만든다. 내부적으로는 서버 프록시를 생성·제공하는 최소한의 기능(`getWebMCPServer`, 초기화/정리, proxy 상태)을 유지한다.

체크리스트
- WebMCPContext는 오직 내부 worker lifecycle과 `getWebMCPServer`(내부) / `useWebMCPServer`(공개)만 제공한다.
- `availableTools`, `listTools`, `callTool` 등 고수준 MCPTool API는 제거한다(또는 features/tools로 이관).
- `useWebMCPManagement`는 `getWebMCPServer`와 `serverStates`만 반환하도록 축소한다.
- 관련 소비자(예: `BuiltInToolsSystemPrompt`, `ResourceAttachmentContext`, `Chat`)는 `useWebMCPServer`를 사용하도록 유지/수정한다.
- 단위/통합 테스트와 빌드(git branch, CI 통과)를 준비한다.

변경 범위(파일)
- 주요: `src/context/WebMCPContext.tsx` — 공용 API 축소, 내부 `getWebMCPServer` 유지, `useWebMCPServer`만 공개.
- 소비자: `src/features/*`, `src/context/ResourceAttachmentContext.tsx`, `src/features/prompts/BuiltInToolsSystemPrompt.tsx`, `src/features/chat/Chat.tsx` 등에서 `useWebMCPServer` 사용을 확인.
- 문서: `docs/history/refactoring_YYYYMMDD_HHMM.md`에 요약 기록.

세부 단계 (작업 순서)
1) 안전망: 모든 `useWebMCPServer` 사용처를 찾고 리스트업(자동화된 검색). (완료: 검색 결과가 존재함)
2) 테스트 추가(먼저):
  - `getWebMCPServer`가 정상적으로 proxy를 반환하는 유닛(모킹).
  - `useWebMCPServer`의 기본 흐름(happy path + 서버 미로딩 에러).
3) `WebMCPContext.tsx` 리팩터:
  - `WebMCPContextType`에서 `availableTools`, `callTool`, `listTools` 등을 제거.
  - 내부에 `getWebMCPServer`, `initializeProxy`, `cleanup`, `getProxyStatus` 유지.
  - `useWebMCPManagement`가 `getWebMCPServer`와 `serverStates`만 노출하도록 축소.
  - `useWebMCPServer` 훅은 기존 동작(타입 안전한 프록시 반환)을 그대로 유지.
4) 소비자 점검/수정:
  - `ResourceAttachmentContext`, `BuiltInToolsSystemPrompt`, `Chat` 등에서 `useWebMCPServer` 사용을 확인하고 필요 시 경량 수정.
5) 통합: `features/tools`가 `listTools`/`callTool` 등의 고수준 로직을 담당하도록 필요한 어댑터(예: `WebMCPToolProvider`)를 구현하거나 기존 구현을 유지.
6) CI: `pnpm lint && pnpm test && pnpm build` 실행 및 문제 해결.
7) 문서/히스토리 추가 및 PR 작성.

테스트/품질 게이트
- Unit tests: `getWebMCPServer`, `useWebMCPServer` (happy path, error path).
- Build: `pnpm build` 성공.
- Lint: `pnpm lint` 통과.

마이그레이션 전략(점진적)
- 먼저 `useWebMCPServer` 사용처는 그대로 두고, 컨텍스트의 public 함수들을 deprecated 상태로 두는 브랜치가 가능하다면 단계적 제거 권장.
- 위험이 큰 변경(consumer가 callTool/listTools 직접 사용)은 어댑터 레이어로 일시적으로 포워딩하고 로그 경고를 남기며 점차 제거.

리스크 & 완화
- 리스크: `callTool`/`listTools`를 직접 사용하던 컴포넌트가 깨짐 — 완화: 어댑터/호환 레이어 제공(짧은 기간).
- 리스크: 초기화 순서(프록시 미초기화 상태) — 완화: `initializeProxy` 상태 버블업과 소비자에서 `useWebMCPServer`가 로딩 상태를 적절히 처리하도록 보장.

예상 소요 시간 (대략)
- 테스트 추가: 1–2시간
- 컨텍스트 리팩터 + 소비자 점검: 2–3시간
- 통합 테스트, 빌드, 문서: 1–2시간

다음 단계 (제가 수행할 수 있는 작업)
1. 이 계획의 세부 패치를 만들어 `WebMCPContext.tsx`를 축소하고 소비자 수정을 적용해 `pnpm build`를 실행해 드립니다.
2. 먼저 테스트를 추가한 뒤 리팩터를 적용하는 안전한 순서로 진행할 수도 있습니다 (권장).

원하시면 지금 바로 1) 컨텍스트 축소 패치를 생성하고 2) `pnpm build`를 실행하겠습니다. 어느 방식으로 진행할까요?
