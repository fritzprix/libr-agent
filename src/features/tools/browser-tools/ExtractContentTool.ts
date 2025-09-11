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
import { writeFile } from '@/lib/rust-backend-client';

const logger = getLogger('ExtractPageContentTool');

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
function validateExtractPageContentArgs(
  args: Record<string, unknown>,
): ValidatedArgs | null {
  logger.debug('Validating extractPageContent args:', args);

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

  logger.debug('Validation successful', {
    sessionId: args.sessionId,
    saveRawHtml,
  });

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
    `document.querySelector("body").outerHTML`,
  );

  if (!rawHtml || typeof rawHtml !== 'string') {
    throw new Error(
      'Failed to extract HTML from the page - no content found or invalid content type',
    );
  }

  return rawHtml;
}

// 메타데이터 생성 함수
function createMetadata(
  result: ConversionResult,
  rawHtml: string,
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
    },
  };
}

// 응답 텍스트 생성 함수
function generateResponseText(result: ConversionResult): string {
  let baseContent: string;

  if (typeof result.content === 'string') {
    baseContent = result.content;
  } else {
    const contentToStringify = result.content || result.domMap;
    baseContent = JSON.stringify(contentToStringify);
  }

  // Raw HTML 저장 경로가 있는 경우 추가 정보 포함
  if (result.raw_html_path) {
    const additionalInfo = `\n\n--- File Save Information ---\nRaw HTML saved to: ${result.raw_html_path}`;
    return baseContent + additionalInfo;
  }

  // 저장 실패한 경우 에러 정보 포함
  if (result.save_html_error) {
    const errorInfo = `\n\n--- File Save Error ---\n${result.save_html_error}`;
    return baseContent + errorInfo;
  }

  return baseContent;
}

export const extractPageContentTool: StrictBrowserMCPTool = {
  name: 'extractPageContent',
  description:
    'Convert the entire webpage into clean, readable markdown format. Extracts the main textual content while removing navigation, ads, scripts, and formatting noise. Ideal for content analysis, summarization, and reading.',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: BROWSER_TOOL_SCHEMAS.sessionId,
      saveRawHtml: {
        type: 'boolean',
        description:
          'Save the raw HTML to a file for debugging purposes. Default: false',
      },
    },
    required: ['sessionId'],
  },
  execute: async (args: Record<string, unknown>, executeScript) => {
    // 인자 검증
    const validatedArgs = validateExtractPageContentArgs(args);
    if (!validatedArgs) {
      return createMCPErrorResponse(
        -32602,
        'Invalid arguments provided - check sessionId type and other parameter types',
        { toolName: 'extractPageContent', args },
        createId(),
      );
    }

    const { sessionId, saveRawHtml } = validatedArgs;

    logger.debug('Executing browser_extractPageContent', {
      sessionId,
    });

    // executeScript 함수 존재 검증
    if (!executeScript) {
      return createMCPErrorResponse(
        -32603,
        'executeScript function is required for extractPageContent',
        { toolName: 'extractPageContent', args },
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
          -32603,
          `Content conversion failed: ${conversionError instanceof Error ? conversionError.message : String(conversionError)}`,
          { toolName: 'extractContent', args },
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
          await writeFile(relativePath, contentBytes);

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

      // 메타데이터 추가
      const resultWithMetadata = createMetadata(result, rawHtml);

      // 응답 생성
      const textContent = generateResponseText(result);

      return createMCPStructuredResponse(
        textContent,
        resultWithMetadata,
        createId(),
      );
    } catch (error) {
      logger.error('Error in browser_extractPageContent:', {
        error,
        sessionId,
      });
      return createMCPErrorResponse(
        -32603,
        `Failed to extract page content: ${error instanceof Error ? error.message : String(error)}`,
        { toolName: 'extractPageContent', args },
        createId(),
      );
    }
  },
};
