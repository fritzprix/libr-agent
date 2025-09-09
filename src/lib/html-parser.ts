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
  maxDepth: 3,
  maxChildren: 10,
  maxTextLength: 50,
  includeInteractiveOnly: true,
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
      return {
        metadata: {
          title: '',
          timestamp: new Date().toISOString(),
        },
        content: { tag: 'body', children: [] },
        error: 'Invalid HTML input',
      };
    }

    const parser = new DOMParser();
    const doc = parser.parseFromString(htmlString, 'text/html');

    if (doc.querySelector('parsererror')) {
      return {
        metadata: createMetadata(doc),
        content: { tag: 'body', children: [] },
        error: 'Failed to parse HTML',
      };
    }

    const bodyElement = doc.body || doc.documentElement;
    const content = parseElementToStructured(bodyElement, 0, opts);

    return {
      metadata: createMetadata(doc),
      content: content || { tag: 'body', children: [] },
    };
  } catch (error) {
    logger.error('Error parsing HTML to structured format:', error);
    return {
      metadata: {
        title: '',
        timestamp: new Date().toISOString(),
      },
      content: { tag: 'body', children: [] },
      error: error instanceof Error ? error.message : 'Unknown parsing error',
    };
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
      return {
        timestamp: new Date().toISOString(),
        domMap: { tag: 'body', selector: 'body', children: [] },
        format: 'dom-map',
        error: 'Invalid HTML input',
      };
    }

    const parser = new DOMParser();
    const doc = parser.parseFromString(htmlString, 'text/html');

    if (doc.querySelector('parsererror')) {
      return {
        timestamp: new Date().toISOString(),
        domMap: { tag: 'body', selector: 'body', children: [] },
        format: 'dom-map',
        error: 'Failed to parse HTML',
      };
    }

    const bodyElement = doc.body || doc.documentElement;
    const domMap = parseElementToDOMMap(bodyElement, 0, opts);

    return {
      url: extractMetaURL(doc),
      title: extractTitle(doc),
      timestamp: new Date().toISOString(),
      domMap: domMap || { tag: 'body', selector: 'body', children: [] },
      format: 'dom-map',
    };
  } catch (error) {
    logger.error('Error parsing HTML to DOM map:', error);
    return {
      timestamp: new Date().toISOString(),
      domMap: { tag: 'body', selector: 'body', children: [] },
      format: 'dom-map',
      error: error instanceof Error ? error.message : 'Unknown parsing error',
    };
  }
}

/**
 * Extracts basic metadata from HTML document
 */
export function extractHTMLMetadata(htmlString: string): PageMetadata {
  try {
    const parser = new DOMParser();
    const doc = parser.parseFromString(htmlString, 'text/html');

    return createMetadata(doc);
  } catch (error) {
    logger.error('Error extracting HTML metadata:', error);
    return {
      title: '',
      timestamp: new Date().toISOString(),
    };
  }
}

// Helper functions
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

  // Extract direct text content (not from children)
  let textContent = '';
  for (const child of element.childNodes) {
    if (child.nodeType === Node.TEXT_NODE) {
      const text = child.textContent?.trim();
      if (text) textContent += text + ' ';
    }
  }

  if (textContent.trim()) {
    const trimmedText = textContent.trim();
    result.text =
      trimmedText.length > options.maxTextLength
        ? trimmedText.substring(0, options.maxTextLength) + '...'
        : trimmedText;
  }

  // Process child elements
  for (const child of element.children) {
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
    if (classes.length > 0) result.class = classes[0]; // First class only
  }

  // Extract direct text content
  let textContent = '';
  for (const child of element.childNodes) {
    if (child.nodeType === Node.TEXT_NODE) {
      textContent += child.textContent || '';
    }
  }

  textContent = textContent.trim();
  if (textContent) {
    result.text =
      textContent.length > options.maxTextLength
        ? textContent.substring(0, options.maxTextLength)
        : textContent;
  }

  // Add important attributes for interactive elements
  const type = element.getAttribute('type');
  if (type) result.type = type;

  const href = element.getAttribute('href');
  if (href) result.href = href;

  const placeholder = element.getAttribute('placeholder');
  if (placeholder) result.placeholder = placeholder;

  if (element.tagName === 'INPUT') {
    const value = (element as HTMLInputElement).value;
    if (value) result.value = value;
  }

  const name = element.getAttribute('name');
  if (name) result.name = name;

  const role = element.getAttribute('role');
  if (role) result.role = role;

  const ariaLabel = element.getAttribute('aria-label');
  if (ariaLabel) result.ariaLabel = ariaLabel;

  // Process child elements
  let childElements = Array.from(element.children);

  // Process child elements first to determine if we need to include this element
  const processedChildren: DOMMapNode[] = [];
  for (const child of childElements) {
    const childResult = parseElementToDOMMap(child, depth + 1, options);
    if (childResult) {
      processedChildren.push(childResult);
    }
  }

  // Filter children if interactive-only mode is enabled
  if (options.includeInteractiveOnly) {
    const interactiveChildren = childElements.filter((child) => {
      const hasImportantTag = INTERACTIVE_TAGS.has(child.tagName);
      const hasId = !!child.getAttribute('id');
      const hasClass = !!child.getAttribute('class');
      const hasClickHandler = !!child.getAttribute('onclick');

      return hasImportantTag || hasId || hasClass || hasClickHandler;
    });

    // Sort by importance
    interactiveChildren.sort((a, b) => {
      if (a.getAttribute('id') && !b.getAttribute('id')) return -1;
      if (!a.getAttribute('id') && b.getAttribute('id')) return 1;

      const aIsInteractive = INTERACTIVE_TAGS.has(a.tagName);
      const bIsInteractive = INTERACTIVE_TAGS.has(b.tagName);

      if (aIsInteractive && !bIsInteractive) return -1;
      if (!aIsInteractive && bIsInteractive) return 1;

      return 0;
    });

    childElements = interactiveChildren;
  }

  // Limit children count
  childElements = childElements.slice(0, options.maxChildren);

  for (const child of childElements) {
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
  if (id) {
    return '#' + id;
  }

  const className = element.getAttribute('class');
  if (className?.trim()) {
    const classes = className.trim().split(/\s+/);
    if (classes.length > 0) {
      return '.' + classes[0];
    }
  }

  return element.tagName.toLowerCase();
}
