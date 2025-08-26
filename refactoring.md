# BuiltIn Tool Provider / BrowserToolProvider refactor

## 작업 목적

기존 `src/context/BuiltInToolContext.tsx`와 `src/features/tools/index.tsx`에 중복되어 있는 내장(BuiltIn) 도구 등록/라우팅 책임을 정리하고, 브라우저 제공자(`BrowserToolProvider`)와 기타 서비스들이 `features/tools`의 단일 `BuiltInToolProvider`에 `register`/`unregister`로 서비스 형태로 등록하도록 리팩터링합니다.

이유 요약:
- 책임 분리: 한 곳에서만 내장 도구를 관리하면 라우팅/네이밍 규칙을 일관성 있게 유지할 수 있습니다.
- 안전성: `__` 구분 처리 등 이름 파싱을 견고하게 하여 edge-case를 제거합니다.

## 현재 상태 / 문제점

- `BuiltInToolContext`와 `features/tools`가 중복 API를 제공하여 혼란 발생
- 브라우저 툴 제공자(`BrowserToolProvider`)가 레거시 컨텍스트에 직접 등록함(결합도 높음)
- 도구 이름 파싱에 `split('__')`를 사용해 툴 이름에 `__`가 여러 개인 경우 잘못 분리됨
- UI(예: `ToolsModal`)에서 도구 출처를 구분해 표시하지 않음. 일부 텍스트 색상(token)이 배경과 대비가 낮아 보이지 않을 수 있음

## 변경 이후 상태 / 성공 판정 기준

- `BrowserToolProvider`, `RustMCPToolProvider`, `WebMCPToolProvider` 등 모든 내장 서비스가 `useBuiltInTool().register(serviceId, service)`로 등록하고 언레지스터합니다.
- `BuiltInToolProvider`가 내장 도구의 단일 소스 오브 트루스로 동작합니다.
- 이름 파싱은 첫번째 `__`만 분리(indexOf 기반)하도록 변경되어 `builtin.filesystem__name__with__underscores` 같은 케이스가 올바르게 처리됩니다.
- `ToolsModal` 등 UI는 MCP 도구와 Builtin 도구 출처를 명확히 표시합니다.
- 관련 단위/통합 테스트가 추가/업데이트되어 통과합니다.

## 수정이 필요한 코드(파일목록 및 목적)

- `src/features/tools/index.tsx` (BuiltInToolProvider)
  - execute 라우팅에서 `__` 분해를 안전하게 변경
  - `availableTools` 집계 로직 검토

# BuiltIn Tool Provider / BrowserToolProvider refactor

## 작업 목적

기존 `src/context/BuiltInToolContext.tsx`와 `src/features/tools/index.tsx`에 중복되어 있는 내장(BuiltIn) 도구 등록/라우팅 책임을 정리하고, 브라우저 제공자(`BrowserToolProvider`)와 기타 서비스들이 `features/tools`의 단일 `BuiltInToolProvider`에 `register`/`unregister`로 서비스 형태로 등록하도록 리팩터링합니다.

이유 요약:

- 책임 분리: 한 곳에서만 내장 도구를 관리하면 라우팅/네이밍 규칙을 일관성 있게 유지할 수 있습니다.

- 안전성: `__` 구분 처리 등 이름 파싱을 견고하게 하여 edge-case를 제거합니다.

## 현재 상태 / 문제점

- `BuiltInToolContext`와 `features/tools`가 중복 API를 제공하여 혼란 발생

- 브라우저 툴 제공자(`BrowserToolProvider`)가 레거시 컨텍스트에 직접 등록함(결합도 높음)

- 도구 이름 파싱에 `split('__')`를 사용해 툴 이름에 `__`가 여러 개인 경우 잘못 분리됨

- UI(예: `ToolsModal`)에서 도구 출처를 구분해 표시하지 않음. 일부 텍스트 색상(token)이 배경과 대비가 낮아 보이지 않을 수 있음

## 변경 이후 상태 / 성공 판정 기준

- `BrowserToolProvider`, `RustMCPToolProvider`, `WebMCPToolProvider` 등 모든 내장 서비스가 `useBuiltInTool().register(serviceId, service)`로 등록하고 언레지스터합니다.

- `BuiltInToolProvider`가 내장 도구의 단일 소스 오브 트루스로 동작합니다.

- 이름 파싱은 첫번째 `__`만 분리(indexOf 기반)하도록 변경되어 `builtin.filesystem__name__with__underscores` 같은 케이스가 올바르게 처리됩니다.

- `ToolsModal` 등 UI는 MCP 도구와 Builtin 도구 출처를 명확히 표시합니다.

