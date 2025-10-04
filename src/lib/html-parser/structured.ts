import type { ParseOptions, StructuredContent } from './types';
import {
  validateHtmlInput,
  parseHTMLDocument,
  createMetadata,
  createErrorResult,
  handleParsingError,
} from './utils';
import { parseElementToStructured } from './pipelines';
import { DEFAULT_PARSE_OPTIONS } from './constants';

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
