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
        name: 'extractContent',
        description:
          'Extracts page content as Markdown (default) or structured JSON. Saves raw HTML optionally.',
        inputSchema: {
          type: 'object',
          properties: {
            sessionId: { type: 'string', description: 'Browser session ID' },
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
              description:
                'Maximum nesting depth for JSON extraction (default: 5)',
            },
          },
          required: ['sessionId'],
        },
        execute: async (args: Record<string, unknown>) => {
          const {
            sessionId,
            selector = 'body',
            format = 'markdown',
            saveRawHtml = false,
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

            // Save Raw HTML if requested
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
            // First check element state using findElement
            const elementStateScript = `
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

            const elementStateResult = await executeScript(
              sessionId,
              elementStateScript,
            );
            const elementState = JSON.parse(elementStateResult);

            if (!elementState.exists) {
              return formatBrowserResult(
                JSON.stringify({
                  ok: false,
                  action: 'click',
                  reason: 'element_not_found',
                  selector,
                }),
              );
            }

            if (!elementState.clickable) {
              return formatBrowserResult(
                JSON.stringify({
                  ok: false,
                  action: 'click',
                  reason: 'element_not_clickable',
                  selector,
                  diagnostics: elementState,
                }),
              );
            }

            // Element is valid, proceed with click
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
            // First check element state using findElement logic with input-specific validation
            const elementStateScript = `
(function() {
  const selector = '${selector.replace(/'/g, "\\'")}';
  try {
    const el = document.querySelector(selector);
    if (!el) return JSON.stringify({ exists: false, selector });

    const rect = el.getBoundingClientRect();
    const style = window.getComputedStyle(el);
    const visible = !!(rect.width > 0 && rect.height > 0 && style.display !== 'none' && style.visibility !== 'hidden');
    const disabled = el.disabled || el.hasAttribute('disabled') || el.readOnly || el.hasAttribute('readonly');
    const inputable = visible && !disabled && style.pointerEvents !== 'none';

    return JSON.stringify({
      exists: true,
      visible,
      inputable,
      disabled,
      tagName: el.tagName.toLowerCase(),
      type: el.type || 'unknown',
      rect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
      attributes: {
        id: el.id || null,
        className: el.className || null,
        disabled: el.disabled || false,
        readOnly: el.readOnly || false
      },
      selector
    });
  } catch (error) {
    return JSON.stringify({ exists: false, error: error.message, selector });
  }
})()`;

            const elementStateResult = await executeScript(
              sessionId,
              elementStateScript,
            );
            const elementState = JSON.parse(elementStateResult);

            if (!elementState.exists) {
              return formatBrowserResult(
                JSON.stringify({
                  ok: false,
                  action: 'input',
                  reason: 'element_not_found',
                  selector,
                }),
              );
            }

            if (!elementState.inputable) {
              return formatBrowserResult(
                JSON.stringify({
                  ok: false,
                  action: 'input',
                  reason: 'element_not_inputable',
                  selector,
                  diagnostics: elementState,
                }),
              );
            }

            // Element is valid, proceed with input
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
        name: 'findElement',
        description:
          'Find element and check its state (existence, visibility, interactability)',
        inputSchema: {
          type: 'object',
          properties: {
            sessionId: { type: 'string', description: 'Browser session ID' },
            selector: {
              type: 'string',
              description: 'CSS selector of the element to find',
            },
          },
          required: ['sessionId', 'selector'],
        },
        execute: async (args: Record<string, unknown>) => {
          const { sessionId, selector } = args as {
            sessionId: string;
            selector: string;
          };
          logger.debug('Executing browser_findElement', {
            sessionId,
            selector,
          });

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

          const result = await executeScript(sessionId, script);
          return result;
        },
      },
      {
        name: 'navigateBack',
        description: 'Navigate back in browser history',
        inputSchema: {
          type: 'object',
          properties: { sessionId: { type: 'string' } },
          required: ['sessionId'],
        },
        execute: async (args: Record<string, unknown>) => {
          const { sessionId } = args as { sessionId: string };
          logger.debug('Executing browser_navigateBack', { sessionId });
          return executeScript(sessionId, 'history.back(); "Navigated back"');
        },
      },
      {
        name: 'navigateForward',
        description: 'Navigate forward in browser history',
        inputSchema: {
          type: 'object',
          properties: { sessionId: { type: 'string' } },
          required: ['sessionId'],
        },
        execute: async (args: Record<string, unknown>) => {
          const { sessionId } = args as { sessionId: string };
          logger.debug('Executing browser_navigateForward', { sessionId });
          return executeScript(
            sessionId,
            'history.forward(); "Navigated forward"',
          );
        },
      },
      {
        name: 'getElementText',
        description: 'Get text content of a specific element',
        inputSchema: {
          type: 'object',
          properties: {
            sessionId: { type: 'string' },
            selector: { type: 'string' },
          },
          required: ['sessionId', 'selector'],
        },
        execute: async (args: Record<string, unknown>) => {
          const { sessionId, selector } = args as {
            sessionId: string;
            selector: string;
          };
          logger.debug('Executing browser_getElementText', {
            sessionId,
            selector,
          });
          const script = `
            const el = document.querySelector('${selector.replace(/'/g, "\\'")}');
            el ? el.textContent.trim() : null
          `;
          return executeScript(sessionId, script);
        },
      },
      {
        name: 'getElementAttribute',
        description: 'Get specific attribute value of an element',
        inputSchema: {
          type: 'object',
          properties: {
            sessionId: { type: 'string' },
            selector: { type: 'string' },
            attribute: { type: 'string' },
          },
          required: ['sessionId', 'selector', 'attribute'],
        },
        execute: async (args: Record<string, unknown>) => {
          const { sessionId, selector, attribute } = args as {
            sessionId: string;
            selector: string;
            attribute: string;
          };
          logger.debug('Executing browser_getElementAttribute', {
            sessionId,
            selector,
            attribute,
          });
          const script = `
            const el = document.querySelector('${selector.replace(/'/g, "\\'")}');
            el ? el.getAttribute('${attribute}') : null
          `;
          return executeScript(sessionId, script);
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
