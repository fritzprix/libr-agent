import {
  FileParser,
  ParserError,
  ParseFailedError,
  validateFileSize,
  truncateContent,
} from './index';
import mammoth from 'mammoth';

// Web Worker에서 사용 가능한 console 로거
const logger = {
  debug: (message: string, data?: unknown) => {
    console.log(`[docx-parser][DEBUG] ${message}`, data || '');
  },
  info: (message: string, data?: unknown) => {
    console.log(`[docx-parser][INFO] ${message}`, data || '');
  },
  warn: (message: string, data?: unknown) => {
    console.warn(`[docx-parser][WARN] ${message}`, data || '');
  },
  error: (message: string, data?: unknown) => {
    console.error(`[docx-parser][ERROR] ${message}`, data || '');
  },
};

export class DocxParser implements FileParser {
  supportedMimeTypes = [
    'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
  ];
  supportedExtensions = ['.docx'];

  async parse(file: File): Promise<string> {
    try {
      validateFileSize(file);

      logger.debug('Parsing DOCX file', { name: file.name, size: file.size });

      const arrayBuffer = await file.arrayBuffer();
      const result = await mammoth.extractRawText({ arrayBuffer });

      if (!result.value.trim()) {
        throw new ParserError(
          `DOCX file ${file.name} appears to be empty or contains no extractable text`,
          'EMPTY_CONTENT',
          { filename: file.name },
        );
      }

      const text = truncateContent(result.value);

      logger.debug('DOCX file parsed successfully', {
        name: file.name,
        originalLength: result.value.length,
        finalLength: text.length,
        warnings: result.messages.length,
      });

      return text;
    } catch (error) {
      logger.error('Failed to parse DOCX file', { name: file.name, error });

      if (error instanceof ParserError) {
        throw error;
      }

      throw new ParseFailedError(file.name, error as Error);
    }
  }
}
