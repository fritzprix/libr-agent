import { useEffect, useRef } from 'react';
import { useBuiltInTool } from '.';
import { useBrowserInvoker } from '@/hooks/use-browser-invoker';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { MCPTool, MCPResponse } from '@/lib/mcp-types';
import { getLogger } from '@/lib/logger';
import {
  createBrowserSession,
  closeBrowserSession,
  listBrowserSessions,
  clickElement as rbClickElement,
  inputText as rbInputText,
  pollScriptResult as rbPollScriptResult,
  navigateToUrl as rbNavigateToUrl,
} from '@/lib/rust-backend-client';
import TurndownService from 'turndown';
import { ToolCall } from '@/models/chat';

const logger = getLogger('BrowserToolProvider');

// Extended MCPTool type that includes execute function
type LocalMCPTool = MCPTool & {
  execute: (args: Record<string, unknown>) => Promise<unknown>;
};

/**
 * Formats browser tool results that return JSON envelopes
 * Tries to parse JSON and format diagnostics nicely, falls back to raw text
 */
function formatBrowserResult(raw: unknown): string {
  if (typeof raw === 'string') {
    try {
      const parsed = JSON.parse(raw);
      if ('ok' in parsed) {
        if (parsed.ok) {
          let result = `✓ ${parsed.action.toUpperCase()} successful (selector: ${parsed.selector})`;
          if (parsed.diagnostics) {
            result += `\n\nDiagnostics:\n${JSON.stringify(parsed.diagnostics, null, 2)}`;
          }
          if (parsed.value_preview) {
            result += `\nValue preview: "${parsed.value_preview}"`;
          }
          if (parsed.note) {
            result += `\n\nNote: ${parsed.note}`;
          }
          return result;
        } else {
          let result = `✗ ${parsed.action.toUpperCase()} failed`;
          if (parsed.reason) {
            result += ` - ${parsed.reason}`;
          }
          if (parsed.error) {
            result += ` - ${parsed.error}`;
          }
          if (parsed.selector) {
            result += ` (selector: ${parsed.selector})`;
          }
          if (parsed.diagnostics) {
            result += `\n\nDiagnostics:\n${JSON.stringify(parsed.diagnostics, null, 2)}`;
          }
          return result;
        }
      }
      return raw;
    } catch {
      return raw;
    }
  }
  return String(raw);
}

/**
 * Provider component that registers browser-specific tools with the BuiltInToolProvider.
 * Uses the useBrowserInvoker hook to provide non-blocking browser script execution.
 */
