# Refactoring Plan: browser_getPageContent 기능 개선 (v2) - ✅ COMPLETED

**Status**: ✅ **COMPLETED** (2025-08-23 22:40)  
**Completion Document**: `refactoring_20250823_2240_COMPLETED.md`

## 작업의 목적

AI 에이전트가 웹페이지의 실제 콘텐츠를 효율적으로 활용할 수 있도록,  
불필요한 HTML 노이즈를 제거하고 Markdown 변환 및 원본 HTML의 안전한 저장 경로를 함께 제공하는 기능으로 개선한다.

## 현재의 상태 / 문제점

- 프론트엔드 `browser_getPageContent` 도구는 전체 Raw HTML을 반환하여, AI 분석에 불필요한 정보가 많음.
- HTML에는 스크립트, 스타일, 광고, 내비게이션 등 노이즈가 다량 포함되어 있어 LLM API에 비효율적임.
- Raw HTML을 파일로 안전하게 저장하는 표준화된 방법이 없고, 경로 검증 및 보안 정책이 일관되지 않음.

## 변경 이후의 상태 / 해결 판정 기준

- 기존 `browser_getPageContent` 도구를 개선하여, injection script로부터 Raw HTML만 추출한다.
- 추출된 Raw HTML을 프론트엔드에서 Turndown 라이브러리를 사용해 Markdown으로 변환한다.
- 변환된 Markdown과 원본 Raw HTML은 모두 프론트엔드에서 안전하게 파일로 저장된다.
- 파일 저장/읽기는 SecureFileManager를 통해 Tauri 커맨드(`write_file`, `read_file`)로 일관되게 처리된다.
- 응답에는 Markdown 변환 결과(`content`)와 저장된 Raw HTML의 상대 경로(`saved_raw_html`)가 JSON으로 포함된다.
- 프론트엔드에서는 useRustBackend의 파일 API만 사용하여, MCP와 커맨드 모두 동일한 방식으로 파일을 다룬다.

## 수정이 필요한 코드 및 수정부분의 코드 스니핏

### 1. 프론트엔드: BrowserToolProvider.tsx에서 Raw HTML 추출 및 Turndown 변환

```typescript
import { useRustBackend } from '@/hooks/use-rust-backend';
const { writeFile } = useRustBackend();

{
  name: 'browser_getPageContent',
  description: 'Extracts clean content from the page as Markdown, and saves the raw HTML to a temporary file for reference.',
  execute: async (args: Record<string, unknown>) => {
    // 1. Raw HTML만 injection script로 추출
    const rawHtml = await executeScript(sessionId, 'document.documentElement.outerHTML');
    if (!rawHtml || typeof rawHtml !== 'string') {
      return JSON.stringify({ error: "Failed to get raw HTML from the page." });
    }

    // 2. Turndown으로 Markdown 변환 (프론트엔드에서 처리)
    import('turndown').then(({ default: TurndownService }) => {
      const turndownService = new TurndownService({ headingStyle: 'atx', codeBlockStyle: 'fenced' });
      const markdownContent = turndownService.turndown(rawHtml);

      // 3. Raw HTML 파일 저장
      const tempDir = 'temp_html';
      const uniqueId = `${Date.now()}-${Math.floor(Math.random() * 10000)}`;
      const relativePath = `${tempDir}/${uniqueId}.html`;
      writeFile(relativePath, new TextEncoder().encode(rawHtml));

      // 4. 결과 반환
      return JSON.stringify({
        content: markdownContent,
        saved_raw_html: relativePath,
      }, null, 2);
    });
  },
}
```

### 2. 백엔드: SecureFileManager를 통한 파일 저장/읽기 (이미 구현됨)

- Tauri 커맨드(`write_file`, `read_file`)는 SecureFileManager를 통해 파일 시스템에 접근한다.
- MCP 서버에서도 SecureFileManager를 활용하여 파일 관련 도구를 구현한다.
