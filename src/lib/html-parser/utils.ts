import { getLogger } from '@/lib/logger';
import { createCompactText } from '@/lib/text-utils';
import { buildUniqueSelector, isValidCSSIdentifier } from '@/lib/dom/selector';
import type {
  PageMetadata,
  StructuredContent,
  DOMMapResult,
  DOMMapNode,
  ParsedElement,
} from './types';
import {
  EXCLUDE_TAGS,
  EXCLUDE_CLASSES,
  INTERACTIVE_TAGS,
  MEANINGFUL_ELEMENTS,
} from './constants';

const logger = getLogger('HTMLParser:utils');

// Error types
export class HTMLParseError extends Error {
  constructor(
    message: string,
    public readonly cause?: unknown,
  ) {
    super(message);
    this.name = 'HTMLParseError';
  }
}

export class DOMParserError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'DOMParserError';
  }
}

// Type guards
export function isHTMLInputElement(
  element: Element,
): element is HTMLInputElement {
  return element.tagName.toUpperCase() === 'INPUT' && 'value' in element;
}

// HTML input validation
export function validateHtmlInput(html: string): string | null {
  if (!html || typeof html !== 'string') {
    return 'Invalid HTML input: must be a non-empty string';
  }

  if (html.trim().length === 0) {
    return 'Invalid HTML input: cannot be empty or whitespace-only';
  }

  // Basic HTML structure validation
  if (!/<[^>]+>/g.test(html)) {
    return 'Invalid HTML input: must contain at least one valid tag';
  }

  return null;
}

// Document parsing utilities
export function parseHTMLDocument(htmlString: string): Document {
  const parser = new DOMParser();
  const doc = parser.parseFromString(htmlString, 'text/html');

  const parserError = doc.querySelector('parsererror');
  if (parserError) {
    throw new DOMParserError(
      `Failed to parse HTML: ${parserError.textContent || 'Unknown parser error'}`,
    );
  }

  return doc;
}

// Selector generation
export function generateSelector(element: Element, doc: Document): string {
  return buildUniqueSelector(element, doc);
}

// Text extraction utility
export function extractTextContent(
  element: Element,
  maxLength: number,
): string {
  const allText = createCompactText(element.textContent || '');

  if (!allText) {
    return '';
  }

  if (element.children.length === 0) {
    return allText.length > maxLength
      ? allText.substring(0, maxLength) + '...'
      : allText;
  }

  let directText = '';
  for (let i = 0; i < element.childNodes.length; i++) {
    const child = element.childNodes[i];
    if (child.nodeType === Node.TEXT_NODE && child.textContent) {
      directText += child.textContent;
    }
  }

  directText = createCompactText(directText);

  if (directText && directText.length > 2) {
    return directText.length > maxLength
      ? directText.substring(0, maxLength) + '...'
      : directText;
  }

  if (allText.length <= 100 && element.children.length <= 3) {
    return allText.length > maxLength
      ? allText.substring(0, maxLength) + '...'
      : allText;
  }

  return '';
}

// Metadata extraction utilities
export function createMetadata(doc: Document): PageMetadata {
  return {
    title: extractTitle(doc),
    url: extractMetaURL(doc),
    timestamp: new Date().toISOString(),
  };
}

export function extractTitle(doc: Document): string {
  return doc.title || doc.querySelector('title')?.textContent?.trim() || '';
}

export function extractMetaURL(doc: Document): string | undefined {
  const canonical = doc
    .querySelector('link[rel="canonical"]')
    ?.getAttribute('href');
  const ogUrl = doc
    .querySelector('meta[property="og:url"]')
    ?.getAttribute('content');
  return canonical || ogUrl || undefined;
}

// Element validation utilities
export class ElementValidator {
  static validateForParsing(
    element: Element | null,
    depth: number,
    maxDepth: number,
  ): boolean {
    if (depth > maxDepth || !element) {
      return false;
    }

    const tagName = element.tagName.toUpperCase();
    return !EXCLUDE_TAGS.has(tagName);
  }

  static shouldSkipByClass(element: Element): boolean {
    const className = element.getAttribute('class') || '';
    return (
      className !== '' &&
      EXCLUDE_CLASSES.some((cls) => className.toLowerCase().includes(cls))
    );
  }

  static isImportantElement(element: Element): boolean {
    const hasImportantTag = INTERACTIVE_TAGS.has(element.tagName.toUpperCase());
    const hasId = !!element.getAttribute('id');
    const hasClass = !!element.getAttribute('class');
    const hasClickHandler = !!element.getAttribute('onclick');

    return hasImportantTag || hasId || hasClass || hasClickHandler;
  }

