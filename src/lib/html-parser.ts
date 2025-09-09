import { getLogger } from '@/lib/logger';

const logger = getLogger('HTMLParser');

// Core interfaces for parsed HTML structures
export interface ParsedElement {
  tag: string;
  text?: string;
  id?: string;
  class?: string;
  href?: string;
  src?: string;
  alt?: string;
  title?: string;
  children: ParsedElement[];
}

export interface DOMMapNode {
  tag: string;
  selector: string;
  id?: string;
  class?: string;
  text?: string;
  type?: string;
  href?: string;
  placeholder?: string;
  value?: string;
  name?: string;
  role?: string;
  ariaLabel?: string;
  children: DOMMapNode[];
}

export interface PageMetadata {
  title: string;
  url?: string;
  timestamp: string;
}

export interface ParseOptions {
  maxDepth?: number;
  includeLinks?: boolean;
  maxTextLength?: number;
}

export interface DOMMapOptions {
  maxDepth?: number;
  maxChildren?: number;
  maxTextLength?: number;
  includeInteractiveOnly?: boolean;
}

export interface StructuredContent {
  metadata: PageMetadata;
  content: ParsedElement;
  error?: string;
}

export interface DOMMapResult {
  url?: string;
  title?: string;
  timestamp: string;
  selector?: string;
  domMap: DOMMapNode;
  format: 'dom-map';
  error?: string;
}

// Configuration constants
const DEFAULT_PARSE_OPTIONS: Required<ParseOptions> = {
  maxDepth: 5,
  includeLinks: true,
  maxTextLength: 1000,
};

const DEFAULT_DOM_MAP_OPTIONS: Required<DOMMapOptions> = {
  maxDepth: 10,
  maxChildren: 20,
  maxTextLength: 100,
  includeInteractiveOnly: false,
};

const EXCLUDE_TAGS = new Set([
  'SCRIPT',
  'STYLE',
  'NOSCRIPT',
  'META',
  'LINK',
  'HEAD',
]);
const EXCLUDE_CLASSES = [
  'ad',
  'banner',
  'popup',
  'sidebar',
  'advertisement',
  'tracking',
];
const INTERACTIVE_TAGS = new Set([
  'A',
  'BUTTON',
  'INPUT',
  'SELECT',
  'TEXTAREA',
  'FORM',
  'IFRAME',
]);
const MEANINGFUL_ELEMENTS = new Set([
  'a',
  'button',
  'input',
  'img',
  'video',
  'audio',
  'iframe',
  'form',
  'table',
]);

// Type guards for safer type checking
function isHTMLInputElement(element: Element): element is HTMLInputElement {
  return element.tagName === 'INPUT' && 'value' in element;
}

function isValidCSSIdentifier(str: string): boolean {
  // CSS identifier must start with letter, underscore, or dash
  // and contain only letters, digits, hyphens, and underscores
  return /^[a-zA-Z_-][a-zA-Z0-9_-]*$/.test(str);
}

// Error types for better error handling
class HTMLParseError extends Error {
  constructor(message: string, public readonly cause?: unknown) {
    super(message);
    this.name = 'HTMLParseError';
  }
}

class DOMParserError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'DOMParserError';
  }
}

/**
 * Parses HTML string to structured format similar to the browser script injection method
 */
export function parseHTMLToStructured(
  htmlString: string,
  options: ParseOptions = {},
): StructuredContent {
  const opts = { ...DEFAULT_PARSE_OPTIONS, ...options };

  try {
    // Handle null/undefined input
    if (!htmlString || typeof htmlString !== 'string') {
      return createErrorResult('Invalid HTML input: input is null, undefined, or not a string');
    }

    if (htmlString.length === 0) {
      return createErrorResult('Invalid HTML input: empty string');
    }

    const doc = parseHTMLDocument(htmlString);
    const bodyElement = doc.body || doc.documentElement;
    
    if (!bodyElement) {
      return createErrorResult('No body or document element found');
    }

    const content = parseElementToStructured(bodyElement, 0, opts);

    return {
      metadata: createMetadata(doc),
      content: content || { tag: 'body', children: [] },
    };
  } catch (error) {
    return handleParsingError(error, 'Error parsing HTML to structured format');
  }
}

/**
 * Parses HTML string to DOM map format for browser navigation
 */
export function parseHTMLToDOMMap(
  htmlString: string,
  options: DOMMapOptions = {},
): DOMMapResult {
  const opts = { ...DEFAULT_DOM_MAP_OPTIONS, ...options };

  try {
    // Handle null/undefined input
    if (!htmlString || typeof htmlString !== 'string') {
      return createDOMMapErrorResult('Invalid HTML input: input is null, undefined, or not a string');
    }

    if (htmlString.length === 0) {
      return createDOMMapErrorResult('Invalid HTML input: empty string');
    }

    const doc = parseHTMLDocument(htmlString);
    const bodyElement = doc.body || doc.documentElement;
    
    if (!bodyElement) {
      return createDOMMapErrorResult('No body or document element found');
    }

    const domMap = parseElementToDOMMap(bodyElement, 0, opts);

    return {
      url: extractMetaURL(doc),
      title: extractTitle(doc),
      timestamp: new Date().toISOString(),
      domMap: domMap || { tag: 'body', selector: 'body', children: [] },
      format: 'dom-map',
    };
  } catch (error) {
    return handleDOMMapError(error, 'Error parsing HTML to DOM map');
  }
}

