import { getLogger } from '@/lib/logger';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { StrictBrowserMCPTool } from './types';
import {
  createMCPStructuredResponse,
  createMCPErrorResponse,
} from '@/lib/mcp-response-utils';
import { createId } from '@paralleldrive/cuid2';
import TurndownService from 'turndown';
import { cleanMarkdownText } from '@/lib/text-utils';
import { workspaceWriteFile } from '@/lib/rust-backend-client';
import { ContentStore } from './content-store';

const logger = getLogger('ExtractContentTool');

// 타입 정의
interface ValidatedArgs {
  sessionId: string;
  saveRawHtml: boolean;
}

interface ConversionResult {
  content?: string | unknown;
  domMap?: unknown;
  title?: string;
  url?: string;
  timestamp?: string;
  format: string;
  [key: string]: unknown;
}

// 타입 검증 함수
function validateExtractContentArgs(
  args: Record<string, unknown>,
): ValidatedArgs | null {
  logger.debug('Validating extractWebContent args:', args);

  if (typeof args.sessionId !== 'string') {
    logger.warn('Invalid sessionId type', {
      sessionId: args.sessionId,
      type: typeof args.sessionId,
    });
    return null;
  }

  const saveRawHtml = args.saveRawHtml ?? false;
  if (typeof saveRawHtml !== 'boolean') {
    logger.warn('Invalid saveRawHtml type', {
      saveRawHtml: args.saveRawHtml,
      type: typeof args.saveRawHtml,
    });
    return null;
  }

  return {
    sessionId: args.sessionId,
    saveRawHtml,
  };
}

// 마크다운 변환 함수
function convertToMarkdown(rawHtml: string): ConversionResult {
  const turndownService = new TurndownService({
    headingStyle: 'atx',
    codeBlockStyle: 'fenced',
    bulletListMarker: '-',
    emDelimiter: '*',
  });

  turndownService.addRule('removeScripts', {
    filter: ['script', 'style', 'noscript'],
    replacement: () => '',
  });

  turndownService.addRule('preserveLineBreaks', {
    filter: 'br',
    replacement: () => '\n',
  });

  const markdown = cleanMarkdownText(turndownService.turndown(rawHtml));

  return {
    content: markdown,
    format: 'markdown',
  };
}

// 변환 실행 (markdown only)
function executeConversion(rawHtml: string): ConversionResult {
  return convertToMarkdown(rawHtml);
}

// HTML 추출 함수 (항상 body 기준)
async function extractHtmlFromPage(
  executeScript: (sessionId: string, script: string) => Promise<unknown>,
  sessionId: string,
): Promise<string> {
  const rawHtml = await executeScript(
    sessionId,
    `document.body ? document.body.outerHTML : ""`,
  );

  if (typeof rawHtml !== 'string') {
    throw new Error(
      'Failed to extract HTML from the page - invalid content type',
    );
  }

  return rawHtml;
}

// 메타데이터 생성 함수
function createMetadata(
  result: ConversionResult,
  rawHtml: string,
  totalPages: number,
): Record<string, unknown> {
  if (result.metadata) {
    return result;
  }

  return {
    ...result,
    metadata: {
      extraction_timestamp: new Date().toISOString(),
      content_length:
        typeof result.content === 'string' ? result.content.length : 0,
      raw_html_size: rawHtml.length,
      selector: 'body',
      format: 'markdown',
      total_pages: totalPages,
    },
  };
}

