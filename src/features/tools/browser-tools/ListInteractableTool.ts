import { getLogger } from '@/lib/logger';
import { StrictBrowserMCPTool } from './types';
import {
  createMCPStructuredResponse,
  createMCPErrorResponse,
} from '@/lib/mcp-response-utils';
import { createId } from '@paralleldrive/cuid2';
import { validateSessionId, handleBrowserError } from './error-utils';

const logger = getLogger('ListInteractableTool');

/**
 * Generates browser-side JavaScript for filtering interactive elements.
 * Executes in browser context with access to getComputedStyle() and getBoundingClientRect().
 */
function generateFilterScript(filterType: string, scope: string): string {
  // CRITICAL FIX #3: Validate filterType before string interpolation
  const validFilterTypes = [
    'semantic_clickable',
    'semantic_input',
    'all_focusable',
  ];
  const safeFilterType = validFilterTypes.includes(filterType)
    ? filterType
    : 'semantic_clickable';

  const validScopes = ['viewport', 'all'];
  const safeScope = validScopes.includes(scope) ? scope : 'viewport';

  return `
(function() {
  // ===== Filter Type Selector Definitions =====
  const filterSelectors = {
    semantic_clickable: 'a[href], button:not([disabled]), [role="button"]:not([disabled]), [onclick], [role="link"]',
    semantic_input: 'input:not([type="hidden"]):not([disabled]), select:not([disabled]), textarea:not([disabled]), [contenteditable="true"]',
    all_focusable: 'a, button, input, select, textarea, [tabindex]:not([tabindex="-1"]), [contenteditable]'
  };

  // Use validated filterType (no direct interpolation vulnerability)
  const selector = filterSelectors.${safeFilterType} || filterSelectors.semantic_clickable;
  const candidates = Array.from(document.querySelectorAll(selector));

  // ===== Visibility and Viewport Check =====
  function isActuallyVisible(el) {
    const style = window.getComputedStyle(el);

    // Check CSS visibility properties
    if (style.display === 'none' || style.visibility === 'hidden' || style.opacity === '0') {
      return false;
    }

    // Check element dimensions
    const rect = el.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) {
      return false;
    }

    // Scope-based viewport check
    if ('${safeScope}' === 'viewport') {
      const inViewport = (
        rect.top < window.innerHeight &&
        rect.bottom > 0 &&
        rect.left < window.innerWidth &&
        rect.right > 0
      );
      if (!inViewport) return false;
    }

    return true;
  }

  // ===== Unique Selector Logic =====
  function getUniqueSelector(el) {
    // 1. ID is preferred if unique
    if (el.id) {
      const idSel = '#' + CSS.escape(el.id);
      if (document.querySelectorAll(idSel).length === 1) return idSel;
    }

    const tagName = el.tagName.toLowerCase();

    // 2. Try unique attributes
    const uniqueAttrs = ['name', 'role', 'aria-label', 'placeholder', 'data-testid', 'href'];
    for (const attr of uniqueAttrs) {
      if (el.hasAttribute(attr)) {
        const val = el.getAttribute(attr);
        if (val) {
          const attrSel = tagName + '[' + attr + '="' + val.replace(/"/g, '\\\\"') + '"]';
          if (document.querySelectorAll(attrSel).length === 1) return attrSel;
        }
      }
    }

    // 3. Try class combinations
    if (el.className && typeof el.className === 'string' && el.className.trim()) {
      const classes = el.className.trim().split(/\\s+/);
      const classSel = classes.map(c => '.' + CSS.escape(c)).join('');
      const fullSel = tagName + classSel;
      if (document.querySelectorAll(fullSel).length === 1) return fullSel;
    }

    // 4. Hierarchical fallback with nth-of-type
    let path = [];
    let current = el;
    while (current && current.nodeType === Node.ELEMENT_NODE) {
      let selector = current.tagName.toLowerCase();
      if (current.id) {
        const idSel = '#' + CSS.escape(current.id);
        if (document.querySelectorAll(idSel).length === 1) {
          path.unshift(idSel);
          return path.join(' > ');
        }
        selector = idSel;
      } else {
        const parent = current.parentElement;
        if (parent) {
          const siblings = Array.from(parent.children).filter(c => c.tagName === current.tagName);
          if (siblings.length > 1) {
            selector += ':nth-of-type(' + (siblings.indexOf(current) + 1) + ')';
          }
        }
      }
      path.unshift(selector);
      const fullPath = path.join(' > ');
      if (document.querySelectorAll(fullPath).length === 1) return fullPath;
      current = current.parentElement;
    }
    return path.join(' > ');
  }

  // ===== Extract Element Information =====
  function extractElementInfo(el, index) {
    const tagName = el.tagName.toLowerCase();
    const text = el.textContent?.trim().substring(0, 50) || '';
    const attrs = {};

    // Collect relevant attributes
    ['id', 'class', 'name', 'type', 'href', 'role', 'aria-label', 'placeholder'].forEach(attr => {
      const value = el.getAttribute(attr);
      if (value) attrs[attr] = value;
    });

    return {
      index,
      tag: tagName,
      text,
      attributes: attrs,
      selector: getUniqueSelector(el)
    };
  }

  // ===== Main Filtering Logic =====
  const visibleElements = candidates
    .filter(isActuallyVisible)
    .slice(0, 50) // Hard limit for safety
    .map((el, idx) => extractElementInfo(el, idx));

  return JSON.stringify(visibleElements);
})();
  `.trim();
}

