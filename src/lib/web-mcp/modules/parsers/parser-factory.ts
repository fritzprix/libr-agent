import { FileParser, UnsupportedFormatError } from './index';
import { TextParser } from './text-parser';
import { DocxParser } from './docx-parser';
import { XlsxParser } from './xlsx-parser';
import { PdfParser } from './pdf-parser';

// Web Worker에서 사용 가능한 console 로거
const logger = {
  debug: (message: string, data?: unknown) => {
    console.log(`[parser-factory][DEBUG] ${message}`, data || '');
  },
  info: (message: string, data?: unknown) => {
    console.log(`[parser-factory][INFO] ${message}`, data || '');
  },
  warn: (message: string, data?: unknown) => {
    console.warn(`[parser-factory][WARN] ${message}`, data || '');
  },
  error: (message: string, data?: unknown) => {
    console.error(`[parser-factory][ERROR] ${message}`, data || '');
  },
};

export class ParserFactory {
  private static parsers: FileParser[] = [
    new TextParser(),
    new DocxParser(),
    new XlsxParser(),
    new PdfParser(),
  ];

  static getParser(file: File): FileParser {
    const mimeType = file.type.toLowerCase();
    const fileName = file.name.toLowerCase();

    logger.debug('Finding parser for file', { name: file.name, mimeType });

    for (const parser of this.parsers) {
      // Check by MIME type first
      if (parser.supportedMimeTypes.includes(mimeType)) {
        logger.debug('Parser found by MIME type', {
          parser: parser.constructor.name,
          mimeType,
        });
        return parser;
      }

      // Fallback to file extension
      const hasMatchingExtension = parser.supportedExtensions.some((ext) =>
        fileName.endsWith(ext),
      );
      if (hasMatchingExtension) {
        logger.debug('Parser found by extension', {
          parser: parser.constructor.name,
          fileName,
        });
        return parser;
      }
    }

    throw new UnsupportedFormatError(file.name, mimeType || 'unknown');
  }

  static async parseFile(file: File): Promise<string> {
    const parser = this.getParser(file);
    return await parser.parse(file);
  }

  static getSupportedFormats(): { mimeTypes: string[]; extensions: string[] } {
    const allMimeTypes = new Set<string>();
    const allExtensions = new Set<string>();

    for (const parser of this.parsers) {
      parser.supportedMimeTypes.forEach((type) => allMimeTypes.add(type));
      parser.supportedExtensions.forEach((ext) => allExtensions.add(ext));
    }

    return {
      mimeTypes: Array.from(allMimeTypes),
      extensions: Array.from(allExtensions),
    };
  }
}
