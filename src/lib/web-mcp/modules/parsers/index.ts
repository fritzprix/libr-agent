// Worker-safe logger that falls back to console if Tauri logger is not available
const createWorkerSafeLogger = (context: string) => {
  return {
    debug: (...args: unknown[]) => {
      try {
        // Try to use Tauri logger if available
        if (
          typeof window !== 'undefined' &&
          (
            window as unknown as {
              __TAURI__?: { tauri: { debug: (msg: string) => void } };
            }
          ).__TAURI__
        ) {
          const { debug } = (
            window as unknown as {
              __TAURI__: { tauri: { debug: (msg: string) => void } };
            }
          ).__TAURI__.tauri;
          debug(`[${context}] ${args.join(' ')}`);
        } else {
          console.debug(`[${context}]`, ...args);
        }
      } catch {
        console.debug(`[${context}]`, ...args);
      }
    },
    info: (...args: unknown[]) => {
      try {
        if (
          typeof window !== 'undefined' &&
          (
            window as unknown as {
              __TAURI__?: { tauri: { info: (msg: string) => void } };
            }
          ).__TAURI__
        ) {
          const { info } = (
            window as unknown as {
              __TAURI__: { tauri: { info: (msg: string) => void } };
            }
          ).__TAURI__.tauri;
          info(`[${context}] ${args.join(' ')}`);
        } else {
          console.info(`[${context}]`, ...args);
        }
      } catch {
        console.info(`[${context}]`, ...args);
      }
    },
    warn: (...args: unknown[]) => {
      try {
        if (
          typeof window !== 'undefined' &&
          (
            window as unknown as {
              __TAURI__?: { tauri: { warn: (msg: string) => void } };
            }
          ).__TAURI__
        ) {
          const { warn } = (
            window as unknown as {
              __TAURI__: { tauri: { warn: (msg: string) => void } };
            }
          ).__TAURI__.tauri;
          warn(`[${context}] ${args.join(' ')}`);
        } else {
          console.warn(`[${context}]`, ...args);
        }
      } catch {
        console.warn(`[${context}]`, ...args);
      }
    },
    error: (...args: unknown[]) => {
      try {
        if (
          typeof window !== 'undefined' &&
          (
            window as unknown as {
              __TAURI__?: { tauri: { error: (msg: string) => void } };
            }
          ).__TAURI__
        ) {
          const { error } = (
            window as unknown as {
              __TAURI__: { tauri: { error: (msg: string) => void } };
            }
          ).__TAURI__.tauri;
          error(`[${context}] ${args.join(' ')}`);
        } else {
          console.error(`[${context}]`, ...args);
        }
      } catch {
        console.error(`[${context}]`, ...args);
      }
    },
  };
};

const logger = createWorkerSafeLogger('file-parsers');

// Custom error classes for parser-specific errors
export class ParserError extends Error {
  constructor(
    message: string,
    public readonly code: string = 'PARSER_ERROR',
    public readonly details?: Record<string, unknown>,
  ) {
    super(message);
    this.name = 'ParserError';
  }
}

export class UnsupportedFormatError extends ParserError {
  constructor(filename: string, format: string) {
    super(
      `Unsupported file format: ${format} for file ${filename}`,
      'UNSUPPORTED_FORMAT',
      {
        filename,
        format,
      },
    );
    this.name = 'UnsupportedFormatError';
  }
}

export class ParseFailedError extends ParserError {
  constructor(filename: string, originalError: Error) {
    const message = `Failed to parse ${filename}: ${originalError.message || 'Unknown error'}`;
    super(message, 'PARSE_FAILED', {
      filename,
      originalError: {
        name: originalError.name,
        message: originalError.message,
        stack: originalError.stack,
      },
    });
    this.name = 'ParseFailedError';
  }
}

// Parser interface for type safety
export interface FileParser {
  supportedMimeTypes: string[];
  supportedExtensions: string[];
  parse(file: File): Promise<string>;
}

// File size limits (in bytes) - unified with workspace limits
export const MAX_FILE_SIZE = 50 * 1024 * 1024; // 50MB (unified limit)
export const MAX_CONTENT_LENGTH = 10 * 1024 * 1024; // 10MB text content

// Utility function to validate file size
export function validateFileSize(file: File): void {
  if (file.size > MAX_FILE_SIZE) {
    throw new ParserError(
      `File size (${Math.round(file.size / 1024 / 1024)}MB) exceeds maximum allowed size (${MAX_FILE_SIZE / 1024 / 1024}MB)`,
      'FILE_TOO_LARGE',
      { fileSize: file.size, maxSize: MAX_FILE_SIZE },
    );
  }
}

// Utility function to truncate content if too long
export function truncateContent(content: string): string {
  if (content.length > MAX_CONTENT_LENGTH) {
    logger.warn('Content truncated due to size limit', {
      originalLength: content.length,
      truncatedLength: MAX_CONTENT_LENGTH,
    });
    return (
      content.substring(0, MAX_CONTENT_LENGTH) + '\n... [Content truncated]'
    );
  }
  return content;
}

// Export all parsers
export { TextParser } from './text-parser'; // Removed due to missing module
export { XlsxParser } from './xlsx-parser';
export { PdfParser } from './pdf-parser';
export { ParserFactory } from './parser-factory';
