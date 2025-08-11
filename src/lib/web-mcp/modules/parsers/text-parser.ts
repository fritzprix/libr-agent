import {
  FileParser,
  ParserError,
  ParseFailedError,
  validateFileSize,
  truncateContent,
} from './index';

// Web Worker에서 사용 가능한 console 로거
const logger = {
  debug: (message: string, data?: unknown) => {
    console.log(`[text-parser][DEBUG] ${message}`, data || '');
  },
  info: (message: string, data?: unknown) => {
    console.log(`[text-parser][INFO] ${message}`, data || '');
  },
  warn: (message: string, data?: unknown) => {
    console.warn(`[text-parser][WARN] ${message}`, data || '');
  },
  error: (message: string, data?: unknown) => {
    console.error(`[text-parser][ERROR] ${message}`, data || '');
  },
};

export class TextParser implements FileParser {
  supportedMimeTypes = [
    'text/plain',
    'text/markdown',
    'text/csv',
    'application/json',
    'text/javascript',
    'text/typescript',
    'text/html',
    'text/css',
    'text/xml',
  ];

  supportedExtensions = [
    '.txt',
    '.md',
    '.markdown',
    '.csv',
    '.json',
    '.js',
    '.ts',
    '.tsx',
    '.jsx',
    '.html',
    '.htm',
    '.css',
    '.xml',
    '.yaml',
    '.yml',
    '.log',
  ];

  async parse(file: File): Promise<string> {
    try {
      validateFileSize(file);

      logger.debug('Parsing text file', { name: file.name, size: file.size });

      const text = await file.text();
      const result = truncateContent(text);

      logger.debug('Text file parsed successfully', {
        name: file.name,
        originalLength: text.length,
        finalLength: result.length,
      });

      return result;
    } catch (error) {
      logger.error('Failed to parse text file', { name: file.name, error });

      if (error instanceof ParserError) {
        throw error;
      }

      throw new ParseFailedError(file.name, error as Error);
    }
  }
}
