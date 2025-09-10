import { getLogger } from '@/lib/logger';
import { createCompactText } from '@/lib/text-utils';

const logger = getLogger('HTMLParser');

// Core interfaces for parsed HTML structures
export interface ParsedElement {
  tag: string;
  selector: string;
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

// Type guards and utility functions
function isHTMLInputElement(element: Element): element is HTMLInputElement {
  return element.tagName === 'INPUT' && 'value' in element;
}

function isValidCSSIdentifier(str: string): boolean {
  return /^[a-zA-Z_-][a-zA-Z0-9_-]*$/.test(str);
}

// Error types
class HTMLParseError extends Error {
  constructor(
    message: string,
    public readonly cause?: unknown,
  ) {
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

// Common parsing interfaces
interface BaseParseResult {
  tag: string;
  children: BaseParseResult[];
  text?: string;
}

interface BaseParseOptions {
  maxDepth: number;
  maxTextLength: number;
}

interface ParseContext<T extends BaseParseResult, O extends BaseParseOptions> {
  element: Element;
  depth: number;
  options: O;
  result: T;
}

// Parsing pipeline interface
interface ParsePipeline<T extends BaseParseResult, O extends BaseParseOptions> {
  document: Document;
  preValidate(element: Element, depth: number, options: O): boolean;
  createBaseResult(element: Element): T;
  extractAttributes(context: ParseContext<T, O>): void;
  extractText(context: ParseContext<T, O>): void;
  processChildren(context: ParseContext<T, O>): void;
  postValidate(context: ParseContext<T, O>): boolean;
}

// Attribute extractor system
interface AttributeExtractor<T> {
  canExtract(element: Element): boolean;
  extract(element: Element): Partial<T>;
}

class BasicAttributeExtractor
  implements AttributeExtractor<{ id?: string; class?: string; title?: string }>
{
  canExtract(): boolean {
    return true;
  }

  extract(element: Element): { id?: string; class?: string; title?: string } {
    const result: { id?: string; class?: string; title?: string } = {};

    const id = element.getAttribute('id');
    if (id) result.id = id;

    const className = element.getAttribute('class');
    if (className?.trim()) result.class = className.trim();

    const title = element.getAttribute('title');
    if (title) result.title = title;

    return result;
  }
}

class LinkAttributeExtractor
  implements AttributeExtractor<{ href?: string; src?: string; alt?: string }>
{
  constructor(private includeLinks: boolean) {}

  canExtract(): boolean {
    return this.includeLinks;
  }

  extract(element: Element): { href?: string; src?: string; alt?: string } {
    const result: { href?: string; src?: string; alt?: string } = {};

    const href = element.getAttribute('href');
    if (href) result.href = href;

    const src = element.getAttribute('src');
    if (src) result.src = src;

    const alt = element.getAttribute('alt');
    if (alt) result.alt = alt;

    return result;
  }
}

class InteractiveAttributeExtractor
  implements
    AttributeExtractor<{
      type?: string;
      placeholder?: string;
      value?: string;
      name?: string;
      role?: string;
      ariaLabel?: string;
    }>
{
  canExtract(element: Element): boolean {
    return INTERACTIVE_TAGS.has(element.tagName.toUpperCase());
  }

  extract(element: Element): {
    type?: string;
    placeholder?: string;
    value?: string;
    name?: string;
    role?: string;
    ariaLabel?: string;
  } {
    const result: {
      type?: string;
      placeholder?: string;
      value?: string;
      name?: string;
      role?: string;
      ariaLabel?: string;
    } = {};

    const type = element.getAttribute('type');
    if (type) result.type = type;

    const placeholder = element.getAttribute('placeholder');
    if (placeholder) result.placeholder = placeholder;

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

    return result;
  }
}

class AttributeExtractorManager<T> {
  private extractors: AttributeExtractor<Partial<T>>[] = [];

  addExtractor(extractor: AttributeExtractor<Partial<T>>): this {
    this.extractors.push(extractor);
    return this;
  }

  extractAll(element: Element): Partial<T> {
    let result: Partial<T> = {};

    for (const extractor of this.extractors) {
      if (extractor.canExtract(element)) {
        const extracted = extractor.extract(element);
        result = { ...result, ...extracted };
      }
    }

    return result;
  }
}

// Element validation utility
class ElementValidator {
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
    const hasImportantTag = INTERACTIVE_TAGS.has(element.tagName);
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

    const aIsInteractive = INTERACTIVE_TAGS.has(a.tagName);
    const bIsInteractive = INTERACTIVE_TAGS.has(b.tagName);

    if (aIsInteractive && !bIsInteractive) return -1;
    if (!aIsInteractive && bIsInteractive) return 1;

    return 0;
  }
}

// Child element processing utilities
class ChildElementProcessor {
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

// Text extraction utility
function extractTextContent(element: Element, maxLength: number): string {
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

// Element validation functions
function isValidStructuredElement(element: ParsedElement): boolean {
  return !!(
    element.text ||
    element.children.length > 0 ||
    MEANINGFUL_ELEMENTS.has(element.tag)
  );
}

function isValidDOMMapElement(
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

// Enhanced selector generation with uniqueness guarantee
function generateSelector(element: Element, doc: Document): string {
  // Try to generate a simple unique selector first
  const simpleSelector = generateSimpleSelector(element);

  // Check if the simple selector is unique
  if (isSelectorUnique(simpleSelector, element, doc)) {
    return simpleSelector;
  }

  // If not unique, generate a hierarchical selector
  return generateHierarchicalSelector(element, doc);
}

function generateSimpleSelector(element: Element): string {
  const id = element.getAttribute('id');
  if (id && isValidCSSIdentifier(id)) {
    return '#' + id;
  }

  const className = element.getAttribute('class');
  if (className?.trim()) {
    const classes = className.trim().split(/\s+/);
    const validClass = classes.find((cls) => isValidCSSIdentifier(cls));
    if (validClass) {
      return '.' + validClass;
    }
  }

  return element.tagName.toLowerCase();
}

function generateHierarchicalSelector(element: Element, doc: Document): string {
  const path: string[] = [];
  let current: Element | null = element;

  while (current && current !== doc.documentElement) {
    const selector = generateElementSelectorWithIndex(current);
    path.unshift(selector);
    current = current.parentElement;
  }

  // Start with the full path and reduce until we find a unique selector
  for (let i = 0; i < path.length; i++) {
    const partialSelector = path.slice(i).join(' > ');
    if (isSelectorUnique(partialSelector, element, doc)) {
      return partialSelector;
    }
  }

  // Fallback to full path
  return path.join(' > ');
}

function generateElementSelectorWithIndex(element: Element): string {
  const tag = element.tagName.toLowerCase();

  // Try ID first
  const id = element.getAttribute('id');
  if (id && isValidCSSIdentifier(id)) {
    return `${tag}#${id}`;
  }

  // Try class
  const className = element.getAttribute('class');
  if (className?.trim()) {
    const classes = className.trim().split(/\s+/);
    const validClass = classes.find((cls) => isValidCSSIdentifier(cls));
    if (validClass) {
      return `${tag}.${validClass}`;
    }
  }

  // Add nth-child index for disambiguation
  const parent = element.parentElement;
  if (parent) {
    const siblings = Array.from(parent.children).filter(
      (child) => child.tagName === element.tagName,
    );

    if (siblings.length > 1) {
      const index = siblings.indexOf(element) + 1;
      return `${tag}:nth-child(${index})`;
    }
  }

  return tag;
}

function isSelectorUnique(
  selector: string,
  targetElement: Element,
  doc: Document,
): boolean {
  try {
    const elements = doc.querySelectorAll(selector);
    return elements.length === 1 && elements[0] === targetElement;
  } catch {
    // Invalid selector
    return false;
  }
}

// Common parsing pipeline
function parseElementWithPipeline<
  T extends BaseParseResult,
  O extends BaseParseOptions,
>(
  element: Element,
  depth: number,
  options: O,
  pipeline: ParsePipeline<T, O>,
): T | null {
  if (!pipeline.preValidate(element, depth, options)) {
    return null;
  }

  const result = pipeline.createBaseResult(element);
  const context: ParseContext<T, O> = {
    element,
    depth,
    options,
    result,
  };

  pipeline.extractAttributes(context);
  pipeline.extractText(context);
  pipeline.processChildren(context);

  if (!pipeline.postValidate(context)) {
    return null;
  }

  return result;
}

// Structured parsing pipeline
class StructuredParsePipeline
  implements ParsePipeline<ParsedElement, Required<ParseOptions>>
{
  document: Document;
  private attributeManager: AttributeExtractorManager<ParsedElement>;

  constructor(document: Document) {
    this.document = document;
    this.attributeManager =
      new AttributeExtractorManager<ParsedElement>().addExtractor(
        new BasicAttributeExtractor() as AttributeExtractor<
          Partial<ParsedElement>
        >,
      );
  }

  preValidate(
    element: Element,
    depth: number,
    options: Required<ParseOptions>,
  ): boolean {
    return ElementValidator.validateForParsing(
      element,
      depth,
      options.maxDepth,
    );
  }

  createBaseResult(element: Element): ParsedElement {
    return {
      tag: element.tagName.toLowerCase(),
      selector: generateSelector(element, this.document),
      children: [],
    };
  }

  extractAttributes(
    context: ParseContext<ParsedElement, Required<ParseOptions>>,
  ): void {
    const basicAttrs = this.attributeManager.extractAll(context.element);
    Object.assign(context.result, basicAttrs);

    if (context.options.includeLinks) {
      const linkExtractor = new LinkAttributeExtractor(true);
      const linkAttrs = linkExtractor.extract(context.element);
      Object.assign(context.result, linkAttrs);
    }
  }

  extractText(
    context: ParseContext<ParsedElement, Required<ParseOptions>>,
  ): void {
    const textContent = extractTextContent(
      context.element,
      context.options.maxTextLength,
    );
    if (textContent) {
      context.result.text = textContent;
    }
  }

  processChildren(
    context: ParseContext<ParsedElement, Required<ParseOptions>>,
  ): void {
    for (let i = 0; i < context.element.children.length; i++) {
      const child = context.element.children[i];
      const childResult = parseElementToStructured(
        child,
        context.depth + 1,
        context.options,
        this.document,
      );
      if (childResult) {
        context.result.children.push(childResult);
      }
    }
  }

  postValidate(
    context: ParseContext<ParsedElement, Required<ParseOptions>>,
  ): boolean {
    return isValidStructuredElement(context.result);
  }
}

// DOM Map parsing pipeline
class DOMMapParsePipeline
  implements ParsePipeline<DOMMapNode, Required<DOMMapOptions>>
{
  document: Document;
  private attributeManager: AttributeExtractorManager<DOMMapNode>;

  constructor(document: Document) {
    this.document = document;
    this.attributeManager = new AttributeExtractorManager<DOMMapNode>()
      .addExtractor(
        new BasicAttributeExtractor() as AttributeExtractor<
          Partial<DOMMapNode>
        >,
      )
      .addExtractor(
        new InteractiveAttributeExtractor() as AttributeExtractor<
          Partial<DOMMapNode>
        >,
      );
  }

  preValidate(
    element: Element,
    depth: number,
    options: Required<DOMMapOptions>,
  ): boolean {
    return (
      ElementValidator.validateForParsing(element, depth, options.maxDepth) &&
      !ElementValidator.shouldSkipByClass(element)
    );
  }

  createBaseResult(element: Element): DOMMapNode {
    return {
      tag: element.tagName.toLowerCase(),
      selector: generateSelector(element, this.document),
      children: [],
    };
  }

  extractAttributes(
    context: ParseContext<DOMMapNode, Required<DOMMapOptions>>,
  ): void {
    const attrs = this.attributeManager.extractAll(context.element);
    Object.assign(context.result, attrs);

    if (context.result.class) {
      const classes = context.result.class.split(/\s+/);
      const validClass = classes.find((cls) => isValidCSSIdentifier(cls));
      if (validClass) {
        context.result.class = validClass;
      } else {
        delete context.result.class;
      }
    }

    const href = context.element.getAttribute('href');
    if (href) context.result.href = href;
  }

  extractText(
    context: ParseContext<DOMMapNode, Required<DOMMapOptions>>,
  ): void {
    const textContent = extractTextContent(
      context.element,
      context.options.maxTextLength,
    );
    if (textContent) {
      context.result.text = textContent;
    }
  }

  processChildren(
    context: ParseContext<DOMMapNode, Required<DOMMapOptions>>,
  ): void {
    const childElements = ChildElementProcessor.getFilteredChildElements(
      context.element,
      context.options.includeInteractiveOnly,
      context.options.maxChildren,
    );

    for (const child of childElements) {
      const childResult = parseElementToDOMMap(
        child,
        context.depth + 1,
        context.options,
        this.document,
      );
      if (childResult) {
        context.result.children.push(childResult);
      }
    }
  }

  postValidate(
    context: ParseContext<DOMMapNode, Required<DOMMapOptions>>,
  ): boolean {
    const tagName = context.element.tagName.toUpperCase();
    return isValidDOMMapElement(
      context.result,
      tagName,
      context.options.includeInteractiveOnly,
    );
  }
}

// Main parsing functions
function parseElementToStructured(
  element: Element,
  depth: number,
  options: Required<ParseOptions>,
  document: Document,
): ParsedElement | null {
  const pipeline = new StructuredParsePipeline(document);
  return parseElementWithPipeline(element, depth, options, pipeline);
}

function parseElementToDOMMap(
  element: Element,
  depth: number,
  options: Required<DOMMapOptions>,
  document: Document,
): DOMMapNode | null {
  const pipeline = new DOMMapParsePipeline(document);
  return parseElementWithPipeline(element, depth, options, pipeline);
}

// Document parsing utilities
function parseHTMLDocument(htmlString: string): Document {
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

// Metadata extraction utilities
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

// Error handling utilities
function createErrorResult(errorMessage: string): StructuredContent {
  return {
    metadata: {
      title: '',
      timestamp: new Date().toISOString(),
    },
    content: { tag: 'body', selector: 'body', children: [] },
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

function handleParsingError(
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

// Public API functions
export function parseHTMLToStructured(
  htmlString: string,
  options: ParseOptions = {},
): StructuredContent {
  const opts = { ...DEFAULT_PARSE_OPTIONS, ...options };

  try {
    if (!htmlString || typeof htmlString !== 'string') {
      return createErrorResult(
        'Invalid HTML input: input is null, undefined, or not a string',
      );
    }

    if (htmlString.length === 0) {
      return createErrorResult('Invalid HTML input: empty string');
    }

    const doc = parseHTMLDocument(htmlString);
    const bodyElement = doc.body || doc.documentElement;

    if (!bodyElement) {
      return createErrorResult('No body or document element found');
    }

    const content = parseElementToStructured(bodyElement, 0, opts, doc);

    return {
      metadata: createMetadata(doc),
      content: content || { tag: 'body', selector: 'body', children: [] },
    };
  } catch (error) {
    return handleParsingError(error, 'Error parsing HTML to structured format');
  }
}

export function parseHTMLToDOMMap(
  htmlString: string,
  options: DOMMapOptions = {},
): DOMMapResult {
  const opts = { ...DEFAULT_DOM_MAP_OPTIONS, ...options };

  try {
    if (!htmlString || typeof htmlString !== 'string') {
      return createDOMMapErrorResult(
        'Invalid HTML input: input is null, undefined, or not a string',
      );
    }

    if (htmlString.length === 0) {
      return createDOMMapErrorResult('Invalid HTML input: empty string');
    }

    const doc = parseHTMLDocument(htmlString);
    const bodyElement = doc.body || doc.documentElement;

    if (!bodyElement) {
      return createDOMMapErrorResult('No body or document element found');
    }

    const domMap = parseElementToDOMMap(bodyElement, 0, opts, doc);

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
