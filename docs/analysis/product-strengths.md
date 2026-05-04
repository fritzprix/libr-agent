# LibrAgent Product Strengths

이 문서는 루트에 흩어져 있던 `FEATURES.md`와 `FEATURES_GEMINI.md`의 내용을 병합해 정리한 **제품 강점 분석 문서**다.  
기준은 `CHANGELOG.md` 전반에 반복적으로 드러난 방향성과, 현재 README/제품 구조가 실제로 무엇을 잘하는지다.

---

## 한 문장 요약

**LibrAgent는 로컬 실행 환경, MCP 확장성, 실전형 workspace/browser/knowledge 도구, 그리고 multi-agent orchestration을 하나의 데스크톱 제품으로 묶은 Agent Harness 플랫폼이다.**

---

## 핵심 강점

### 1. 에이전트 오케스트레이션이 제품의 중심이다

- **Rust 중심 Agent V2 orchestration**: 세션 생성, 실행, 재개, 취소, 결과 수집을 백엔드 중심으로 안정적으로 제어한다.
- **세션 계보(lineage)와 위임 흐름**: parent-child session 제어, delegated session 검증, teamwork/org 확장까지 이어지는 구조가 있다.
- **Draft 기반 세션 생성 UX**: 빈 세션을 먼저 만들지 않고, 에이전트 프로필과 도구 구성을 검토한 뒤 첫 메시지와 함께 시작할 수 있다.
- **Playbook / Skills / Scheduled Tasks**: 반복 업무를 구조화하고 자동화하는 레이어가 별도 기능이 아니라 전체 시스템 안에 결합되어 있다.

### 2. MCP를 기능이 아니라 인프라로 다룬다

- **외부 MCP + 내장 MCP 공존**: builtin server와 external server를 함께 다룬다.
- **전송 계층 다양성**: stdio / HTTP / SSE / OAuth 2.1 흐름을 다룰 수 있다.
- **registry / preset / metadata 수명주기 관리**: 설치, 표시, 상태 추적, 중복 방지까지 포함한 운영 계층이 있다.
- **에이전트 친화적 도구 설계**: 도구 설명, 응답 형식, metadata 정합성을 반복 개선해 실제 agent runtime에서 쓰기 좋은 생태계를 지향한다.

### 3. “실제로 일시키는” 실행 서브스트레이트가 강하다

- **Workspace**: 파일 생성/편집/검색/내보내기, 라인 기반 수정, 멀티 파일 작업 등 실제 개발·운영 작업에 필요한 기능이 강하다.
- **Shell**: isolated 실행과 persistent shell을 분리해 사용하며, 비동기 프로세스 모니터링까지 지원한다.
- **Browser**: 페이지 읽기 단순화, 캐시 일관성 개선, 탐색 자동화 흐름이 안정적으로 정리돼 있다.
- **Knowledge / Content Store**: 검색 스니펫, 소스 필터링, 업로드 컨텍스트, 전역 지식 관리 등 RAG성 사용 흐름이 계속 확장됐다.

### 4. 긴 세션과 복잡한 흐름을 버티는 데 집착한다

- **Context compaction**: 긴 세션에서 토큰 압박과 DB 부하를 줄이는 흐름을 지속적으로 개선했다.
- **Loop prevention / circuit breaker / stale response 대응**: 에이전트가 잘못된 루프나 오래된 상태에 매달리는 문제를 방지한다.
- **세션 격리와 상태 동기화**: startup race, proxy initialization, session loading, runtime sequencing 같은 까다로운 안정성 문제를 꾸준히 정리했다.
- **출력 가시성 개선**: 프로세스 출력, 파일 라인 번호, markdown table 기반 히스토리 등 agent와 사용자가 상태를 더 쉽게 읽도록 다듬었다.

### 5. 로컬 중심 데이터 관리와 검색 경험이 좋다

- **IndexedDB → Rust/SQLite/SeaORM 전환**: 설정, 메시지, 엔티티 CRUD가 점점 더 일관된 백엔드 구조로 정리됐다.
- **BM25 기반 검색과 history 흐름**: 검색 경험이 단순 부가기능이 아니라 플랫폼의 기본 축 중 하나로 성장했다.
- **데이터 보존성**: MCP 서버 업데이트 시 식별자/생성일 보존, session metadata 최적화, schema mismatch 수정처럼 운영상 중요한 정합성을 챙긴다.

### 6. UI/UX를 덤으로 취급하지 않는다

- **Agent chat UX**: 스크롤 앵커링, bottom-follow, 세션 전환 flicker, tool grouping 등 사용감 개선이 반복된다.
- **Localization**: Rosetta 기반 현지화와 한국어 번역 확장이 매우 자주 등장한다.
- **Accessibility**: ARIA, semantic button, keyboard/focus 개선이 꾸준히 들어간다.
- **관리 화면 품질**: Settings, MCP servers, skills, knowledge manager, history 같은 운영성 UI를 계속 다듬고 있다.

---

## 왜 이게 중요한가

### 1. “채팅 앱”보다 “에이전트 운영체제”에 가깝다

CHANGELOG를 보면 단순한 프롬프트 UI보다 **세션, 위임, 협업, 자동화, 상태 관리**에 훨씬 더 많은 투자가 들어갔다.  
즉, LibrAgent는 질문-응답 앱이 아니라 **Agent를 운영하는 기반**에 더 가깝다.

### 2. 화려한 기능보다 안정성 투자 비율이 높다

scroll anchoring, stale response, registry race, delegated access validation, workspace hydration 같은 수정이 반복된다.  
이건 데모보다 **장시간 운용**에 더 집착한 제품이라는 뜻이다.

### 3. 기능이 커질수록 아키텍처도 같이 정리한다

큰 모듈 분리, metadata SSOT화, MCP 타입/응답 계층 정리 등은 이 코드베이스가 기능만 쌓는 방식이 아니라는 걸 보여준다.

### 4. 에이전트가 덜 헛짓하도록 설계한다

도구 설명, 출력 가시성, metadata 정합성, inventory contract를 손본 흔적이 많다.  
즉, LibrAgent는 사람이 보기 좋은 UI를 넘어서 **agent runtime 자체의 품질**을 신경 쓴다.

---

## 정리

LibrAgent의 가장 큰 매력은 아래 네 가지가 한 제품 안에서 같이 자란다는 점이다.

1. **로컬 중심 보안과 실행 환경**
2. **MCP 기반 확장성**
3. **실전형 작업 substrate**
4. **단일 agent에서 swarm/org로 이어지는 orchestration**

쉽게 말하면, LibrAgent의 강점은 기능 수가 아니라 **에이전트가 실제로 오래, 깊게, 협업하며 일할 수 있도록 기반을 계속 다듬어 왔다는 점**이다.