- 관련 단위/통합 테스트가 추가/업데이트되어 통과합니다.

## 수정이 필요한 코드(파일목록 및 목적)

- `src/features/tools/index.tsx` (BuiltInToolProvider)

	- execute 라우팅에서 `__` 분해를 안전하게 변경

	- `availableTools` 집계 로직 검토

- `src/features/tools/BrowserToolProvider.tsx`

	- 기존 `registerLocalTools` 사용을 제거하고 `useBuiltInTool().register/unregister`로 전환

	- `listTools`가 반환하는 툴 메타는 단축(서비스 내부) 이름만 포함하도록 조정

- `src/features/tools/RustMCPToolProvider.tsx`, `WebMCPToolProvider.tsx`

	- 같은 방식으로 서비스 등록/언레지스터 확인

- `src/context/BuiltInToolContext.tsx`

	- (선택) deprecated 처리 또는 어댑터로 `features/tools`로 포워딩

- `src/features/tools/ToolsTestPage.tsx`, `src/features/tools/ToolsModal.tsx`

	- `split('__')` 사용처를 찾아 안전 분해로 교체

	- `ToolsModal`에 출처 배지(source)와 대비 개선

- 테스트 파일들: `src/features/tools/*.test.tsx`, `src/context/MCPServerContext.test.tsx` 등

	- 파싱 규칙/라우팅 변경에 따른 테스트 업데이트 및 추가

## 핵심 코드 스니펫(예시 변경안)

안전한 이름 분해(추천)

```tsx
const stripped = name.startsWith('builtin.') ? name.slice('builtin.'.length) : name;
const idx = stripped.indexOf('__');
if (idx === -1) throw new Error(`Invalid builtin tool name: ${stripped}`);
const serviceId = stripped.slice(0, idx);
const toolName = stripped.slice(idx + 2);
```

Browser 서비스 등록 예시

```tsx
const serviceId = 'browser';
const service: BuiltInService = {
	listTools: () => browserTools.map(({ name, description, inputSchema }) => ({ name, description, inputSchema })),
	executeTool: async (call) => { /* 안전한 인수 파싱 및 실행 */ },
};
register(serviceId, service);
// cleanup -> unregister(serviceId)
```

ToolsModal: 출처 배지 및 대비 개선(요약)

```tsx
// 병합: builtinTools.map(t => ({...t, source: 'builtin'})) + mcpTools.map(t => ({...t, source: 'mcp'}))
// badge: source === 'builtin' ? 'builtin' : 'mcp'
// 텍스트 토큰: text-foreground 대신 text-muted-foreground 사용 위치 점검
```

## 테스트

- 단위

	- 이름 파싱: `builtin.filesystem__list_directory`, `builtin.filesystem__name__with__underscores`, `builtin.invalidname`(에러)

	- BrowserToolProvider의 `register`/`unregister` 동작

	- ToolsModal에서 두 출처가 모두 보이는지

- 통합

	- 가짜 서비스 등록 후 `BuiltInToolProvider.executeTool` 경로 검증

## 마이그레이션 / 실행 순서 (권장)

1. `features/tools/index.tsx` 에서 안전한 이름 분해 로직을 먼저 적용(작은 범위) 및 테스트 업데이트
2. `ToolsModal`을 개선하여 출처 표시 및 대비 개선(시각적 회귀 확인)
3. `BrowserToolProvider`, `RustMCPToolProvider`, `WebMCPToolProvider`의 register/unregister 경로 점검 및 필요시 수정
4. `src/context/BuiltInToolContext.tsx`를 adapter로 전환하거나 deprecated 표시
5. 전체 lint / typecheck 및 관련 테스트 실행

## Rollout 및 검증

- 로컬: `pnpm install` (필요시), `pnpm -w test`, `pnpm lint`, `pnpm build` 실행 후 UI 점검

- UI 수동 검증: Chat -> ToolsModal 열어 `mcp`와 `builtin` 배지 확인, 대비 문제 시 토큰 조정

## 위험 및 회피 전략

- 위험: 도구 이름 포맷을 기대하던 기존 코드가 실패할 수 있음. 회피: 변경 전 기존 포맷을 사용하는 모든 call-site 검색 및 테스트 보강

- 위험: 스타일 토큰 변경이 앱 전역에 영향. 회피: 토큰을 직접 바꾸기보다 UI 컴포넌트에서 클래스 오버라이드로 우선 적용

## 일정(예상)

- 1일: 안전한 분해 로직 적용 + 관련 단위테스트 수정
- 1일: ToolsModal 및 UI 개선, 시각적 점검
- 1일: 프로바이더들(register/unregister) 수정 및 테스트 보강
- 0.5일: 전체 lint/typecheck + smoke build