export function BrowserToolProvider() {
  const { register, unregister } = useBuiltInTool();
  const { executeScript } = useBrowserInvoker();
  const { writeFile } = useRustBackend();
  const hasRegistered = useRef(false);

  useEffect(() => {
    // Prevent duplicate registrations
    if (hasRegistered.current) {
      logger.debug('Browser tools already registered, skipping...');
      return;
    }

    logger.debug('Registering browser tools...');

    const browserTools: LocalMCPTool[] = [
      {
        name: 'createSession',
        description:
          'Creates a new interactive browser session in a separate window.',
        inputSchema: {
          type: 'object',
          properties: {
            url: {
              type: 'string',
              description: 'The URL to navigate to when creating the session.',
            },
            title: {
              type: 'string',
              description: 'Optional title for the browser session window.',
            },
          },
          required: ['url'],
        },
        execute: async (args: Record<string, unknown>) => {
          const { url, title } = args as { url: string; title?: string };
          logger.debug('Executing browser_createSession', { url, title });
          const sessionId = await createBrowserSession({
            url,
            title: title || null,
          });
          return `Browser session created successfully: ${sessionId}`;
        },
      },
      {
        name: 'closeSession',
        description: 'Closes an existing browser session and its window.',
        inputSchema: {
          type: 'object',
          properties: {
            sessionId: {
              type: 'string',
              description: 'The ID of the browser session to close.',
            },
          },
          required: ['sessionId'],
        },
        execute: async (args: Record<string, unknown>) => {
          const { sessionId } = args as { sessionId: string };
          logger.debug('Executing browser_closeSession', { sessionId });
          await closeBrowserSession(sessionId);
          return `Browser session closed: ${sessionId}`;
        },
      },
      {
        name: 'listSessions',
        description: 'Lists all active browser sessions.',
        inputSchema: {
          type: 'object',
          properties: {},
          required: [],
        },
        execute: async () => {
          logger.debug('Executing browser_listSessions');
          const sessions = await listBrowserSessions();
          return `Active browser sessions: ${JSON.stringify(sessions, null, 2)}`;
        },
      },
      {
        name: 'getPageContent',
        description:
          'Extracts clean content from the page as Markdown, and saves the raw HTML to a temporary file for reference.',
        inputSchema: {
          type: 'object',
          properties: {
            sessionId: {
              type: 'string',
              description: 'The ID of the browser session.',
            },
          },
          required: ['sessionId'],
        },
        execute: async (args: Record<string, unknown>) => {
          const { sessionId } = args as { sessionId: string };
          logger.debug('Executing browser_getPageContent', { sessionId });

          try {
            // 1. Extract Raw HTML using injection script
            const rawHtml = await executeScript(
              sessionId,
              'document.documentElement.outerHTML',
            );
            if (!rawHtml || typeof rawHtml !== 'string') {
              return JSON.stringify({
                error: 'Failed to get raw HTML from the page.',
              });
            }

            // 2. Convert HTML to Markdown using Turndown
            const turndownService = new TurndownService({
              headingStyle: 'atx',
              codeBlockStyle: 'fenced',
              bulletListMarker: '-',
              emDelimiter: '*',
            });

            // Configure Turndown to handle common HTML elements better
            turndownService.addRule('removeScripts', {
              filter: ['script', 'style', 'noscript'],
              replacement: () => '',
            });

            turndownService.addRule('preserveLineBreaks', {
              filter: 'br',
              replacement: () => '\n',
            });

            const markdownContent = turndownService.turndown(rawHtml);

            // 3. Save Raw HTML file using SecureFileManager
            const tempDir = 'temp_html';
            const uniqueId = `${Date.now()}-${Math.floor(Math.random() * 10000)}`;
            const relativePath = `${tempDir}/${uniqueId}.html`;

            // Convert string to Uint8Array for writeFile
            const encoder = new TextEncoder();
            const htmlBytes = Array.from(encoder.encode(rawHtml));

            await writeFile(relativePath, htmlBytes);

            // 4. Return structured response
            return JSON.stringify(
              {
                content: markdownContent,
                saved_raw_html: relativePath,
                metadata: {
                  extraction_timestamp: new Date().toISOString(),
                  content_length: markdownContent.length,
                  raw_html_size: rawHtml.length,
                },
              },
              null,
              2,
            );
          } catch (error) {
            logger.error('Error in browser_getPageContent:', error);
            return JSON.stringify({
              error: `Failed to process page content: ${error instanceof Error ? error.message : String(error)}`,
            });
          }
        },
      },
      {
        name: 'clickElement',
        description: 'Clicks on a DOM element using CSS selector.',
        inputSchema: {
          type: 'object',
          properties: {
            sessionId: {
              type: 'string',
              description: 'The ID of the browser session.',
            },
            selector: {
              type: 'string',
              description: 'CSS selector of the element to click.',
            },
          },
          required: ['sessionId', 'selector'],
        },
        execute: async (args: Record<string, unknown>) => {
          const { sessionId, selector } = args as {
            sessionId: string;
            selector: string;
          };
          logger.debug('Executing browser_clickElement', {
            sessionId,
            selector,
          });

          try {
            const requestId = await rbClickElement(sessionId, selector);

            logger.debug('Click element request ID received', { requestId });

            // Poll for result with timeout
            let attempts = 0;
            const maxAttempts = 30; // 3 seconds with 100ms intervals

            while (attempts < maxAttempts) {
              const result = await rbPollScriptResult(requestId);

              if (result !== null) {
                logger.debug('Poll result received', { result });
                return formatBrowserResult(result);
              }

              await new Promise((resolve) => setTimeout(resolve, 100));
              attempts++;
            }

            return 'Click operation timed out - no response received from browser';
          } catch (error) {
            logger.error('Error in click_element execution', {
              error,
              sessionId,
              selector,
            });
            return `Error executing click: ${error instanceof Error ? error.message : String(error)}`;
          }
        },
      },
      {
        name: 'inputText',
        description: 'Inputs text into a form field using CSS selector.',
        inputSchema: {
          type: 'object',
          properties: {
            sessionId: {
              type: 'string',
              description: 'The ID of the browser session.',
            },
            selector: {
              type: 'string',
              description: 'CSS selector of the input element.',
            },
            text: {
              type: 'string',
              description: 'Text to input into the field.',
            },
          },
          required: ['sessionId', 'selector', 'text'],
        },
        execute: async (args: Record<string, unknown>) => {
          const { sessionId, selector, text } = args as {
            sessionId: string;
            selector: string;
            text: string;
          };
          logger.debug('Executing browser_inputText', { sessionId, selector });

          try {
            const requestId = await rbInputText(sessionId, selector, text);

            logger.debug('Input text request ID received', { requestId });

            // Poll for result with timeout
            let attempts = 0;
            const maxAttempts = 30; // 3 seconds with 100ms intervals

            while (attempts < maxAttempts) {
              const result = await rbPollScriptResult(requestId);

              if (result !== null) {
                logger.debug('Poll result received', { result });
                return formatBrowserResult(result);
              }

              await new Promise((resolve) => setTimeout(resolve, 100));
              attempts++;
            }

            return 'Input operation timed out - no response received from browser';
          } catch (error) {
            logger.error('Error in input_text execution', {
              error,
              sessionId,
              selector,
            });
            return `Error executing input: ${error instanceof Error ? error.message : String(error)}`;
          }
        },
      },
      {
        name: 'getCurrentUrl',
        description: 'Gets the current URL of the browser page.',
        inputSchema: {
          type: 'object',
          properties: {
            sessionId: {
              type: 'string',
              description: 'The ID of the browser session.',
            },
          },
          required: ['sessionId'],
        },
        execute: async (args: Record<string, unknown>) => {
          const { sessionId } = args as { sessionId: string };
          logger.debug('Executing browser_getCurrentUrl', { sessionId });
          return executeScript(sessionId, 'window.location.href');
        },
      },
      {
        name: 'getPageTitle',
        description: 'Gets the title of the current browser page.',
        inputSchema: {
          type: 'object',
          properties: {
            sessionId: {
              type: 'string',
              description: 'The ID of the browser session.',
            },
          },
          required: ['sessionId'],
        },
        execute: async (args: Record<string, unknown>) => {
          const { sessionId } = args as { sessionId: string };
          logger.debug('Executing browser_getPageTitle', { sessionId });
          return executeScript(sessionId, 'document.title');
        },
      },
      {
        name: 'elementExists',
        description: 'Checks if a DOM element exists using CSS selector.',
        inputSchema: {
          type: 'object',
          properties: {
            sessionId: {
              type: 'string',
              description: 'The ID of the browser session.',
            },
            selector: {
              type: 'string',
              description: 'CSS selector of the element to check.',
            },
          },
          required: ['sessionId', 'selector'],
        },
        execute: async (args: Record<string, unknown>) => {
          const { sessionId, selector } = args as {
            sessionId: string;
            selector: string;
          };
          logger.debug('Executing browser_elementExists', {
            sessionId,
            selector,
          });
          const script = `
(function() {
  try {
    const element = document.querySelector('${selector.replace(/'/g, "\\'")}');
    return element !== null;
  } catch (error) {
    return false;
  }
})()`;
          const result = await executeScript(sessionId, script);
          return result.includes('true') ? 'true' : 'false';
        },
      },
      {
        name: 'scrollPage',
        description: 'Scrolls the page to specified coordinates.',
        inputSchema: {
          type: 'object',
          properties: {
            sessionId: {
              type: 'string',
              description: 'The ID of the browser session.',
            },
            x: {
              type: 'number',
              description: 'X coordinate to scroll to.',
            },
            y: {
              type: 'number',
              description: 'Y coordinate to scroll to.',
            },
          },
          required: ['sessionId', 'x', 'y'],
        },
        execute: async (args: Record<string, unknown>) => {
          const { sessionId, x, y } = args as {
            sessionId: string;
            x: number;
            y: number;
          };
          logger.debug('Executing browser_scrollPage', { sessionId, x, y });
          const script = `window.scrollTo(${x}, ${y}); 'Scrolled to (${x}, ${y})'`;
          return executeScript(sessionId, script);
        },
      },
      {
        name: 'navigateToUrl',
        description: 'Navigates to a new URL in an existing browser session.',
        inputSchema: {
          type: 'object',
          properties: {
            sessionId: {
              type: 'string',
              description: 'The ID of the browser session.',
            },
            url: {
              type: 'string',
              description: 'The URL to navigate to.',
            },
          },
          required: ['sessionId', 'url'],
        },
        execute: async (args: Record<string, unknown>) => {
          const { sessionId, url } = args as {
            sessionId: string;
            url: string;
          };
          logger.debug('Executing browser_navigateToUrl', { sessionId, url });
          const result = await rbNavigateToUrl(sessionId, url);
          return result;
        },
      },
      {
        name: 'extractStructuredContent',
        description:
          'Extracts clean, structured content from the page as JSON, removing styling and focusing on meaningful content.',
        inputSchema: {
          type: 'object',
          properties: {
            sessionId: {
              type: 'string',
              description: 'The ID of the browser session.',
            },
            selector: {
              type: 'string',
              description:
                'CSS selector to focus extraction on specific content area (optional, defaults to body).',
            },
            includeLinks: {
              type: 'boolean',
              description:
                'Whether to include href attributes from links (default: true).',
            },
            maxDepth: {
              type: 'number',
              description:
                'Maximum nesting depth for content extraction (default: 5).',
            },
          },
          required: ['sessionId'],
        },
        execute: async (args: Record<string, unknown>) => {
          const {
            sessionId,
            selector = 'body',
            includeLinks = true,
            maxDepth = 5,
          } = args as {
            sessionId: string;
            selector?: string;
            includeLinks?: boolean;
            maxDepth?: number;
          };
          logger.debug('Executing browser_extractStructuredContent', {
            sessionId,
            selector,
          });

          // JavaScript to extract clean structured content
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

          const result = await executeScript(sessionId, script);

          // Try to parse as JSON, if it fails return as text
          try {
            const parsed = JSON.parse(result);
            return JSON.stringify(parsed, null, 2);
          } catch {
            return result;
          }
        },
      },
    ];

    logger.info('Registering browser tools', {
      toolCount: browserTools.length,
      toolNames: browserTools.map((t) => t.name),
    });

    const serviceId = 'browser';
    const service = {
      listTools: () =>
        browserTools.map((tool) => {
          // Extract meta data without execute function
          return {
            name: tool.name,
            description: tool.description,
            inputSchema: tool.inputSchema,
          };
        }),
      executeTool: async (toolCall: ToolCall): Promise<MCPResponse> => {
        const toolName = toolCall.function.name;
        const tool = browserTools.find((t) => t.name === toolName);

        if (!tool) {
          throw new Error(`Browser tool not found: ${toolName}`);
        }

        // Parse arguments
        let args: Record<string, unknown> = {};
        try {
          const raw = toolCall.function.arguments;
          if (typeof raw === 'string') {
            args = raw.length ? JSON.parse(raw) : {};
          } else if (typeof raw === 'object' && raw !== null) {
            args = raw as Record<string, unknown>;
          }
        } catch (error) {
          logger.warn('Failed parsing browser tool arguments', {
            toolName,
            error,
          });
          args = {};
        }

        // Execute the tool
        const result = await tool.execute(args);

        return {
          jsonrpc: '2.0',
          id: toolCall.id,
          result: {
            content: [
              {
                type: 'text',
                text:
                  typeof result === 'string'
                    ? result
                    : JSON.stringify(result, null, 2),
              },
            ],
          },
        };
      },
    };

    register(serviceId, service);
    hasRegistered.current = true;

    logger.debug('Browser tools registered successfully');

    // Return cleanup function
    return () => {
      if (hasRegistered.current) {
        unregister(serviceId);
        hasRegistered.current = false;
        logger.debug('Browser tools unregistered');
      }
    };
  }, [register, unregister, executeScript, writeFile]);

  // This is a provider component, it doesn't render anything
  return null;
}
