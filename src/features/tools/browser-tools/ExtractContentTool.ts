import { getLogger } from '@/lib/logger';
import { BROWSER_TOOL_SCHEMAS } from './helpers';
import { BrowserLocalMCPTool } from './types';
import TurndownService from 'turndown';

const logger = getLogger('ExtractContentTool');

export const extractContentTool: BrowserLocalMCPTool = {
  name: 'extractContent',
  description:
    'Extracts page content as Markdown (default) or structured JSON. Saves raw HTML optionally.',
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
        enum: ['markdown', 'json'],
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
      format?: 'markdown' | 'json';
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
        return JSON.stringify({
          error: 'Failed to extract HTML from the page.',
        });
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
      } else {
        // JSON structured extraction
        const script = `
(function() {
  function extractStructuredContent(element, depth = 0, maxDepth = ${maxDepth}) {
    if (depth > maxDepth || !element) return null;
    
    // Skip script, style, and hidden elements
    if (['SCRIPT', 'STYLE', 'NOSCRIPT', 'META', 'LINK', 'HEAD'].includes(element.tagName)) {
      return null;
    }
    
    // Check if element is visible
    const style = window.getComputedStyle(element);
    if (style.display === 'none' || style.visibility === 'hidden') {
      return null;
    }
    
    const result = {
      tag: element.tagName.toLowerCase(),
      text: '',
      children: []
    };
    
    // Add meaningful attributes
    if (element.id) result.id = element.id;
    if (element.className && typeof element.className === 'string') {
      result.class = element.className.trim();
    }
    if (${includeLinks} && element.href) result.href = element.href;
    if (element.src) result.src = element.src;
    if (element.alt) result.alt = element.alt;
    if (element.title) result.title = element.title;
    
    // Extract text content
    let textContent = '';
    for (let child of element.childNodes) {
      if (child.nodeType === Node.TEXT_NODE) {
        const text = child.textContent.trim();
        if (text) textContent += text + ' ';
      }
    }
    if (textContent.trim()) {
      result.text = textContent.trim();
    }
    
    // Process child elements
    const childElements = Array.from(element.children);
    for (let child of childElements) {
      const childResult = extractStructuredContent(child, depth + 1, maxDepth);
      if (childResult) {
        result.children.push(childResult);
      }
    }
    
    // If no text and no meaningful children, return null unless it's a meaningful element
    const meaningfulElements = ['a', 'button', 'input', 'img', 'video', 'audio', 'iframe', 'form', 'table'];
    if (!result.text && result.children.length === 0 && !meaningfulElements.includes(result.tag)) {
      return null;
    }
    
    return result;
  }
  
  const targetElement = document.querySelector('${selector.replace(/'/g, "\\'")}');
  if (!targetElement) {
    return { error: 'Element not found with selector: ${selector}' };
  }
  
  const structured = extractStructuredContent(targetElement);
  
  // Add page metadata
  const pageInfo = {
    title: document.title,
    url: window.location.href,
    timestamp: new Date().toISOString(),
    content: structured
  };
  
  return pageInfo;
})()`;

        const jsonResult = await executeScript(sessionId, script);
        try {
          result = JSON.parse(jsonResult);
        } catch {
          result = { content: jsonResult, format: 'json' };
        }
      }

      // Note: Save Raw HTML functionality will be handled by the main provider
      // since writeFile needs to be injected from useRustBackend hook
      if (saveRawHtml) {
        result.raw_html_content = rawHtml; // Include raw HTML in result for provider to handle
        result.save_html_requested = true;
      }

      result.metadata = {
        extraction_timestamp: new Date().toISOString(),
        content_length:
          typeof result.content === 'string' ? result.content.length : 0,
        raw_html_size: rawHtml.length,
        selector: selector,
        format: format,
      };

      return JSON.stringify(result, null, 2);
    } catch (error) {
      logger.error('Error in browser_extractContent:', error);
      return JSON.stringify({
        error: `Failed to extract content: ${error instanceof Error ? error.message : String(error)}`,
      });
    }
  },
};