---

작성: 프로젝트 리포지토리 규칙에 따라 `./docs/history/refactoring_{yyyyMMdd_hhmm}.md`에 상세 변경 로그를 추가할 예정입니다.

	- `listTools`가 반환하는 툴 메타는 단축(서비스 내부) 이름만 포함하도록 조정

- `src/features/tools/RustMCPToolProvider.tsx`, `WebMCPToolProvider.tsx`
	- 같은 방식으로 서비스 등록/언레지스터 확인

- `src/context/BuiltInToolContext.tsx`
	- (선택) deprecated 처리 또는 어댑터로 `features/tools`로 포워딩

- `src/features/tools/ToolsTestPage.tsx`, `src/features/tools/ToolsModal.tsx`
	- `split('__')` 사용처를 찾아 안전 분해로 교체
	- `ToolsModal`에 출처 배지(source)와 대비 개선

- 테스트 파일들: `src/features/tools/*.test.tsx`, `src/context/MCPServerContext.test.tsx` 등
	- 파싱 규칙/라우팅 변경에 따른 테스트 업데이트 및 추가

## 핵심 코드 스니펫(예시 변경안)

안전한 이름 분해(추천)

```tsx
const stripped = name.startsWith('builtin.') ? name.slice('builtin.'.length) : name;
const idx = stripped.indexOf('__');
if (idx === -1) throw new Error(`Invalid builtin tool name: ${stripped}`);
const serviceId = stripped.slice(0, idx);
const toolName = stripped.slice(idx + 2);
```

Browser 서비스 등록 예시

```tsx
const serviceId = 'browser';
const service: BuiltInService = {
	listTools: () => browserTools.map(({ name, description, inputSchema }) => ({ name, description, inputSchema })),
	executeTool: async (call) => { /* 안전한 인수 파싱 및 실행 */ },
};
register(serviceId, service);
// cleanup -> unregister(serviceId)
```

ToolsModal: 출처 배지 및 대비 개선(요약)

```tsx
// 병합: builtinTools.map(t => ({...t, source: 'builtin'})) + mcpTools.map(t => ({...t, source: 'mcp'}))
// badge: source === 'builtin' ? 'builtin' : 'mcp'
// 텍스트 토큰: text-foreground 대신 text-muted-foreground 사용 위치 점검
```

## 테스트

- 단위
	- 이름 파싱: `builtin.filesystem__list_directory`, `builtin.filesystem__name__with__underscores`, `builtin.invalidname`(에러)
	- BrowserToolProvider의 `register`/`unregister` 동작
	- ToolsModal에서 두 출처가 모두 보이는지

- 통합
	- 가짜 서비스 등록 후 `BuiltInToolProvider.executeTool` 경로 검증

## 마이그레이션 / 실행 순서 (권장)

1. `features/tools/index.tsx` 에서 안전한 이름 분해 로직을 먼저 적용(작은 범위) 및 테스트 업데이트
2. `ToolsModal`을 개선하여 출처 표시 및 대비 개선(시각적 회귀 확인)
3. `BrowserToolProvider`, `RustMCPToolProvider`, `WebMCPToolProvider`의 register/unregister 경로 점검 및 필요시 수정
4. `src/context/BuiltInToolContext.tsx`를 adapter로 전환하거나 deprecated 표시
5. 전체 lint / typecheck 및 관련 테스트 실행

## Rollout 및 검증

- 로컬: `pnpm install` (필요시), `pnpm -w test`, `pnpm lint`, `pnpm build` 실행 후 UI 점검
- UI 수동 검증: Chat -> ToolsModal 열어 `mcp`와 `builtin` 배지 확인, 대비 문제 시 토큰 조정

## 위험 및 회피 전략

- 위험: 도구 이름 포맷을 기대하던 기존 코드가 실패할 수 있음. 회피: 변경 전 기존 포맷을 사용하는 모든 call-site 검색 및 테스트 보강
- 위험: 스타일 토큰 변경이 앱 전역에 영향. 회피: 토큰을 직접 바꾸기보다 UI 컴포넌트에서 클래스 오버라이드로 우선 적용

## 일정(예상)

- 1일: 안전한 분해 로직 적용 + 관련 단위테스트 수정
- 1일: ToolsModal 및 UI 개선, 시각적 점검
- 1일: 프로바이더들(register/unregister) 수정 및 테스트 보강
- 0.5일: 전체 lint/typecheck + smoke build

---

작성: 프로젝트 리포지토리 규칙에 따라 `./docs/history/refactoring_{yyyyMMdd_hhmm}.md`에 상세 변경 로그를 추가할 예정입니다.

```
