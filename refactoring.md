# Browser Tools Refactoring Plan

## 작업의 목적

- 신뢰성 향상: 성공률이 낮은 도구들의 안정성 개선
- 코드 중복 제거: `getPageContent`와 `extractStructuredContent`의 기능 통합
- 누락된 기본 기능 추가: 브라우저 네비게이션 기본 기능 보완
- 효율적인 데이터 추출: 전체 페이지 분석 없이 특정 정보만 추출하는 도구 제공

## 현재의 상태 / 문제점

### 1. 중복된 페이지 추출 도구

- `getPageContent`(Markdown 중심)와 `extractStructuredContent`(JSON 중심)가 유사한 기능 수행
- 코드 중복으로 인한 유지보수 비용 증가

### 2. 낮은 성공률의 상호작용 도구

- `clickElement`, `inputText`가 요소 상태 확인 없이 즉시 실행하여 타임아웃 빈발
- `elementExists`는 존재 여부만 확인하고 상호작용 가능 여부는 미확인

### 3. 누락된 기본 브라우저 기능

- 뒤로가기/앞으로가기 기능 없음
- 특정 요소의 텍스트나 속성만 조회하는 효율적 방법 부재

## 변경 이후의 상태 / 해결 판정 기준

### 1. 통합된 콘텐츠 추출

- 단일 `extractContent` 도구로 Markdown/JSON 형식 모두 지원
- 기본값은 토큰 효율적인 Markdown 형식

### 2. 신뢰성 높은 상호작용 체계

- `findElement`로 요소 상태 사전 확인
- 상호작용 도구들이 상태 확인 후 실행

### 3. 완전한 브라우저 제어

- 네비게이션 기본 기능 완비
- 효율적인 데이터 추출 도구 제공

## 수정이 필요한 코드 및 수정부분의 코드 스니핏

### 1. extractContent 통합 (BrowserToolProvider.tsx)

**제거할 기존 도구**:

```typescript
// 제거: getPageContent
// 제거: extractStructuredContent
// 제거: elementExists
```

**추가할 통합 도구**:

```typescript
{
  name: 'extractContent',
  description: 'Extracts page content as Markdown (default) or structured JSON. Saves raw HTML optionally.',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: { type: 'string', description: 'Browser session ID' },
      selector: { type: 'string', description: 'CSS selector to focus extraction (optional, defaults to body)' },
      format: { type: 'string', enum: ['markdown', 'json'], default: 'markdown' },
      saveRawHtml: { type: 'boolean', default: false },
      includeLinks: { type: 'boolean', default: true },
      maxDepth: { type: 'number', default: 5 }
    },
    required: ['sessionId']
  },
  execute: async (args: Record<string, unknown>) => {
    const { sessionId, selector = 'body', format = 'markdown', saveRawHtml = false, includeLinks = true, maxDepth = 5 } = args as {
      sessionId: string; selector?: string; format?: 'markdown' | 'json'; saveRawHtml?: boolean; includeLinks?: boolean; maxDepth?: number;
    };

    try {
      // Raw HTML 추출
      const rawHtml = await executeScript(sessionId, `document.querySelector('${selector}').outerHTML`);
      if (!rawHtml || typeof rawHtml !== 'string') {
        return JSON.stringify({ error: 'Failed to extract HTML from the page.' });
      }

      let result: any = {};

      if (format === 'markdown') {
        const turndownService = new TurndownService({
          headingStyle: 'atx', codeBlockStyle: 'fenced', bulletListMarker: '-', emDelimiter: '*'
        });
        turndownService.addRule('removeScripts', { filter: ['script', 'style', 'noscript'], replacement: () => '' });
        turndownService.addRule('preserveLineBreaks', { filter: 'br', replacement: () => '\n' });

        result.content = turndownService.turndown(rawHtml);
        result.format = 'markdown';
      } else {
        // JSON 구조화 로직 (기존 extractStructuredContent와 동일)
        const structuredScript = `/* JSON 추출 스크립트 */`;
        const jsonResult = await executeScript(sessionId, structuredScript);
        result = JSON.parse(jsonResult);
      }

      // Raw HTML 저장
      if (saveRawHtml) {
        const tempDir = 'temp_html';
        const uniqueId = `${Date.now()}-${Math.floor(Math.random() * 10000)}`;
        const relativePath = `${tempDir}/${uniqueId}.html`;
        const encoder = new TextEncoder();
        const htmlBytes = Array.from(encoder.encode(rawHtml));
        await writeFile(relativePath, htmlBytes);
        result.saved_raw_html = relativePath;
      }

      result.metadata = {
        extraction_timestamp: new Date().toISOString(),
        content_length: result.content?.length || 0,
        raw_html_size: rawHtml.length
      };

      return JSON.stringify(result, null, 2);
    } catch (error) {
      return JSON.stringify({ error: `Failed to extract content: ${error instanceof Error ? error.message : String(error)}` });
    }
  }
}
```

