import type { DOMMapOptions, DOMMapResult, PageMetadata } from './types';
import {
  validateHtmlInput,
  parseHTMLDocument,
  extractTitle,
  extractMetaURL,
  createDOMMapErrorResult,
  handleDOMMapError,
} from './utils';
import { parseElementToDOMMap } from './pipelines';
import { DEFAULT_DOM_MAP_OPTIONS } from './constants';

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
      throw new Error(validationError);
    }

    const doc = parseHTMLDocument(htmlString);
    return {
      title: extractTitle(doc),
      url: extractMetaURL(doc),
      timestamp: new Date().toISOString(),
    };
  } catch {
    return {
      title: '',
      timestamp: new Date().toISOString(),
    };
  }
}