/**
 * Formats filtered elements into user-friendly text output.
 * Keeps output minimal (no purpose estimation) for token efficiency.
 */
function formatSmartResults(
  elements: Array<{
    index: number;
    tag: string;
    text: string;
    attributes: Record<string, string>;
    selector: string;
  }>,
  filterType: string,
  scope: string,
): string {
  if (elements.length === 0) {
    return `No ${filterType.replace('_', ' ')} elements found in ${scope === 'viewport' ? 'current viewport' : 'page'}.`;
  }

  // Header with metadata
  const filterLabel = filterType.replace('_', ' ');
  const scopeLabel = scope === 'viewport' ? 'viewport' : 'page';
  let text = `Found ${elements.length} ${filterLabel} element(s) in ${scopeLabel}:\n\n`;

  // Format each element (minimal, no purpose estimation per user requirement)
  elements.forEach((el) => {
    const attrs = Object.entries(el.attributes)
      .map(([k, v]) => `${k}="${v}"`)
      .join(' ');

    const attrStr = attrs ? ` ${attrs}` : '';
    const textStr = el.text ? ` "${el.text}"` : '';

    text += `[${el.index}] <${el.tag}${attrStr}>${textStr}\n`;
    text += `    Selector: ${el.selector}\n\n`;
  });

  // Footer with usage hint
  text += `💡 Use the selector or index to interact with these elements.`;

  return text;
}

/**
 * Smart interactive element listing with semantic filtering and viewport awareness.
 * Reduces token usage by 90%+ through browser-side filtering and semantic categorization.
 */
export const listInteractableTool: StrictBrowserMCPTool = {
  name: 'listInteractable',
  description:
    'Lists interactive elements with semantic filtering and viewport awareness. Reduces output tokens by 90%+ through smart categorization and browser-side filtering.',
  inputSchema: {
    type: 'object',
    properties: {
      sessionId: {
        type: 'string',
        description: 'The browser session ID to list elements from',
      },
      filterType: {
        type: 'string',
        enum: ['semantic_clickable', 'semantic_input', 'all_focusable'],
        description: `Filter type determines which elements to return:
- semantic_clickable: <a>, <button>, [role="button"], [onclick] (for navigation/actions)
- semantic_input: <input>, <select>, <textarea>, [contenteditable] (for data entry)
- all_focusable: All elements with tabindex or naturally focusable (comprehensive)`.trim(),
        default: 'semantic_clickable',
      },
      scope: {
        type: 'string',
        enum: ['viewport', 'all'],
        description: `Scope determines visibility filter:
- viewport: Only elements visible in current viewport (reduces noise)
- all: All visible elements regardless of scroll position`.trim(),
        default: 'viewport',
      },
    },
    required: ['sessionId'],
  },

  execute: async (
    args: Record<string, unknown>,
    executeScript?: (sessionId: string, script: string) => Promise<string>,
  ) => {
    // 1. Validate sessionId (CRITICAL FIX #2)
    const { isValid, errorResponse } = validateSessionId(args.sessionId);
    if (!isValid && errorResponse) {
      return errorResponse;
    }

    const sessionId = String(args.sessionId);

    // 2. Validate and set filterType with type checking (MEDIUM PRIORITY FIX #4)
    const validFilterTypes = [
      'semantic_clickable',
      'semantic_input',
      'all_focusable',
    ] as const;
    const filterType =
      typeof args.filterType === 'string' &&
      validFilterTypes.includes(
        args.filterType as (typeof validFilterTypes)[number],
      )
        ? args.filterType
        : 'semantic_clickable';

    // 3. Validate and set scope with type checking
    const validScopes = ['viewport', 'all'] as const;
    const scope =
      typeof args.scope === 'string' &&
      validScopes.includes(args.scope as (typeof validScopes)[number])
        ? args.scope
        : 'viewport';

    // 4. Check executeScript availability
    if (!executeScript) {
      logger.error('executeScript not available');
      return createMCPErrorResponse(
        'Browser script execution not available',
        -32603,
        { sessionId, filterType, scope },
      );
    }

    try {
      // 5. Generate and execute browser-side filtering script
      const filterScript = generateFilterScript(filterType, scope);
      const resultJson = await executeScript(sessionId, filterScript);

      // 6. Parse result with error handling (MEDIUM PRIORITY FIX #7)
      let elements: Array<{
        index: number;
        tag: string;
        text: string;
        attributes: Record<string, string>;
        selector: string;
      }>;

      try {
        elements = JSON.parse(resultJson);
      } catch (parseError) {
        logger.error('Failed to parse script result', {
          parseError,
          resultJson,
        });
        return createMCPErrorResponse(
          'Failed to parse browser script result',
          -32603,
          { sessionId, filterType, scope, parseError: String(parseError) },
        );
      }

      // 7. Format results (text + metadata)
      const formattedText = formatSmartResults(elements, filterType, scope);

      // 8. Return MCPResponse with text and metadata (CRITICAL FIX #1 + USER REQUIREMENT)
      return createMCPStructuredResponse(
        formattedText,
        {
          elementCount: elements.length,
          filterType,
          scope,
          sessionId,
        },
        createId(),
      );
    } catch (error) {
      // 9. Use handleBrowserError for proper context (MEDIUM PRIORITY FIX #6)
      logger.error('Failed to execute listInteractable', {
        error,
        sessionId,
        filterType,
        scope,
      });
      return handleBrowserError(error, {
        toolName: 'listInteractable',
        sessionId,
      });
    }
  },
};