### 2. findElement 도구 추가

```typescript
{
  name: 'findElement',
  description: 'Find element and check its state (existence, visibility, interactability)',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: { type: 'string' },
      selector: { type: 'string' }
    },
    required: ['sessionId', 'selector']
  },
  execute: async (args: Record<string, unknown>) => {
    const { sessionId, selector } = args as { sessionId: string; selector: string };

    const script = `
(function() {
  const selector = '${selector.replace(/'/g, "\\'")}';
  try {
    const el = document.querySelector(selector);
    if (!el) return JSON.stringify({ exists: false, selector });

    const rect = el.getBoundingClientRect();
    const style = window.getComputedStyle(el);
    const visible = !!(rect.width > 0 && rect.height > 0 && style.display !== 'none' && style.visibility !== 'hidden');
    const clickable = visible && style.pointerEvents !== 'none' && !el.disabled;

    return JSON.stringify({
      exists: true,
      visible,
      clickable,
      tagName: el.tagName.toLowerCase(),
      rect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
      attributes: {
        id: el.id || null,
        className: el.className || null,
        disabled: el.disabled || false
      },
      selector
    });
  } catch (error) {
    return JSON.stringify({ exists: false, error: error.message, selector });
  }
})()`;

    return executeScript(sessionId, script);
  }
}
```

### 3. 네비게이션 도구 추가

```typescript
{
  name: 'navigateBack',
  description: 'Navigate back in browser history',
  inputSchema: { type: 'object', properties: { sessionId: { type: 'string' } }, required: ['sessionId'] },
  execute: async (args: Record<string, unknown>) => {
    const { sessionId } = args as { sessionId: string };
    return executeScript(sessionId, 'history.back(); "Navigated back"');
  }
},
{
  name: 'navigateForward',
  description: 'Navigate forward in browser history',
  inputSchema: { type: 'object', properties: { sessionId: { type: 'string' } }, required: ['sessionId'] },
  execute: async (args: Record<string, unknown>) => {
    const { sessionId } = args as { sessionId: string };
    return executeScript(sessionId, 'history.forward(); "Navigated forward"');
  }
}
```

### 4. 효율적 데이터 추출 도구 추가

```typescript
{
  name: 'getElementText',
  description: 'Get text content of a specific element',
  inputSchema: {
    type: 'object',
    properties: { sessionId: { type: 'string' }, selector: { type: 'string' } },
    required: ['sessionId', 'selector']
  },
  execute: async (args: Record<string, unknown>) => {
    const { sessionId, selector } = args as { sessionId: string; selector: string };
    const script = `
      const el = document.querySelector('${selector.replace(/'/g, "\\'")}');
      el ? el.textContent.trim() : null
    `;
    return executeScript(sessionId, script);
  }
},
{
  name: 'getElementAttribute',
  description: 'Get specific attribute value of an element',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: { type: 'string' },
      selector: { type: 'string' },
      attribute: { type: 'string' }
    },
    required: ['sessionId', 'selector', 'attribute']
  },
  execute: async (args: Record<string, unknown>) => {
    const { sessionId, selector, attribute } = args as { sessionId: string; selector: string; attribute: string };
    const script = `
      const el = document.querySelector('${selector.replace(/'/g, "\\'")}');
      el ? el.getAttribute('${attribute}') : null
    `;
    return executeScript(sessionId, script);
  }
}
```

### 5. clickElement 개선

```typescript
// 기존 clickElement를 개선하여 findElement 기반으로 상태 확인 후 클릭
execute: async (args: Record<string, unknown>) => {
  const { sessionId, selector } = args as { sessionId: string; selector: string };

  // 먼저 요소 상태 확인
  const elementState = await /* findElement 로직 */;
  const state = JSON.parse(elementState);

  if (!state.exists) {
    return formatBrowserResult(JSON.stringify({ ok: false, action: 'click', reason: 'element_not_found', selector }));
  }

  if (!state.clickable) {
    return formatBrowserResult(JSON.stringify({ ok: false, action: 'click', reason: 'element_not_clickable', selector, diagnostics: state }));
  }

  // 기존 클릭 로직 수행
  // ... 기존 rbClickElement 로직
}
```

이 계획에 따라 단계별로 구현하면 도구의 신뢰성과 효율성을 크게 향상시킬 수 있습니다.
