# Comprehensive Overview of Refactoring Plans (August 2025)

이 overview는 제공된 refactoring 계획 문서들을 시간 순서(파일명 기반: 2025-08-23부터 2025-08-28까지)로 분석하여 작성된 것입니다. 각 문서는 코드베이스의 다양한 부분(예: 브라우저 도구, 파일 관리, 메시지 저장, 시스템 프롬프트 등)을 개선하기 위한 계획을 담고 있으며, 일부는 완료(COMPLETED) 상태로 표시되어 있습니다. 유사한 주제나 변경(예: BuiltIn Tool Provider 리팩터링의 중복 문서, 브라우저 도구 관련 다중 개선)가 겹칠 경우, 가장 최근 문서의 내용을 우선 반영했습니다. 이는 계획의 진화나 수정(예: 초기 계획이 후속 문서에서 업데이트됨)을 반영하기 위함입니다. 전체적으로 이 refactoring 노력은 코드 중복 제거, 보안 강화, 성능 최적화, 모듈화, 그리고 AI 에이전트(예: 브라우저 상호작용)의 안정성을 목표로 하며, Tauri 기반 백엔드(Rust)와 프론트엔드(React/TypeScript)의 통합을 강조합니다.

## 1. Overall Goals and Themes

이 refactoring 시리즈는 애플리케이션의 핵심 컴포넌트(브라우저 도구, 파일 시스템, 채팅 메시지, 시스템 프롬프트, 백엔드 호출)를 모듈화하고 최적화하는 데 초점을 맞춥니다. 주요 테마:

- **모듈화와 관심사 분리**: 중복된 로직(예: 파일 접근, 도구 등록)을 단일 서비스나 컨텍스트로 통합.
- **보안 및 안정성 강화**: 경로 검증, 에러 처리, 데드락 해결을 통해 보안 정책 일관성 확보.
- **성능 및 효율성**: LLM(AI 모델) 입력 최적화(예: HTML을 Markdown으로 변환), 병렬 처리 충돌 방지, 폴링 기반 비동기 통신.
- **테스트 및 유지보수성**: 린팅, 빌드 검증, 단위 테스트 추가를 강조.
- **완료 상태**: 일부(예: SecureFileManager, browser_getPageContent, Browser Tool Return Value)는 완료되었으나, 나머지는 계획 단계로 보임.

전체 변경은 Tauri 앱의 Rust 백엔드와 React 프론트엔드 간의 통합을 강화하며, MCP(Multi-Command Protocol) 서버와 도구 시스템의 확장성을 높입니다. 예상 이점: 코드 중복 50% 이상 감소, AI 처리 비용 절감(토큰 수 감소), 데드락/충돌 제거로 안정성 향상.

## 2. Key Refactoring Areas

변경을 주제별로 그룹화하여 요약. 시간 순서상 초기 계획이 후속으로 업데이트된 경우(예: 브라우저 도구 관련), 최근 버전을 우선 반영.

### A. Browser Tools and Integration (주요 테마: 데드락 해결, 도구 향상)

브라우저 에이전트(예: WebView 상호작용)의 안정성과 유용성을 개선하는 데 초점. 초기 데드락 해결 계획(20250823_2210)이 후속 문서(20250823_2240_COMPLETED, 20250825_1400)에서 구체화됨.

- **Deadlock Resolution (20250823_2210.md)**: Rust 백엔드의 `execute_script` 명령어가 WebView에서 결과를 기다리다 데드락 발생 문제를 해결. 비동기 폴링 메커니즘 도입: `request_id` 생성 후 `poll_script_result` 명령어로 결과 폴링(인터벌 100ms, 타임아웃 10s). 프론트엔드에 `use-browser-invoker.ts` 훅 추가. 도구 등록을 `BuiltInToolContext.tsx`에 `registerLocalTools`로 확장하여 브라우저 도구(예: `browser_getPageContent`)를 표준화.
- **browser_getPageContent Enhancement (20250823_2240_COMPLETED.md, 완료)**: Raw HTML 반환의 노이즈(스크립트, 스타일)를 제거하고 Markdown으로 변환(Turndown 라이브러리 추가). Raw HTML을 SecureFileManager로 안전 저장(`temp_html/[timestamp-random].html`). 응답 형식: JSON 구조({content: markdown, saved_raw_html: path, metadata: {...}}). LLM 토큰 수 60-80% 절감. Turndown 규칙 추가(스크립트 제거, 줄바꿈 보존).

- **Click/Input Return Value Diagnostics (20250825_1400.md, 완료)**: `click_element`와 `input_text` 도구의 반환값을 null에서 JSON envelope로 업그레이드({ok: true/false, action, selector, timestamp, diagnostics: {visible, disabled, ...}}). 프론트엔드에 `formatBrowserResult` 함수 추가(인간-읽기 형식 변환). 폴링 메커니즘 통합(30회 시도, 3초 타임아웃). Rust 백엔드에서 다중 클릭/이벤트 시도 및 에러 처리 강화. 단위 테스트 추가(성공/실패 시나리오).

