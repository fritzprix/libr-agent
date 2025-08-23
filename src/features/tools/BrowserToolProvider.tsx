import { useEffect, useRef } from 'react';
import { useBuiltInTools } from '@/context/BuiltInToolContext';
import { useBrowserInvoker } from '@/hooks/use-browser-invoker';
import { MCPTool } from '@/lib/mcp-types';
import { getLogger } from '@/lib/logger';
import { invoke } from '@tauri-apps/api/core';

const logger = getLogger('BrowserToolProvider');

// Extended MCPTool type that includes execute function
type LocalMCPTool = MCPTool & {
  execute: (args: Record<string, unknown>) => Promise<unknown>;
};

/**
 * Provider component that registers browser-specific tools with the BuiltInToolContext.
 * Uses the useBrowserInvoker hook to provide non-blocking browser script execution.
 */
export function BrowserToolProvider() {
  const { registerLocalTools } = useBuiltInTools();
  const { executeScript } = useBrowserInvoker();
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
        name: 'browser_createSession',
        description: 'Creates a new interactive browser session in a separate window.',
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
          const sessionId = await invoke<string>('create_browser_session', {
            url,
            title: title || null,
          });
          return `Browser session created successfully: ${sessionId}`;
        },
      },
      {
        name: 'browser_closeSession',
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
          const result = await invoke<string>('close_browser_session', {
            sessionId,
          });
          return result;
        },
      },
      {
        name: 'browser_listSessions',
        description: 'Lists all active browser sessions.',
        inputSchema: {
          type: 'object',
          properties: {},
          required: [],
        },
        execute: async () => {
          logger.debug('Executing browser_listSessions');
          const sessions = await invoke<unknown[]>('list_browser_sessions');
          return `Active browser sessions: ${JSON.stringify(sessions, null, 2)}`;
        },
      },
      {
        name: 'browser_getPageContent',
        description: 'Gets the full HTML content of the current browser page.',
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
          return executeScript(sessionId, 'document.documentElement.outerHTML');
        },
      },
      {
        name: 'browser_clickElement',
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
          const script = `
(function() {
  try {
    const element = document.querySelector('${selector.replace(/'/g, "\\'")}');
    if (element) {
      element.click();
      return 'Element clicked successfully';
    } else {
      throw new Error('Element not found: ${selector.replace(/'/g, "\\'")}');
    }
  } catch (error) {
    throw new Error('Click failed: ' + error.message);
  }
})()`;
          return executeScript(sessionId, script);
        },
      },
      {
        name: 'browser_inputText',
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
          const script = `
(function() {
  try {
    const element = document.querySelector('${selector.replace(/'/g, "\\'")}');
    if (element) {
      element.value = '${text.replace(/'/g, "\\'").replace(/\n/g, '\\n')}';
      element.dispatchEvent(new Event('input', { bubbles: true }));
      element.dispatchEvent(new Event('change', { bubbles: true }));
      return 'Text input successfully: ' + element.value;
    } else {
      throw new Error('Input element not found: ${selector.replace(/'/g, "\\'")}');
    }
  } catch (error) {
    throw new Error('Input failed: ' + error.message);
  }
})()`;
          return executeScript(sessionId, script);
        },
      },
      {
        name: 'browser_getCurrentUrl',
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
        name: 'browser_getPageTitle',
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
        name: 'browser_elementExists',
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
        name: 'browser_scrollPage',
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
        name: 'browser_navigateToUrl',
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
          const result = await invoke<string>('navigate_to_url', {
            sessionId,
            url,
          });
          return result;
        },
      },
      {
        name: 'browser_extractStructuredContent',
        description: 'Extracts clean, structured content from the page as JSON, removing styling and focusing on meaningful content.',
        inputSchema: {
          type: 'object',
          properties: {
            sessionId: {
              type: 'string',
              description: 'The ID of the browser session.',
            },
            selector: {
              type: 'string',
              description: 'CSS selector to focus extraction on specific content area (optional, defaults to body).',
            },
            includeLinks: {
              type: 'boolean',
              description: 'Whether to include href attributes from links (default: true).',
            },
            maxDepth: {
              type: 'number',
              description: 'Maximum nesting depth for content extraction (default: 5).',
            },
          },
          required: ['sessionId'],
        },
        execute: async (args: Record<string, unknown>) => {
          const { sessionId, selector = 'body', includeLinks = true, maxDepth = 5 } = args as {
            sessionId: string;
            selector?: string;
            includeLinks?: boolean;
            maxDepth?: number;
          };
          logger.debug('Executing browser_extractStructuredContent', { sessionId, selector });
          
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

    registerLocalTools(browserTools);
    hasRegistered.current = true;

    logger.debug('Browser tools registered successfully');
  }, [registerLocalTools, executeScript]);

  // This is a provider component, it doesn't render anything
  return null;
}
