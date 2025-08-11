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
    console.log(`[pdf-parser][DEBUG] ${message}`, data || '');
  },
  info: (message: string, data?: unknown) => {
    console.log(`[pdf-parser][INFO] ${message}`, data || '');
  },
  warn: (message: string, data?: unknown) => {
    console.warn(`[pdf-parser][WARN] ${message}`, data || '');
  },
  error: (message: string, data?: unknown) => {
    console.error(`[pdf-parser][ERROR] ${message}`, data || '');
  },
};

export class PdfParser implements FileParser {
  supportedMimeTypes = ['application/pdf'];
  supportedExtensions = ['.pdf'];

  async parse(file: File): Promise<string> {
    try {
      validateFileSize(file);

      logger.debug('Starting PDF parsing with unpdf', {
        name: file.name,
        size: file.size,
      });

      // unpdf 동적 import - 서버리스와 웹 워커 환경에 최적화됨
      const { extractText, getDocumentProxy } = await import('unpdf');

      // File을 ArrayBuffer로 변환
      const arrayBuffer = await file.arrayBuffer();
      logger.debug('File loaded into memory', {
        arrayBufferSize: arrayBuffer.byteLength,
      });

      // PDF 문서 프록시 생성
      const pdf = await getDocumentProxy(new Uint8Array(arrayBuffer));
      logger.debug('PDF document proxy created');

      // 텍스트 추출 (mergePages: false로 페이지별 배열 반환)
      const { totalPages, text: pageTexts } = await extractText(pdf, {
        mergePages: false,
      });

      logger.debug('PDF text extraction completed', {
        totalPages,
        pagesWithText: pageTexts.filter((t) => t.trim()).length,
      });

      // 페이지별 텍스트를 하나의 문자열로 병합
      let text = '';
      pageTexts.forEach((pageText, index) => {
        const pageNum = index + 1;
        if (pageText.trim()) {
          text += `\n=== Page ${pageNum} ===\n${pageText.trim()}\n`;
          logger.debug(`Page ${pageNum} processed`, {
            pageTextLength: pageText.trim().length,
          });
        } else {
          logger.debug(`Page ${pageNum} is empty or contains no text`);
        }
      });

      if (!text.trim()) {
        logger.error('PDF file appears to be empty', {
          filename: file.name,
          totalPages,
          rawTextLength: text.length,
          trimmedTextLength: text.trim().length,
        });
        throw new ParserError(
          `PDF file ${file.name} appears to be empty or contains no extractable text`,
          'EMPTY_CONTENT',
          { filename: file.name, totalPages },
        );
      }

      const result = truncateContent(text);

      logger.info('PDF parsing completed successfully', {
        name: file.name,
        pages: totalPages,
        originalLength: text.length,
        finalLength: result.length,
        firstChars: result.substring(0, 100), // 첫 100자 미리보기
      });

      return result;
    } catch (error) {
      logger.error('PDF parsing failed', {
        name: file.name,
        error:
          error instanceof Error
            ? {
                message: error.message,
                stack: error.stack,
                name: error.name,
              }
            : error,
      });

      if (error instanceof ParserError) {
        throw error;
      }

      throw new ParseFailedError(file.name, error as Error);
    }
  }
}