- **BuiltIn Tool Provider Refactor (20250825_062210.md & 062220.md, 중복 - 최근 버전 반영)**: `features/tools`를 내장 도구의 단일 레지스트리로 통합. `BrowserToolProvider`가 `register/unregister`로 서비스 등록(예: `BuiltInService` 인터페이스). 이름 파싱 강화(`indexOf('__')`로 첫 `__`만 분리, 단위 테스트 추가). `RustMCPToolProvider`와 `WebMCPToolProvider`도 서비스 등록으로 전환. `BuiltInToolContext.tsx` 중복 제거(Deprecated 마킹). 스크립트 인젝션 강화 및 에지케이스 테스트 추가.

**전체 효과**: 브라우저 도구의 안정성 향상(데드락 제거, 진단 정보 추가), 확장성 강화(등록 메커니즘 표준화).

### B. File Management and Security (주요 테마: 통합 및 보안)

파일 시스템 접근을 중앙화하여 중복 제거.

- **SecureFileManager Structure Improvement (20250823_2230_COMPLETED.md, 완료)**: 파일 읽기/쓰기 로직을 `SecureFileManager` 서비스로 통합. Tauri 커맨드(`lib.rs`)와 MCP 서버(`filesystem.rs`)에서 공유 사용. 프론트엔드(`use-rust-backend.ts`)에서 API 통일(readFile/writeFile). 보안 검증(`SecurityValidator`) 중앙화. 결과: 코드 중복 제거, 일관된 정책 적용.

**전체 효과**: 파일 접근 보안 강화, 유지보수성 향상.

### C. Message and Prompt Management (주요 테마: 모듈화, 충돌 방지)

채팅 메시지와 시스템 프롬프트의 저장/관리를 최적화.

- **Chat Message Storage API Integration (20250825_1333.md)**: `SessionHistoryContext`의 `addMessage`와 `addHistoryMessages`를 `addMessages(messages: Message[])`로 통합(원자적 배치 추가). 병렬 mutate 충돌로 인한 도구 결과 유실 해결. Tool-result `content`를 문자열로 직렬화. `ChatContext.submit`에서 배치 API 사용. ESLint/Prettier 적용, 빌드 검증.

- **System Prompt Management Modularization (20250828_1230.md, 가장 최근)**: 시스템 프롬프트를 `ChatContext`에서 분리하여 `SystemPromptProvider` 생성(`SystemPromptContext.tsx`). 등록/해제 API(`register/unregister`, ID 기반). `BuiltInService` 인터페이스에 `getServiceContext` 추가(MCP 서버 컨텍스트 제공). 프롬프트 컴포넌트(예: `BuiltInToolsSystemPrompt`) 마이그레이션. Rust 백엔드에 `get_service_context` 명령어 추가. 성능 캐싱 고려.

**전체 효과**: 충돌 방지, 모듈화로 새로운 프롬프트 추가 용이, AI 응답 품질 향상.

### D. Backend and Context Centralization (주요 테마: 중복 제거)

백엔드 호출과 컨텍스트를 중앙화.

- **Centralize Rust Backend Invoke Logic (20250824_1515.md)**: 중복된 Tauri 호출 로직을 `rust-backend-client.ts`로 통합(safeInvoke 래퍼). `use-rust-backend.ts`를 thin wrapper로 변경. 컨텍스트(예: `BuiltInToolContext.tsx`)에서 훅 사용. `tauri-mcp-client.ts` 단계적 제거. 명령어 명칭 통일(예: `list_builtin_servers`).

**전체 효과**: API 일관성 확보, 테스트 용이성 향상.

## 3. Completed vs. Planned Changes

- **Completed (✅)**: SecureFileManager 구조 개선 (20250823_2230), browser_getPageContent 기능 개선 (20250823_2240), Browser Tool Return Value Enhancement (20250825_1400). 이들은 실제 구현, 검증(린팅, 빌드, 테스트) 완료.
- **Planned (📝)**: 나머지(데드락 해결, 백엔드 중앙화, 메시지 API 통합, 도구 프로바이더 리팩터링, 시스템 프롬프트 모듈화). 마이그레이션 단계, 테스트 계획, 위험 완화(예: 어댑터 레이어) 포함.
- **중복 처리**: BuiltIn Tool Provider 문서(062210 & 062220)는 동일하므로 최근(062220) 반영. 브라우저 관련 초기 데드락 계획이 후속 향상으로 보완됨.

## 4. Risks, Mitigations, and Next Steps

- **위험**: 마이그레이션 중 호환성 문제(이름 충돌, UI 브레이크), 성능 저하(폴링 오버헤드). 대처: 단계적 전환, 어댑터 사용, 단위/통합 테스트 추가.
- **검증 기준**: pnpm lint/build/test, cargo fmt/clippy/build, 수동 AI 응답 확인.
- **예상 타임라인**: 각 변경 1-4시간(구현/테스트), 전체 3-5일.
- **문서화**: 각 완료 시 `docs/history/refactoring_[timestamp].md` 추가(PR 링크 포함).
- **다음 행동 제안**: 완료된 변경을 기반으로 계획된 부분(예: 시스템 프롬프트 모듈화)을 우선 구현. 전체 코드베이스 검색으로 남은 중복 확인.
