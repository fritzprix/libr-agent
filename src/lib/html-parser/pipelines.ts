import type {
  ParsePipeline,
  ParseContext,
  BaseParseResult,
  BaseParseOptions,
  ParsedElement,
  ParseOptions,
  DOMMapNode,
  DOMMapOptions,
  AttributeExtractor,
} from './types';
import {
  ElementValidator,
  ChildElementProcessor,
  isValidStructuredElement,
  isValidDOMMapElement,
  extractTextContent,
  generateSelector,
  validateCSSClass,
} from './utils';
import {
  BasicAttributeExtractor,
  LinkAttributeExtractor,
  InteractiveAttributeExtractor,
  AttributeExtractorManager,
} from './attribute-extractors';

/**
 * Common parsing pipeline function that processes elements through a pipeline
 */
export function parseElementWithPipeline<
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

/**
 * Pipeline for structured content parsing
 */
export class StructuredParsePipeline
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

/**
 * Pipeline for DOM map parsing
 */
export class DOMMapParsePipeline
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
      const validClass = validateCSSClass(context.result.class);
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

// Main parsing functions for internal use
export function parseElementToStructured(
  element: Element,
  depth: number,
  options: Required<ParseOptions>,
  document: Document,
): ParsedElement | null {
  const pipeline = new StructuredParsePipeline(document);
  return parseElementWithPipeline(element, depth, options, pipeline);
}

export function parseElementToDOMMap(
  element: Element,
  depth: number,
  options: Required<DOMMapOptions>,
  document: Document,
): DOMMapNode | null {
  const pipeline = new DOMMapParsePipeline(document);
  return parseElementWithPipeline(element, depth, options, pipeline);
}
