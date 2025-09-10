import { getLogger } from '@/lib/logger';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { BrowserLocalMCPTool } from './types';
import {
  createMCPStructuredResponse,
  createMCPErrorResponse,
} from '@/lib/mcp-response-utils';
import { createId } from '@paralleldrive/cuid2';
import TurndownService from 'turndown';
import { parseHTMLToStructured, parseHTMLToDOMMap } from '@/lib/html-parser';
import { cleanMarkdownText } from '@/lib/text-utils';

const logger = getLogger('ExtractContentTool');

// 타입 정의
interface ValidatedArgs {
  sessionId: string;
  selector: string;
  format: 'markdown' | 'json' | 'dom-map';
  saveRawHtml: boolean;
  includeLinks: boolean;
  maxDepth: number;
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

// 타입 검증 함수 (타입캐스팅 제거)
function validateExtractContentArgs(
  args: Record<string, unknown>,
): ValidatedArgs | null {
  logger.debug('Validating extractContent args:', args);

  if (typeof args.sessionId !== 'string') {
    logger.warn('Invalid sessionId type', {
      sessionId: args.sessionId,
      type: typeof args.sessionId,
    });
    return null;
  }

  const selector = args.selector ?? 'body';
  if (typeof selector !== 'string') {
    logger.warn('Invalid selector type', { selector, type: typeof selector });
    return null;
  }

  const format = args.format ?? 'markdown';
  if (
    typeof format !== 'string' ||
    !['markdown', 'json', 'dom-map'].includes(format)
  ) {
    logger.warn('Invalid format', { format, type: typeof format });
    return null;
  }

  // saveRawHtml 명시적 타입 검증
  let saveRawHtml: boolean;
  if (args.saveRawHtml === undefined || args.saveRawHtml === null) {
    saveRawHtml = true;
  } else if (typeof args.saveRawHtml === 'boolean') {
    saveRawHtml = args.saveRawHtml;
  } else {
    logger.warn('Invalid saveRawHtml type, using default', {
      saveRawHtml: args.saveRawHtml,
    });
    saveRawHtml = true;
  }

  // includeLinks 명시적 타입 검증
  let includeLinks: boolean;
  if (args.includeLinks === undefined || args.includeLinks === null) {
    includeLinks = true;
  } else if (typeof args.includeLinks === 'boolean') {
    includeLinks = args.includeLinks;
  } else {
    logger.warn('Invalid includeLinks type, using default', {
      includeLinks: args.includeLinks,
    });
    includeLinks = true;
  }

  // maxDepth 명시적 타입 검증
  let maxDepth: number;
  if (args.maxDepth === undefined || args.maxDepth === null) {
    maxDepth = 5;
  } else if (
    typeof args.maxDepth === 'number' &&
    args.maxDepth >= 1 &&
    args.maxDepth <= 20
  ) {
    maxDepth = args.maxDepth;
  } else {
    logger.warn('Invalid maxDepth, using default', { maxDepth: args.maxDepth });
    maxDepth = 5;
  }

  logger.debug('Validation successful', {
    sessionId: args.sessionId,
    selector,
    format,
    saveRawHtml,
    includeLinks,
    maxDepth,
  });

  return {
    sessionId: args.sessionId,
    selector,
    format: format as 'markdown' | 'json' | 'dom-map',
    saveRawHtml,
    includeLinks,
    maxDepth,
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

// JSON 변환 함수
function convertToJson(
  rawHtml: string,
  options: { maxDepth: number; includeLinks: boolean },
): ConversionResult {
  const structuredResult = parseHTMLToStructured(rawHtml, {
    maxDepth: options.maxDepth,
    includeLinks: options.includeLinks,
    maxTextLength: 1000,
  });

  if (structuredResult.error) {
    throw new Error(`Failed to parse HTML: ${structuredResult.error}`);
  }

  return {
    title: structuredResult.metadata.title,
    url: structuredResult.metadata.url,
    timestamp: structuredResult.metadata.timestamp,
    content: structuredResult.content,
    format: 'json',
  };
}

// DOM Map 변환 함수
function convertToDomMap(rawHtml: string, maxDepth: number): ConversionResult {
  const domMapResult = parseHTMLToDOMMap(rawHtml, {
    maxDepth: Math.min(maxDepth, 10),
    maxChildren: 20,
    maxTextLength: 100,
    includeInteractiveOnly: false,
  });

  if (domMapResult.error) {
    throw new Error(`Failed to create DOM map: ${domMapResult.error}`);
  }

  return domMapResult as ConversionResult;
}

// 포맷별 변환 실행
function executeConversion(
  format: 'markdown' | 'json' | 'dom-map',
  rawHtml: string,
  options: { maxDepth: number; includeLinks: boolean },
): ConversionResult {
  switch (format) {
    case 'markdown':
      return convertToMarkdown(rawHtml);
    case 'json':
      return convertToJson(rawHtml, options);
    case 'dom-map':
      return convertToDomMap(rawHtml, options.maxDepth);
    default:
      throw new Error(`Unsupported format: ${format}`);
  }
}

// HTML 추출 함수
async function extractHtmlFromPage(
  executeScript: (sessionId: string, script: string) => Promise<unknown>,
  sessionId: string,
  selector: string,
): Promise<string> {
  const rawHtml = await executeScript(
    sessionId,
    `document.querySelector('${selector.replace(/'/g, "\\'")}').outerHTML`,
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
  selector: string,
  format: string,
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
      selector,
      format,
    },
  };
}

// 응답 텍스트 생성 함수
function generateResponseText(result: ConversionResult): string {
  if (typeof result.content === 'string') {
    return result.content;
  }

  const contentToStringify = result.content || result.domMap;
  return JSON.stringify(contentToStringify, null, 2);
}

export const extractContentTool: BrowserLocalMCPTool = {
  name: 'extractContent',
  description:
    'Extracts page content as DOM Map (default) or structured JSON/Markdown. Saves raw HTML optionally.',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: BROWSER_TOOL_SCHEMAS.sessionId,
      selector: {
        type: 'string',
        description:
          'CSS selector to focus extraction (optional, defaults to body)',
      },
      format: {
        type: 'string',
        enum: ['markdown', 'json', 'dom-map'],
        description: 'Output format (default: markdown)',
      },
      saveRawHtml: {
        type: 'boolean',
        description: 'Save raw HTML to file (default: false)',
      },
      includeLinks: {
        type: 'boolean',
        description: 'Include href attributes from links (default: true)',
      },
      maxDepth: {
        type: 'number',
        description: 'Maximum nesting depth for JSON extraction (default: 5)',
      },
    },
    required: ['sessionId'],
  },
  execute: async (args: Record<string, unknown>, executeScript) => {
    // 인자 검증
    const validatedArgs = validateExtractContentArgs(args);
    if (!validatedArgs) {
      return createMCPErrorResponse(
        -32602,
        'Invalid arguments provided - check sessionId type and other parameter types',
        { toolName: 'extractContent', args },
        createId(),
      );
    }

    const { sessionId, selector, format, saveRawHtml, includeLinks, maxDepth } =
      validatedArgs;

    logger.debug('Executing browser_extractContent', {
      sessionId,
      selector,
      format,
    });

    // executeScript 함수 존재 검증
    if (!executeScript) {
      return createMCPErrorResponse(
        -32603,
        'executeScript function is required for extractContent',
        { toolName: 'extractContent', args },
        createId(),
      );
    }

    try {
      // HTML 추출
      const rawHtml = await extractHtmlFromPage(
        executeScript,
        sessionId,
        selector,
      );

      // 포맷별 변환
      let result: ConversionResult;
      try {
        result = executeConversion(format, rawHtml, { maxDepth, includeLinks });
      } catch (conversionError) {
        logger.error('Content conversion failed', {
          error: conversionError,
          format,
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
        result.raw_html_content = rawHtml;
        result.save_html_requested = true;
      }

      // 메타데이터 추가
      const resultWithMetadata = createMetadata(
        result,
        rawHtml,
        selector,
        format,
      );

      // 응답 생성
      const textContent = generateResponseText(result);

      return createMCPStructuredResponse(
        textContent,
        resultWithMetadata,
        createId(),
      );
    } catch (error) {
      logger.error('Error in browser_extractContent:', {
        error,
        sessionId,
        selector,
        format,
      });
      return createMCPErrorResponse(
        -32603,
        `Failed to extract content: ${error instanceof Error ? error.message : String(error)}`,
        { toolName: 'extractContent', args },
        createId(),
      );
    }
  },
};
