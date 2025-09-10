import { pollScriptResult } from '@/lib/rust-backend-client';

/**
 * Common schema definitions for browser tools to reduce duplication
 */
interface SchemaProperty {
  type: 'string';
  description: string;
}

export const BROWSER_TOOL_SCHEMAS: Record<string, SchemaProperty> = {
  sessionId: {
    type: 'string',
    description: 'The ID of the browser session.',
  },
  selector: {
    type: 'string',
    description: 'CSS selector of the element.',
  },
  url: {
    type: 'string',
    description: 'The URL to navigate to.',
  },
  text: {
    type: 'string',
    description: 'Text content to input.',
  },
  title: {
    type: 'string',
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
 * Validates if parsed result has ElementState structure
 */
function isValidElementState(parsed: unknown): parsed is ElementState {
  if (!parsed || typeof parsed !== 'object') {
    return false;
  }

  const obj = parsed as Record<string, unknown>;
  return typeof obj.exists === 'boolean' && typeof obj.selector === 'string';
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
  // Input validation
  if (!sessionId || typeof sessionId !== 'string') {
    return {
      exists: false,
      selector,
      error: 'Invalid sessionId: must be a non-empty string',
    };
  }

  if (!selector || typeof selector !== 'string') {
    return {
      exists: false,
      selector: selector || '',
      error: 'Invalid selector: must be a non-empty string',
    };
  }

  // Safe selector injection using JSON.stringify
  const script = `
(function() {
  const selector = ${JSON.stringify(selector)};
  const action = ${JSON.stringify(action)};
  
  try {
    const el = document.querySelector(selector);
    if (!el) return JSON.stringify({ exists: false, selector });

    const rect = el.getBoundingClientRect();
    const style = window.getComputedStyle(el);
    const visible = !!(rect.width > 0 && rect.height > 0 && style.display !== 'none' && style.visibility !== 'hidden');
    
    if (action === 'input') {
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

  try {
    const result = await executeScript(sessionId, script);

    // executeScript 결과 검증
    if (!result || typeof result !== 'string') {
      return {
        exists: false,
        selector,
        error: 'No valid result from script execution',
      };
    }

    let parsed: unknown;
    try {
      parsed = JSON.parse(result);
    } catch (parseError) {
      return {
        exists: false,
        selector,
        error: `Failed to parse script result: ${parseError instanceof Error ? parseError.message : String(parseError)}`,
      };
    }

    // 파싱 결과가 null이거나 유효하지 않은 경우
    if (!isValidElementState(parsed)) {
      return {
        exists: false,
        selector,
        error: 'Invalid script result structure - missing required properties',
      };
    }

    return parsed;
  } catch (error) {
    return {
      exists: false,
      selector,
      error: `Script execution failed: ${error instanceof Error ? error.message : String(error)}`,
    };
  }
}

/**
 * Common polling function with timeout for script results
 */
export async function pollWithTimeout(
  requestId: string,
  maxAttempts = 30,
  interval = 100,
): Promise<string | null> {
  // Input validation
  if (!requestId || typeof requestId !== 'string') {
    throw new Error('Invalid requestId: must be a non-empty string');
  }

  let attempts = 0;

  while (attempts < maxAttempts) {
    try {
      const result = await pollScriptResult(requestId);

      if (result !== null) {
        return result;
      }
    } catch (error) {
      // Log error but continue polling
      console.warn(`Poll attempt ${attempts + 1} failed:`, error);
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
      if (parsed && typeof parsed === 'object' && 'ok' in parsed) {
        if (parsed.ok) {
          let result = `✓ ${String(parsed.action || 'ACTION').toUpperCase()} successful`;
          if (parsed.selector) {
            result += ` (selector: ${parsed.selector})`;
          }
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
          let result = `✗ ${String(parsed.action || 'ACTION').toUpperCase()} failed`;
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
      return String(raw);
    } catch {
      return String(raw);
    }
  }
  return String(raw);
}
