import { getLogger } from '@/lib/logger';
import { createCompactText } from '@/lib/text-utils';
import { buildUniqueSelector, isValidCSSIdentifier } from '@/lib/dom/selector';

const logger = getLogger('HTMLParser');

/**
 * Represents a parsed HTML element with its core attributes and content.
 * This interface is used for creating a structured, simplified representation of a DOM tree.
 */
export interface ParsedElement {
  /** The tag name of the element (e.g., 'div', 'p'). */
  tag: string;
  /** A unique CSS selector for the element. */
  selector: string;
  /** The direct text content of the element, if any. */
  text?: string;
  /** The ID of the element, if it has one. */
  id?: string;
  /** The class attribute of the element. */
  class?: string;
  /** The href attribute, typically for anchor tags. */
  href?: string;
  /** The src attribute, for images, scripts, etc. */
  src?: string;
  /** The alt attribute, for images. */
  alt?: string;
  /** The title attribute of the element. */
  title?: string;
  /** An array of child elements, recursively structured. */
  children: ParsedElement[];
}

/**
 * Represents a node in the DOM map, which is a detailed, interactive-focused
 * representation of the DOM.
 */
export interface DOMMapNode {
  /** The tag name of the element. */
  tag: string;
  /** A unique CSS selector for the element. */
  selector: string;
  /** The ID of the element. */
  id?: string;
  /** The class attribute of the element. */
  class?: string;
  /** The text content of the element. */
  text?: string;
  /** The `type` attribute, mainly for input elements. */
  type?: string;
  /** The `href` attribute for links. */
  href?: string;
  /** The `placeholder` attribute for input fields. */
  placeholder?: string;
  /** The `value` of an input element. */
  value?: string;
  /** The `name` attribute of a form element. */
  name?: string;
  /** The ARIA role of the element. */
  role?: string;
  /** The `aria-label` attribute. */
  ariaLabel?: string;
  /** An array of child nodes in the DOM map. */
  children: DOMMapNode[];
}

/**
 * Contains metadata about a parsed HTML page, such as its title and URL.
 */
export interface PageMetadata {
  /** The title of the HTML page. */
  title: string;
  /** The canonical or Open Graph URL of the page. */
  url?: string;
  /** The timestamp of when the metadata was extracted. */
  timestamp: string;
}

/**
 * Defines the options available for the structured parsing process.
 */
export interface ParseOptions {
  /** The maximum depth to traverse the DOM tree. */
  maxDepth?: number;
  /** Whether to include link (`href`) and source (`src`) attributes in the output. */
  includeLinks?: boolean;
  /** The maximum length for extracted text content. */
  maxTextLength?: number;
}

/**
 * Defines the options available for creating a DOM map.
 */
export interface DOMMapOptions {
  /** The maximum depth to traverse the DOM tree. */
  maxDepth?: number;
  /** The maximum number of child elements to process for each node. */
  maxChildren?: number;
  /** The maximum length for extracted text content. */
  maxTextLength?: number;
  /** If true, only elements deemed "interactive" (e.g., buttons, links, inputs) will be included. */
  includeInteractiveOnly?: boolean;
}

/**
 * Represents an interactable element on the page, such as a button or input field.
 */
export interface InteractableElement {
  /** A unique CSS selector for the element. */
  selector: string;
  /** The type of interactable element. */
  type: 'button' | 'input' | 'select' | 'link' | 'textarea';
  /** The text content or label associated with the element. */
  text?: string;
  /** A boolean indicating if the element is enabled. */
  enabled: boolean;
  /** A boolean indicating if the element is visible. */
  visible: boolean;
  /** The `type` attribute for input elements (e.g., 'text', 'checkbox'). */
  inputType?: string;
  /** The current value of the element, for inputs. */
  value?: string;
  /** The placeholder text for input fields. */
  placeholder?: string;
}

/**
 * Defines the options for extracting interactable elements.
 */
export interface InteractableOptions {
  /** If true, hidden elements will be included in the result. */
  includeHidden?: boolean;
  /** The maximum number of interactable elements to return. */
  maxElements?: number;
}

/**
 * The result of an interactable element extraction process.
 */
