# Mermaid 렌더링 지원 개발 기획서

> **작성일**: 2026-09-30
> **작성자**: Coding Expert
> **상태**: Implemented (MVP)

---

## 1. 현상 및 문제점 (AS-IS)

### 1.1 현재 Markdown 렌더링 아키텍처

LibrAgent의 메시지 렌더러(`AgentMessageRenderer`)는 다음 스택으로 구성된다:

```
react-markdown
├── remark-gfm        → GFM (tables, strikethrough, tasklists)
├── remark-math       → LaTeX 수식 구문 분석
└── rehype-katex      → KaTeX로 수식 렌더링
```

코드 블록은 `CodeBlock` 컴포넌트에서 `prism-react-renderer`로 문법 하이라이트 처리한다.

### 1.2 문제점

| # | 문제 | 영향도 | 비고 |
|---|------|--------|------|
| P1 | `language-mermaid` 블록이 일반 코드 블록으로 처리됨 | **높음** | Mermaid 소스 코드가 plain text로 노출, 다이어그램이 그려지지 않음 |
| P2 | Mermaid 렌더링 라이브러리 미존재 | **높음** | `package.json`에 mermaid 의존성 없음 |
| P3 | `CodeBlock` 컴포넌트가 언어별 분기 로직을 갖지 않음 | **중** | `language-mermaid` 감지 시 특별 처리 없음 |
| P4 | `ui__presentInteractive`의 HTML/Markdown 모드에도 동일 문제 | **중** | 수동으로 보낼 때에도 Mermaid가 안 그림 |
| P5 | Streaming 중 Mermaid 블록이 중간에 끊기면 렌더링 깨질 가능성 | **낮음** | `useStreamMarkdownPreprocess`가 mermaid를 고려하지 않음 |

### 1.3 기술적 제약

- **보안**: Mermaid는 클라이언트 측 JS로 SVG 생성 → XSS 위험이 존재함. 현재 URL sanitization(`isSafeExternalUrl`)은 있지만 Mermaid content sanitization은 없음.
- **성능**: 대용량 Mermaid 다이어그램 렌더링 시 메인 스레드 블로킹 가능성.
- **의존성**: `mermaid` npm 패키지는 약 300KB+ (gzip). lazy import 필요.

---

## 2. 목표 (TO-BE)

### 2.1 기능 목표

| 우선순위 | 목표 | 설명 |
|----------|------|------|
| P0 | 메시지 채팅에서 Mermaid 렌더링 | ` ```mermaid ` 블록이 SVG 다이어그램으로 자동 변환 |
| P0 | `ui__presentInteractive`에서 Mermaid 렌더링 | Markdown 모드에서 동일하게 동작 |
| P1 | Streaming 중 Mermaid 블록 감지 | 블록 완성 전까지 placeholder 표시, 완성 시 렌더링 |
| P1 | 수동 Mermaid 코드 블록도 지원 | ` ```mermaid ` 외에 ` ```md ` 등 다른 언어로 감춰진 Mermaid도 인식 |
| P2 | Dark/Light 테마 연동 | Mermaid 테마를 현재 UI 테마와 동기화 |
| P2 | 다이어그램 확대/축소 | SVG에 줌 인/아웃 기능 |

### 2.2 비기능 목표

- **보안**: Mermaid 콘텐츠에 대한 XSS 필터링 적용
- **성능**: Mermaid 라이브러리 lazy import → 번들 크기 증가 최소 150KB(gzip)
- **호환성**: 기존 `remark-math`/`rehype-katex` 렌더링에 영향 없음

---

## 3. 아키텍처 설계

### 3.1 전체 흐름

```
MarkdownContent
    │
    ▼
react-markdown (remark-gfm, remark-math)
    │
    ├─ code block 감지 ──→ CodeBlock 컴포넌트
    │                        │
    │                        ├─ language == 'mermaid' → <MermaidBlock />
    │                        │
    │                        └─ 기타 language → prism-react-renderer
    │
    └─ math block 감지 ──→ rehype-katex (기존 유지)