export const extractWebContentTool: StrictBrowserMCPTool = {
  name: 'extractWebContent',
  description:
    'Convert the webpage into clean, readable markdown format. Returns the first page of content and the total number of pages. Use readWebContent to access subsequent pages. Ideal for content analysis, summarization, and reading.',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: BROWSER_TOOL_SCHEMAS.sessionId,
      saveRawHtml: {
        type: 'boolean',
        description:
          'Save the raw HTML to a file for DOM structure analysis. Default: false',
      },
    },
    required: ['sessionId'],
  },
  execute: async (args: Record<string, unknown>, executeScript) => {
    // 인자 검증
    const validatedArgs = validateExtractContentArgs(args);
    if (!validatedArgs) {
      return createMCPErrorResponse(
        'Invalid arguments provided - check sessionId type and other parameter types',
        -32602,
        { toolName: 'extractWebContent', args },
        createId(),
      );
    }

    const { sessionId, saveRawHtml } = validatedArgs;

    logger.debug('Executing browser_extractWebContent', {
      sessionId,
    });

    // executeScript 함수 존재 검증
    if (!executeScript) {
      return createMCPErrorResponse(
        'executeScript function is required for extractWebContent',
        -32603,
        { toolName: 'extractWebContent', args },
        createId(),
      );
    }

    try {
      // HTML 추출 (body 기준)
      const rawHtml = await extractHtmlFromPage(executeScript, sessionId);

      // 마크다운 변환
      let result: ConversionResult;
      try {
        result = executeConversion(rawHtml);
      } catch (conversionError) {
        logger.error('Content conversion failed', {
          error: conversionError,
          htmlSize: rawHtml.length,
        });
        return createMCPErrorResponse(
          `Content conversion failed: ${conversionError instanceof Error ? conversionError.message : String(conversionError)}`,
          -32603,
          { toolName: 'extractWebContent', args },
          createId(),
        );
      }

      // Raw HTML 저장 요청 처리
      if (saveRawHtml) {
        const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
        const fileName = `extracted-${sessionId}-${timestamp}.html`;
        const relativePath = `extracted-content/${fileName}`;

        try {
          // 문자열을 바이트 배열로 변환
          const encoder = new TextEncoder();
          const contentBytes = Array.from(encoder.encode(rawHtml));

          // 기존 writeFile 인터페이스 사용
          await workspaceWriteFile(relativePath, contentBytes);

          result.raw_html_path = relativePath;
          result.save_html_requested = true;
        } catch (error) {
          logger.error('Failed to save raw HTML file', {
            error,
            path: relativePath,
          });
          result.save_html_error = `Failed to save raw HTML: ${error instanceof Error ? error.message : String(error)}`;
        }
      }

      // Pagination 처리
      let contentText = '';
      if (typeof result.content === 'string') {
        contentText = result.content;
      } else {
        contentText = JSON.stringify(result.content || result.domMap);
      }

      const { totalPages, firstPage } = ContentStore.saveContent(
        sessionId,
        contentText,
      );

      // 메타데이터 추가
      const resultWithMetadata = createMetadata(result, rawHtml, totalPages);

      // 응답 텍스트 생성
      let responseText = `[Page 1/${totalPages}]\n\n${firstPage}`;

      // 빈 페이지 감지 및 경고 메시지 추가
      if (!responseText.trim() || !firstPage.trim()) {
        responseText += `\n\n(Empty Page) The extracted content is empty. This suggests the page might not have loaded correctly or contains no text. Please try calling 'extractWebContent' again to re-capture the page, or use 'extractWebContent' with 'saveRawHtml': true to save the raw HTML for inspection.`;
      }

      if (totalPages > 1) {
        responseText += `\n\n--- End of Page 1 ---\nThere are ${totalPages} pages in total. Use readWebContent(sessionId, page) to read more.`;
      }

      // Raw HTML 저장 정보 추가
      if (result.raw_html_path) {
        responseText += `\n\n--- File Save Information ---\nRaw HTML saved to: ${result.raw_html_path}`;
      }
      if (result.save_html_error) {
        responseText += `\n\n--- File Save Error ---\n${result.save_html_error}`;
      }

      return createMCPStructuredResponse(
        responseText,
        resultWithMetadata,
        createId(),
      );
    } catch (error) {
      logger.error('Error in browser_extractWebContent:', {
        error,
        sessionId,
      });
      return createMCPErrorResponse(
        `Failed to extract page content: ${error instanceof Error ? error.message : String(error)}`,
        -32603,
        { toolName: 'extractWebContent', args, error },
        createId(),
      );
    }
  },
};
