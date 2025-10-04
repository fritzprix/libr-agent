import { getLogger } from '@/lib/logger';
import { buildUniqueSelector } from '@/lib/dom/selector';
import type {
  InteractableElement,
  InteractableOptions,
  InteractableResult,
} from './types';
import { validateHtmlInput, parseHTMLDocument } from './utils';
import {
  DEFAULT_INTERACTABLE_OPTIONS,
  INTERACTABLE_SELECTORS,
} from './constants';

const logger = getLogger('HTMLParser:interactable');

/**
 * Checks if an element is visible based on HTML attributes and inline styles
 */
function isElementVisible(element: Element): boolean {
  // DOMParser environment doesn't support getComputedStyle() or getBoundingClientRect()
  // Check visibility based on HTML attributes and inline styles instead

  if (element instanceof HTMLElement) {
    // Check explicit hidden attribute
    if (element.hasAttribute('hidden')) {
      return false;
    }

    // Check aria-hidden
    if (element.getAttribute('aria-hidden') === 'true') {
      return false;
    }

    // Check inline style for common hiding patterns
    const style = element.getAttribute('style') || '';
    const styleHidden =
      style.includes('display:none') ||
      style.includes('display: none') ||
      style.includes('visibility:hidden') ||
      style.includes('visibility: hidden') ||
      style.includes('opacity:0') ||
      style.includes('opacity: 0');

    if (styleHidden) {
      return false;
    }

    // Check class names that commonly indicate hidden elements
    const className = element.getAttribute('class') || '';
    const hiddenByClass =
      className.includes('hidden') ||
      className.includes('invisible') ||
      className.includes('sr-only'); // screen reader only

    if (hiddenByClass) {
      return false;
    }
  }

  // Default to visible in DOMParser environment
  return true;
}

/**
 * Extracts text content from an element using various strategies
 */
function getElementText(element: Element): string {
  if (element instanceof HTMLElement) {
    return (
      element.textContent ||
      (element as HTMLInputElement).value ||
      element.title ||
      (element as HTMLImageElement).alt ||
      element.getAttribute('aria-label') ||
      element.getAttribute('placeholder') ||
      ''
    ).trim();
  }
  return '';
}

/**
 * Determines the type of interactable element
 */
function getElementType(element: Element): InteractableElement['type'] {
  const tag = element.tagName.toLowerCase();
  if (tag === 'a') return 'link';
  if (tag === 'button') return 'button';
  if (tag === 'input') return 'input';
  if (tag === 'select') return 'select';
  if (tag === 'textarea') return 'textarea';
  if (element.getAttribute('role') === 'button') return 'button';
  if (element.hasAttribute('onclick')) return 'button';
  return 'button'; // default fallback
}

/**
 * Parses a single element into an InteractableElement
 */
export function parseElementToInteractable(
  element: Element,
  document: Document,
  options: Required<InteractableOptions>,
): InteractableElement | null {
  const visible = isElementVisible(element);

  // Skip hidden elements unless explicitly requested
  if (!options.includeHidden && !visible) {
    return null;
  }

  const interactableElement: InteractableElement = {
    selector: buildUniqueSelector(element, document),
    type: getElementType(element),
    text: getElementText(element),
    enabled:
      !element.hasAttribute('disabled') &&
      element.getAttribute('aria-disabled') !== 'true',
    visible,
  };

  // Add input-specific attributes
  if (element instanceof HTMLInputElement) {
    interactableElement.inputType = element.type;
    interactableElement.value = element.value;
    interactableElement.placeholder = element.placeholder;
  }

  return interactableElement;
}

/**
 * Parses an HTML string to extract a list of all interactable elements.
 *
 * @param htmlString The HTML string to parse.
 * @param scopeSelector An optional CSS selector to define the scope of the search. Defaults to 'body'.
 * @param options Optional options to control the extraction process.
 * @returns An `InteractableResult` object containing the list of elements and metadata.
 */
export function parseHtmlToInteractables(
  htmlString: string,
  scopeSelector: string = 'body',
  options: InteractableOptions = {},
): InteractableResult {
  const opts = { ...DEFAULT_INTERACTABLE_OPTIONS, ...options };
  const startTime = performance.now();

  try {
    const validationError = validateHtmlInput(htmlString);
    if (validationError) {
      return {
        elements: [],
        error: validationError,
        metadata: {
          extraction_timestamp: new Date().toISOString(),
          total_count: 0,
          scope_selector: scopeSelector,
          performance: {
            execution_time_ms:
              Math.round((performance.now() - startTime) * 100) / 100,
            data_size_bytes: 0,
          },
        },
      };
    }

    const doc = parseHTMLDocument(htmlString);
    const scopeElement = doc.querySelector(scopeSelector);

    if (!scopeElement) {
      return {
        elements: [],
        error: `Scope element not found: ${scopeSelector}`,
        metadata: {
          extraction_timestamp: new Date().toISOString(),
          total_count: 0,
          scope_selector: scopeSelector,
          performance: {
            execution_time_ms: performance.now() - startTime,
            data_size_bytes: 0,
          },
        },
      };
    }

    const elements: InteractableElement[] = [];
    const interactableElements = scopeElement.querySelectorAll(
      INTERACTABLE_SELECTORS,
    );

    logger.debug('Interactable elements found', {
      total: interactableElements.length,
      selector: INTERACTABLE_SELECTORS,
      includeHidden: opts.includeHidden,
    });

    for (const element of Array.from(interactableElements)) {
      if (elements.length >= opts.maxElements) {
        logger.warn(
          `Maximum elements limit (${opts.maxElements}) reached, truncating results`,
        );
        break;
      }

      const visible = isElementVisible(element);
      logger.debug('Processing element', {
        tag: element.tagName,
        id: element.getAttribute('id'),
        class: element.getAttribute('class'),
        visible,
        willInclude: opts.includeHidden || visible,
      });

      const interactableElement = parseElementToInteractable(
        element,
        doc,
        opts,
      );
      if (interactableElement) {
        elements.push(interactableElement);
      }
    }

    const executionTime = performance.now() - startTime;
    const dataSize = JSON.stringify(elements).length;

    return {
      elements,
      metadata: {
        extraction_timestamp: new Date().toISOString(),
        total_count: elements.length,
        scope_selector: scopeSelector,
        performance: {
          execution_time_ms: Math.round(executionTime * 100) / 100,
          data_size_bytes: dataSize,
        },
      },
    };
  } catch (error) {
    logger.error('Error parsing HTML to interactables:', error);
    return {
      elements: [],
      error: `Error during extraction: ${error instanceof Error ? error.message : String(error)}`,
      metadata: {
        extraction_timestamp: new Date().toISOString(),
        total_count: 0,
        scope_selector: scopeSelector,
        performance: {
          execution_time_ms: performance.now() - startTime,
          data_size_bytes: 0,
        },
      },
    };
  }
}
