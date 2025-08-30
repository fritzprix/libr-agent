/**
 * Content Store Parser Module
 *
 * Handles file parsing and content extraction
 */

import { ParserFactory } from '../parsers/parser-factory';
import { ParserError } from '../parsers/index';
import { logger } from './logger';

// Remove old parsing functions and use the new ParserFactory
export async function parseRichFile(file: File): Promise<string> {
  try {
    logger.info('Starting file parsing', {
      filename: file.name,
      size: file.size,
      type: file.type,
    });

    const result = await ParserFactory.parseFile(file);

    logger.info('File parsing completed', {
      filename: file.name,
      contentLength: result.length,
    });

    return result;
  } catch (error) {
    if (error instanceof ParserError) {
      logger.error('Parser error', {
        filename: file.name,
        error: error.message,
        code: error.code,
      });
      throw error;
    }

    logger.error('Unexpected parsing error', {
      filename: file.name,
      error: error instanceof Error ? error.message : String(error),
    });

    throw new ParserError(
      `Failed to parse file: ${error instanceof Error ? error.message : String(error)}`,
      'PARSER_UNKNOWN_ERROR',
      { filename: file.name },
    );
  }
}

export { ParserFactory, ParserError };