  static compareElementImportance(a: Element, b: Element): number {
    const aId = a.getAttribute('id');
    const bId = b.getAttribute('id');

    if (aId && !bId) return -1;
    if (!aId && bId) return 1;

    const aIsInteractive = INTERACTIVE_TAGS.has(a.tagName.toUpperCase());
    const bIsInteractive = INTERACTIVE_TAGS.has(b.tagName.toUpperCase());

    if (aIsInteractive && !bIsInteractive) return -1;
    if (!aIsInteractive && bIsInteractive) return 1;

    return 0;
  }
}

// Child element processing utilities
export class ChildElementProcessor {
  static getFilteredChildElements(
    element: Element,
    includeInteractiveOnly: boolean,
    maxChildren: number,
  ): Element[] {
    let childElements: Element[];

    if (includeInteractiveOnly) {
      childElements = [];
      for (let i = 0; i < element.children.length; i++) {
        const child = element.children[i];
        if (ElementValidator.isImportantElement(child)) {
          childElements.push(child);
        }
      }

      childElements.sort(ElementValidator.compareElementImportance);
    } else {
      childElements = [];
      const maxChildrenCount = Math.min(element.children.length, maxChildren);
      for (let i = 0; i < maxChildrenCount; i++) {
        childElements.push(element.children[i]);
      }
    }

    return childElements.slice(0, maxChildren);
  }
}

// Element validation functions
export function isValidStructuredElement(element: ParsedElement): boolean {
  return !!(
    element.text ||
    element.children.length > 0 ||
    MEANINGFUL_ELEMENTS.has(element.tag)
  );
}

export function isValidDOMMapElement(
  element: DOMMapNode,
  tagName: string,
  includeInteractiveOnly: boolean,
): boolean {
  if (!includeInteractiveOnly) {
    return true;
  }

  const isInteractive =
    INTERACTIVE_TAGS.has(tagName.toUpperCase()) ||
    !!element.id ||
    !!element.class;

  const hasInteractiveChildren = element.children.length > 0;

  return isInteractive || hasInteractiveChildren;
}

// CSS class validation helper
export function validateCSSClass(
  className: string | undefined,
): string | undefined {
  if (!className) return undefined;

  const classes = className.split(/\s+/);
  const validClass = classes.find((cls) => isValidCSSIdentifier(cls));
  return validClass;
}

// Error handling utilities
export function createErrorResult(errorMessage: string): StructuredContent {
  return {
    metadata: {
      title: '',
      timestamp: new Date().toISOString(),
    },
    content: { tag: 'body', selector: 'body', children: [] },
    error: errorMessage,
  };
}

export function createDOMMapErrorResult(errorMessage: string): DOMMapResult {
  return {
    timestamp: new Date().toISOString(),
    domMap: { tag: 'body', selector: 'body', children: [] },
    format: 'dom-map',
    error: errorMessage,
  };
}

export function handleParsingError(
  error: unknown,
  context: string,
): StructuredContent {
  if (error instanceof DOMParserError) {
    logger.error(`${context} - DOM Parser Error:`, error.message);
    return createErrorResult(`DOM parsing failed: ${error.message}`);
  } else if (error instanceof HTMLParseError) {
    logger.error(`${context} - HTML Parse Error:`, error.message);
    return createErrorResult(error.message);
  } else if (error instanceof Error) {
    logger.error(`${context}:`, error);
    return createErrorResult(`Parsing error: ${error.message}`);
  } else {
    logger.error(`${context} - Unknown error:`, error);
    return createErrorResult('Unknown parsing error occurred');
  }
}

export function handleDOMMapError(
  error: unknown,
  context: string,
): DOMMapResult {
  if (error instanceof DOMParserError) {
    logger.error(`${context} - DOM Parser Error:`, error.message);
    return createDOMMapErrorResult(`DOM parsing failed: ${error.message}`);
  } else if (error instanceof HTMLParseError) {
    logger.error(`${context} - HTML Parse Error:`, error.message);
    return createDOMMapErrorResult(error.message);
  } else if (error instanceof Error) {
    logger.error(`${context}:`, error);
    return createDOMMapErrorResult(`Parsing error: ${error.message}`);
  } else {
    logger.error(`${context} - Unknown error:`, error);
    return createDOMMapErrorResult('Unknown parsing error occurred');
  }
}