```

### 3.2 컴포넌트 구조

```
AgentMessageRenderer/
├── index.tsx                          # 메인 렌더러 (변경 없음)
├── config/markdown.tsx                # 플러그인 설정 (변경 없음)
├── components/
│   ├── CodeBlock.tsx                  # 코드 블록 라우터 (수정)
│   ├── MermaidBlock.tsx               # ⭐ 신규 - Mermaid 렌더링
│   └── MermaidBlockLoader.tsx         # ⭐ 신규 - lazy import wrapper
```

### 3.3 `MermaidBlock` 컴포넌트 설계

```tsx
// MermaidBlock.tsx (구현 요약)
// - lazy render via src/lib/mermaid/loader.ts (queue + theme once-init)
// - 120ms debounce; skeleton → SVG or error+source toggle
// - securityLevel: 'strict'; no source sanitize.ts
```

### 3.4 `CodeBlock` 수정 사항

```tsx
// CodeBlock.tsx (수정 부분)
const match = /language-(\w+)/.exec(className || '');
const language = match ? match[1] : '';

// ⭐ Mermaid 언어 감지
if (language === 'mermaid') {
  const code = String(children).replace(/\n$/, '');
  return <MermaidBlock code={code} isDark={isDark} />;
}

// 기존: 일반 코드 블록 (prism highlight)
```

### 3.5 Lazy Import 전략

```tsx
// MermaidBlockLoader.tsx
const { default: mermaid } = await import('mermaid');
```

`mermaid` 패키지는 300KB+이므로 `CodeBlock`과 동일 파일에 bundling하지 않고,
`language-mermaid` 코드가 최초로 감지될 때만 load한다.

---

## 4. 구현 상세

### 4.1 파일 변경 목록

| 파일 | 변경 타입 | 내용 |
|------|-----------|------|
| `vite.config.ts` | 수정 | `mermaid` → `mermaid.esm.min.mjs` alias (wasm/TLA 플러그인과의 `prototype` 충돌 회피) |
| `src/lib/mermaid/loader.ts` | **신규** | Lazy import + theme once-init + serial render queue |
| `src/features/agent/components/AgentMessageRenderer/components/MermaidBlock.tsx` | **신규** | Mermaid 렌더링 컴포넌트 (skeleton / error+source / debounce) |
| `src/features/agent/components/AgentMessageRenderer/components/CodeBlock.tsx` | **수정** | `language-mermaid` 감지 및 `MermaidBlock`으로 라우팅 |
| `src/features/agent/components/AgentMessageRenderer/components/__tests__/MermaidBlock.test.tsx` | **신규** | 단위 테스트 |
| `src/lib/mermaid/__tests__/loader.test.ts` | **신규** | queue / theme 단위 테스트 |

> **Note:** 초기 기획의 `src/lib/mermaid/sanitize.ts`는 **생성하지 않음**. Mermaid 노드 라벨에 `<>`가 쓰일 수 있어 소스 regex 살균은 다이어그램을 깨뜨리기 쉽고, `securityLevel: 'strict'`(+ mermaid 내부 DOMPurify)로 대체함.

### 4.2 XSS Sanitization

MVP는 **소스 사전 살균 없이** 다음만 사용한다:

```ts
mermaid.initialize({
  startOnLoad: false,
  securityLevel: 'strict',
  theme: isDark ? 'dark' : 'default',
});
```

`dangerouslySetInnerHTML`에는 mermaid가 반환한 SVG만 넣는다. 추가 SVG DOMPurify는 필요 시 P1.
### 4.3 Streaming 고려사항

`useStreamMarkdownPreprocess`는 현재 스트리밍 중일 때 코드 블록을 완성된 것으로 처리하는 로직이 있다.
Mermaid의 경우:

1. 블록 감지 (` ```mermaid ` 시작)
2. 스트리밍 중에는 `MermaidSkeleton` 표시
3. 블록 종료 (` ``` ` 감지) → `MermaidBlock` 렌더링 시작

이는 기존 `CodeBlock`의 `React.memo` 비교 로직과 호환된다.

### 4.4 테마 연동

Mermaid는 내장 테마(`default`, `dark`, `forest`, `neutral`)를 지원한다.
현재 `useIsDarkMode()` hook을 통해 테마 상태를 받아 `mermaid.initialize()`에 전달한다.

```ts
mermaid.initialize({
  theme: isDark ? 'dark' : 'default',
  securityLevel: 'strict',
});
```

---

## 5. 테스트 계획

### 5.1 단위 테스트

| 테스트 케이스 | 설명 |
|--------------|------|
| `MermaidBlock renders valid mermaid code` | 유효한 Mermaid 코드 렌더링 |
| `MermaidBlock shows error for invalid code` | 구문 오류 시 에러 UI 표시 |
| `MermaidBlock respects dark mode` | 테마 변경 시 리렌더링 |
| `MermaidBlock cleanup on unmount` | DOM leak 방지 |
| `CodeBlock routes mermaid language` | `language-mermaid` → `MermaidBlock` 라우팅 |
| `CodeBlock passes other languages to prism` | 기존 코드 블록 동작 유지 |

### 5.2 통합 테스트

| 테스트 케이스 | 설명 |
|--------------|------|
| `AgentMessageRenderer renders mermaid in message` | 실제 메시지 렌더링 통합 테스트 |
| `MermaidBlock lazy loads only on demand` | 번들 분리 확인 (Chrome DevTools) |

---

## 6. 예상 번들 크기 영향

| 항목 | 크기 (gzip) |
|------|-------------|
| `mermaid` 전체 | ~300KB |
| lazy import 시 초기 번들 증가 | ~0KB (첫 mermaid 코드 렌더링 시 ~150KB 추가) |
| 기존 `react-markdown` 등 | 영향 없음 |

---

## 7. 일정 산정

| 단계 | 작업 | 예상 시간 |
|------|------|-----------|
| 1 | `mermaid` 의존성 추가 + lazy loader 구현 | 1h |
| 2 | `MermaidBlock` 컴포넌트 구현 + sanitization | 2h |
| 3 | `CodeBlock` 수정 (라우팅) | 0.5h |
| 4 | 테마 연동 + skeleton UI | 1h |
| 5 | 테스트 코드 작성 | 2h |
| 6 | 검증 (lint, build, refactor:validate) | 1h |
| | **총계** | **~7.5h** |

---

## 8. 리스크 & 완화 전략

| 리스크 | 영향 | 완화 |
|--------|------|------|
| Mermaid 대용량 다이어그램 렌더링 지연 | UX 저하 | Skeleton 표시 + timeout 시 에러 fallback |
| `dangerouslySetInnerHTML` 보안 | XSS | `securityLevel: 'strict'` (소스 regex sanitizer 미사용) |
| Mermaid 초기화 충돌 (동시 렌더링) | 렌더링 실패 | 고유 ID 생성 + cleanup |
| 기존 remark-math/katex와 충돌 | 수식 깨짐 | mermaid는 remark/plug-in 레벨이 아닌 컴포넌트 레벨 처리 → 무해 |

---

## 9. 의사결정 기록

| 항목 | 선택 | 근거 |
|------|-------|------|
| Mermaid 렌더링 위치 | 컴포넌트 레벨 (CodeBlock 내) | remark/rehype 플러그인보다 단순, 기존 구조와 충돌 없음 |
| lazy import | `React.lazy` + `Suspense` | 번들 크기 최소화, 첫 렌더링 시에만 로드 |
| XSS 방어 | `securityLevel: 'strict'` only (no source `sanitize.ts`) | Mermaid 라벨의 `<>` 보존; mermaid 내부 DOMPurify 신뢰 |
| 테마 | mermaid 내장 `dark`/`default` | 외부 테마 연동 복잡도 회피, YAGNI |
| 줌 기능 | 미포함 | MVP에서는 불필요, 추후 요구 시 추가 |

---

## 10. 후속 작업 — PDF 고급 렌더링 (#1966)

| 항목 | 내용 |
|------|------|
| **이슈** | [#1966](https://github.com/fritzprix/libr-agent/issues/1966) |
| **Mermaid** | **(A)** 프론트 `prepareMarkdownForPdfExport` → SVG→PNG → `libragent-pdf-embed:N` + `download_text_pdf(embeddedImages)` → temp PNG → `markdown2pdf` |
| **LaTeX** | 프론트 치환 없음. `markdown2pdf` 내장 TeX (`$…$` / `$$…$$`) 활용 |
| **비고** | `.md` 내보내기는 소스 보존. 채팅 Mermaid MVP(#1964) 의존 |