export interface InteractableResult {
  /** An array of the interactable elements found. */
  elements: InteractableElement[];
  /** Metadata about the extraction process. */
  metadata: {
    /** The timestamp of when the extraction occurred. */
    extraction_timestamp: string;
    /** The total number of elements found. */
    total_count: number;
    /** The CSS selector that defined the scope of the search. */
    scope_selector: string;
    /** Performance metrics for the extraction. */
    performance: {
      /** The time taken for the extraction in milliseconds. */
      execution_time_ms: number;
      /** The size of the resulting data in bytes. */
      data_size_bytes: number;
    };
  };
  /** An error message, if an error occurred during extraction. */
  error?: string;
}

/**
 * The result of a structured parsing process, containing the parsed content
 * and metadata about the page.
 */
export interface StructuredContent {
  /** The metadata of the parsed page. */
  metadata: PageMetadata;
  /** The root element of the parsed content. */
  content: ParsedElement;
  /** An error message, if an error occurred during parsing. */
  error?: string;
}

/**
 * The result of a DOM map creation process.
 */
export interface DOMMapResult {
  /** The URL of the page. */
  url?: string;
  /** The title of the page. */
  title?: string;
  /** The timestamp of when the DOM map was created. */
  timestamp: string;
  /** The selector of the root element of the map. */
  selector?: string;
  /** The root node of the DOM map. */
  domMap: DOMMapNode;
  /** The format identifier, always 'dom-map'. */
  format: 'dom-map';
  /** An error message, if an error occurred. */
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
  return element.tagName.toUpperCase() === 'INPUT' && 'value' in element;
}

// Centralized HTML input validation
function validateHtmlInput(html: string): string | null {
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

// Unified selector builder - replaces multiple selector generation functions
// Removed local buildUniqueSelector implementation; using shared utility from '@/lib/dom/selector'

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
  // use shared utility for consistency
  return buildUniqueSelector(element, doc);
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

/**
 * Parses an HTML string into a structured, simplified tree of `ParsedElement` objects.
 * This function is designed to create a clean, content-focused representation of the HTML.
 *
 * @param htmlString The HTML string to parse.
 * @param options Optional parsing options to control the output.
 * @returns A `StructuredContent` object containing the parsed content and metadata.
 */
export function parseHTMLToStructured(
  htmlString: string,
  options: ParseOptions = {},
): StructuredContent {
  const opts = { ...DEFAULT_PARSE_OPTIONS, ...options };

  try {
    const validationError = validateHtmlInput(htmlString);
    if (validationError) {
      return createErrorResult(validationError);
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

/**
 * Parses an HTML string into a detailed DOM map, focusing on interactable elements.
 * The DOM map provides a rich, structured view of the page's interactive components.
 *
 * @param htmlString The HTML string to parse.
 * @param options Optional options to control the DOM map creation.
 * @returns A `DOMMapResult` object containing the DOM map and metadata.
 */
export function parseHTMLToDOMMap(
  htmlString: string,
  options: DOMMapOptions = {},
): DOMMapResult {
  const opts = { ...DEFAULT_DOM_MAP_OPTIONS, ...options };

  try {
    const validationError = validateHtmlInput(htmlString);
    if (validationError) {
      return createDOMMapErrorResult(validationError);
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

/**
 * Extracts metadata (title, URL, timestamp) from an HTML string.
 *
 * @param htmlString The HTML string to extract metadata from.
 * @returns A `PageMetadata` object. Returns an empty object on failure.
 */
export function extractHTMLMetadata(htmlString: string): PageMetadata {
  try {
    const validationError = validateHtmlInput(htmlString);
    if (validationError) {
      throw new HTMLParseError(validationError);
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

// Interactable elements parsing
const DEFAULT_INTERACTABLE_OPTIONS: Required<InteractableOptions> = {
  includeHidden: false,
  maxElements: 100,
};

const INTERACTABLE_SELECTORS = [
  'button:not([disabled])',
  'input:not([disabled]):not([type="hidden"])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  'a[href]:not([href="#"]):not([href=""])',
  '[role="button"]:not([aria-disabled="true"])',
  '[onclick]',
  '[data-action]',
].join(',');

function generateUniqueSelector(element: Element, document: Document): string {
  // Use the unified selector builder for consistency
  return buildUniqueSelector(element, document);
}

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

function parseElementToInteractable(
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
    selector: generateUniqueSelector(element, document),
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