/**
 * Extracts basic metadata from HTML document
 */
export function extractHTMLMetadata(htmlString: string): PageMetadata {
  try {
    if (!htmlString || typeof htmlString !== 'string') {
      throw new HTMLParseError('Invalid HTML input');
    }

    const doc = parseHTMLDocument(htmlString);
    return createMetadata(doc);
  } catch (error) {
    logger.error('Error extracting HTML metadata:', error);
    return {
      title: '',
      timestamp: new Date().toISOString(),
    };
  }
}

// Helper functions for parsing HTML documents
function parseHTMLDocument(htmlString: string): Document {
  const parser = new DOMParser();
  const doc = parser.parseFromString(htmlString, 'text/html');

  // Check for parser errors
  const parserError = doc.querySelector('parsererror');
  if (parserError) {
    throw new DOMParserError(`Failed to parse HTML: ${parserError.textContent || 'Unknown parser error'}`);
  }

  return doc;
}

// Error handling helpers
function createErrorResult(errorMessage: string): StructuredContent {
  return {
    metadata: {
      title: '',
      timestamp: new Date().toISOString(),
    },
    content: { tag: 'body', children: [] },
    error: errorMessage,
  };
}

function createDOMMapErrorResult(errorMessage: string): DOMMapResult {
  return {
    timestamp: new Date().toISOString(),
    domMap: { tag: 'body', selector: 'body', children: [] },
    format: 'dom-map',
    error: errorMessage,
  };
}

