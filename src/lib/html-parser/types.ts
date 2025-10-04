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

// Internal parsing interfaces
export interface BaseParseResult {
  tag: string;
  children: BaseParseResult[];
  text?: string;
}

export interface BaseParseOptions {
  maxDepth: number;
  maxTextLength: number;
}

export interface ParseContext<
  T extends BaseParseResult,
  O extends BaseParseOptions,
> {
  element: Element;
  depth: number;
  options: O;
  result: T;
}

// Parsing pipeline interface
export interface ParsePipeline<
  T extends BaseParseResult,
  O extends BaseParseOptions,
> {
  document: Document;
  preValidate(element: Element, depth: number, options: O): boolean;
  createBaseResult(element: Element): T;
  extractAttributes(context: ParseContext<T, O>): void;
  extractText(context: ParseContext<T, O>): void;
  processChildren(context: ParseContext<T, O>): void;
  postValidate(context: ParseContext<T, O>): boolean;
}

// Attribute extractor interface
export interface AttributeExtractor<T> {
  canExtract(element: Element): boolean;
  extract(element: Element): Partial<T>;
}
