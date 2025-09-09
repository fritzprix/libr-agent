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

const logger = getLogger('ExtractContentTool');

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
        description: 'Output format (default: dom-map)',
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
    const {
      sessionId,
      selector = 'body',
      format = 'markdown',
      saveRawHtml = true,
      includeLinks = true,
      maxDepth = 5,
    } = args as {
      sessionId: string;
      selector?: string;
      format?: 'markdown' | 'json' | 'dom-map';
      saveRawHtml?: boolean;
      includeLinks?: boolean;
      maxDepth?: number;
    };

    logger.debug('Executing browser_extractContent', {
      sessionId,
      selector,
      format,
    });

    if (!executeScript) {
      throw new Error('executeScript function is required for extractContent');
    }

    try {
      // Extract Raw HTML from specified selector
      const rawHtml = await executeScript(
        sessionId,
        `document.querySelector('${selector.replace(/'/g, "\\'")}').outerHTML`,
      );
      if (!rawHtml || typeof rawHtml !== 'string') {
        return createMCPErrorResponse(
          -32603,
          'Failed to extract HTML from the page.',
          { toolName: 'extractContent', args },
          createId(),
        );
      }

      let result: Record<string, unknown> = {};

      if (format === 'markdown') {
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

        result.content = turndownService.turndown(rawHtml);
        result.format = 'markdown';
      } else if (format === 'json') {
        // JSON structured extraction using TypeScript parser
        const structuredResult = parseHTMLToStructured(rawHtml, {
          maxDepth,
          includeLinks,
          maxTextLength: 1000,
        });

        if (structuredResult.error) {
          return createMCPErrorResponse(
            -32603,
            `Failed to parse HTML: ${structuredResult.error}`,
            { toolName: 'extractContent', args },
            createId(),
          );
        }

        result = {
          title: structuredResult.metadata.title,
          url: structuredResult.metadata.url,
          timestamp: structuredResult.metadata.timestamp,
          content: structuredResult.content,
          format: 'json',
        };
      } else if (format === 'dom-map') {
        // DOM Map extraction using TypeScript parser
        const domMapResult = parseHTMLToDOMMap(rawHtml, {
          maxDepth: Math.min(maxDepth, 3), // Limit DOM map depth for performance
          maxChildren: 10,
          maxTextLength: 50,
          includeInteractiveOnly: true,
        });

        if (domMapResult.error) {
          return createMCPErrorResponse(
            -32603,
            `Failed to create DOM map: ${domMapResult.error}`,
            { toolName: 'extractContent', args },
            createId(),
          );
        }

        result = domMapResult as unknown as Record<string, unknown>;
      }

      // Note: Save Raw HTML functionality will be handled by the main provider
      // since writeFile needs to be injected from useRustBackend hook
      if (saveRawHtml) {
        result.raw_html_content = rawHtml; // Include raw HTML in result for provider to handle
        result.save_html_requested = true;
      }

      // Add metadata if not already present (dom-map format already includes metadata)
      if (!result.metadata) {
        result.metadata = {
          extraction_timestamp: new Date().toISOString(),
          content_length:
            typeof result.content === 'string' ? result.content.length : 0,
          raw_html_size: rawHtml.length,
          selector: selector,
          format: format,
        };
      }

      // Return structured MCP response
      const textContent =
        typeof result.content === 'string'
          ? result.content
          : JSON.stringify(result.content || result.domMap, null, 2);

      return createMCPStructuredResponse(textContent, result, createId());
    } catch (error) {
      logger.error('Error in browser_extractContent:', error);
      return createMCPErrorResponse(
        -32603, // Internal error
        `Failed to extract content: ${error instanceof Error ? error.message : String(error)}`,
        { toolName: 'extractContent', args },
        createId(),
      );
    }
  },
};