function handleParsingError(error: unknown, context: string): StructuredContent {
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

function handleDOMMapError(error: unknown, context: string): DOMMapResult {
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

// Metadata extraction helpers
function createMetadata(doc: Document): PageMetadata {
  return {
    title: extractTitle(doc),
    url: extractMetaURL(doc),
    timestamp: new Date().toISOString(),
  };
}

function extractTitle(doc: Document): string {
  return doc.title || doc.querySelector('title')?.textContent?.trim() || '';
}

function extractMetaURL(doc: Document): string | undefined {
  const canonical = doc
    .querySelector('link[rel="canonical"]')
    ?.getAttribute('href');
  const ogUrl = doc
    .querySelector('meta[property="og:url"]')
    ?.getAttribute('content');
  return canonical || ogUrl || undefined;
}

// Enhanced text content extraction with nested text support
function extractTextContent(element: Element, maxLength: number): string {
  // Get all text content including nested elements
  const allText = element.textContent?.trim() || '';
  
  // If there's no text at all, return empty
  if (!allText) {
    return '';
  }
  
  // If element has no children, return the text directly
  if (element.children.length === 0) {
    return allText.length > maxLength
      ? allText.substring(0, maxLength) + '...'
      : allText;
  }
  
  // For elements with children, we need to be smarter about text extraction
  // to avoid duplication when children are parsed separately
  
  // Extract only direct text nodes to avoid duplication
  let directText = '';
  for (let i = 0; i < element.childNodes.length; i++) {
    const child = element.childNodes[i];
    if (child.nodeType === Node.TEXT_NODE && child.textContent) {
      directText += child.textContent;
    }
  }
  
  // Clean up whitespace
  directText = directText.trim();
  
  // If there's meaningful direct text, use it
  if (directText && directText.length > 2) {
    return directText.length > maxLength
      ? directText.substring(0, maxLength) + '...'
      : directText;
  }
  
  // If no meaningful direct text, but element has short text content,
  // it might be a leaf-like element that should include its text
  if (allText.length <= 100 && element.children.length <= 3) {
    return allText.length > maxLength
      ? allText.substring(0, maxLength) + '...'
      : allText;
  }
  
  return '';
}



function parseElementToStructured(
  element: Element,
  depth: number,
  options: Required<ParseOptions>,
): ParsedElement | null {
  if (depth > options.maxDepth || !element) {
    return null;
  }

  const tagName = element.tagName.toUpperCase();

  // Skip excluded elements
  if (EXCLUDE_TAGS.has(tagName)) {
    return null;
  }

  const result: ParsedElement = {
    tag: element.tagName.toLowerCase(),
    children: [],
  };

  // Add meaningful attributes
  const id = element.getAttribute('id');
  if (id) result.id = id;

  const className = element.getAttribute('class');
  if (className?.trim()) result.class = className.trim();

  if (options.includeLinks) {
    const href = element.getAttribute('href');
    if (href) result.href = href;
  }

  const src = element.getAttribute('src');
  if (src) result.src = src;

  const alt = element.getAttribute('alt');
  if (alt) result.alt = alt;

  const title = element.getAttribute('title');
  if (title) result.title = title;

  // Extract direct text content using intelligent extraction
  const textContent = extractTextContent(element, options.maxTextLength);
  if (textContent) {
    result.text = textContent;
  }

  // Process child elements - use direct children access for better performance
  for (let i = 0; i < element.children.length; i++) {
    const child = element.children[i];
    const childResult = parseElementToStructured(child, depth + 1, options);
    if (childResult) {
      result.children.push(childResult);
    }
  }

  // Filter out meaningless elements
  const tag = result.tag;
  if (
    !result.text &&
    result.children.length === 0 &&
    !MEANINGFUL_ELEMENTS.has(tag)
  ) {
    return null;
  }

  return result;
}

function parseElementToDOMMap(
  element: Element,
  depth: number,
  options: Required<DOMMapOptions>,
): DOMMapNode | null {
  if (depth > options.maxDepth || !element) {
    return null;
  }

  const tagName = element.tagName.toUpperCase();

  // Skip excluded elements
  if (EXCLUDE_TAGS.has(tagName)) {
    return null;
  }

  // Filter by excluded classes
  const className = element.getAttribute('class') || '';
  if (
    className &&
    EXCLUDE_CLASSES.some((cls) => className.toLowerCase().includes(cls))
  ) {
    return null;
  }

  const result: DOMMapNode = {
    tag: element.tagName.toLowerCase(),
    selector: generateSelector(element),
    children: [],
  };

  // Add core attributes
  const id = element.getAttribute('id');
  if (id) result.id = id;

  if (className.trim()) {
    const classes = className.trim().split(/\s+/);
    // Use first valid CSS class
    const validClass = classes.find(cls => isValidCSSIdentifier(cls));
    if (validClass) result.class = validClass;
  }

  // Extract direct text content using optimized function
  const textContent = extractTextContent(element, options.maxTextLength);
  if (textContent) {
    result.text = textContent;
  }

  // Add important attributes for interactive elements
  const type = element.getAttribute('type');
  if (type) result.type = type;

  const href = element.getAttribute('href');
  if (href) result.href = href;

  const placeholder = element.getAttribute('placeholder');
  if (placeholder) result.placeholder = placeholder;

  // Use type guard instead of type casting
  if (isHTMLInputElement(element)) {
    const value = element.value;
    if (value) result.value = value;
  }

  const name = element.getAttribute('name');
  if (name) result.name = name;

  const role = element.getAttribute('role');
  if (role) result.role = role;

  const ariaLabel = element.getAttribute('aria-label');
  if (ariaLabel) result.ariaLabel = ariaLabel;

  // Optimized child processing
  let childElements: Element[];
  
  if (options.includeInteractiveOnly) {
    // Filter and sort interactive elements
    childElements = [];
    for (let i = 0; i < element.children.length; i++) {
      const child = element.children[i];
      const hasImportantTag = INTERACTIVE_TAGS.has(child.tagName);
      const hasId = !!child.getAttribute('id');
      const hasClass = !!child.getAttribute('class');
      const hasClickHandler = !!child.getAttribute('onclick');

      if (hasImportantTag || hasId || hasClass || hasClickHandler) {
        childElements.push(child);
      }
    }

    // Sort by importance
    childElements.sort((a, b) => {
      const aId = a.getAttribute('id');
      const bId = b.getAttribute('id');
      
      if (aId && !bId) return -1;
      if (!aId && bId) return 1;

      const aIsInteractive = INTERACTIVE_TAGS.has(a.tagName);
      const bIsInteractive = INTERACTIVE_TAGS.has(b.tagName);

      if (aIsInteractive && !bIsInteractive) return -1;
      if (!aIsInteractive && bIsInteractive) return 1;

      return 0;
    });
  } else {
    // Convert HTMLCollection to array only when needed
    childElements = [];
    const maxChildren = Math.min(element.children.length, options.maxChildren);
    for (let i = 0; i < maxChildren; i++) {
      childElements.push(element.children[i]);
    }
  }

  // Limit children count
  const childLimit = Math.min(childElements.length, options.maxChildren);
  
  for (let i = 0; i < childLimit; i++) {
    const child = childElements[i];
    const childResult = parseElementToDOMMap(child, depth + 1, options);
    if (childResult) {
      result.children.push(childResult);
    }
  }

  // If we're in interactive-only mode and this element has no interactive children and is not interactive itself, return null
  if (options.includeInteractiveOnly) {
    const isInteractive =
      INTERACTIVE_TAGS.has(tagName) ||
      !!element.getAttribute('id') ||
      !!element.getAttribute('class') ||
      !!element.getAttribute('onclick');
    const hasInteractiveChildren = result.children.length > 0;

    if (!isInteractive && !hasInteractiveChildren) {
      return null;
    }
  }

  return result;
}

function generateSelector(element: Element): string {
  const id = element.getAttribute('id');
  if (id && isValidCSSIdentifier(id)) {
    return '#' + id;
  }

  const className = element.getAttribute('class');
  if (className?.trim()) {
    const classes = className.trim().split(/\s+/);
    const validClass = classes.find(cls => isValidCSSIdentifier(cls));
    if (validClass) {
      return '.' + validClass;
    }
  }

  return element.tagName.toLowerCase();
}