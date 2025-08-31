import { pollScriptResult } from '@/lib/rust-backend-client';

/**
 * Common schema definitions for browser tools to reduce duplication
 */
export const BROWSER_TOOL_SCHEMAS = {
  sessionId: {
    type: 'string' as const,
    description: 'The ID of the browser session.',
  },
  selector: {
    type: 'string' as const,
    description: 'CSS selector of the element.',
  },
  url: {
    type: 'string' as const,
    description: 'The URL to navigate to.',
  },
  text: {
    type: 'string' as const,
    description: 'Text content to input.',
  },
  title: {
    type: 'string' as const,
    description: 'Optional title for the browser session window.',
  },
};

/**
 * Common element state check result interface
 */
export interface ElementState {
  exists: boolean;
  visible?: boolean;
  clickable?: boolean;
  inputable?: boolean;
  disabled?: boolean;
  tagName?: string;
  type?: string;
  rect?: {
    x: number;
    y: number;
    width: number;
    height: number;
  };
  attributes?: {
    id: string | null;
    className: string | null;
    disabled: boolean;
    readOnly?: boolean;
  };
  selector: string;
  error?: string;
}

/**
 * Enhanced element state checking function
 * Checks element existence, visibility, and interaction capability
 */
export async function checkElementState(
  executeScript: (sessionId: string, script: string) => Promise<string>,
  sessionId: string,
  selector: string,
  action: 'click' | 'input',
): Promise<ElementState> {
  const script = `
(function() {
  const selector = '${selector.replace(/'/g, "\\'")}';
  try {
    const el = document.querySelector(selector);
    if (!el) return JSON.stringify({ exists: false, selector });

    const rect = el.getBoundingClientRect();
    const style = window.getComputedStyle(el);
    const visible = !!(rect.width > 0 && rect.height > 0 && style.display !== 'none' && style.visibility !== 'hidden');
    
    if ('${action}' === 'input') {
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
    } else {
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
    }
  } catch (error) {
    return JSON.stringify({ exists: false, error: error.message, selector });
  }
})()`;

  const result = await executeScript(sessionId, script);
  return JSON.parse(result) as ElementState;
}

/**
 * Common polling function with timeout for script results
 */
export async function pollWithTimeout(
  requestId: string,
  maxAttempts = 30,
  interval = 100,
): Promise<string | null> {
  let attempts = 0;

  while (attempts < maxAttempts) {
    const result = await pollScriptResult(requestId);

    if (result !== null) {
      return result;
    }

    await new Promise((resolve) => setTimeout(resolve, interval));
    attempts++;
  }

  return null;
}

/**
 * Common result formatting function
 * Handles JSON envelope responses from browser operations
 */
export function formatBrowserResult(raw: unknown): string {
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
