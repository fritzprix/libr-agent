import {
  FileParser,
  ParserError,
  ParseFailedError,
  validateFileSize,
  truncateContent,
} from './index';
import * as XLSX from 'xlsx';

// Web Worker에서 사용 가능한 console 로거
const logger = {
  debug: (message: string, data?: unknown) => {
    console.log(`[xlsx-parser][DEBUG] ${message}`, data || '');
  },
  info: (message: string, data?: unknown) => {
    console.log(`[xlsx-parser][INFO] ${message}`, data || '');
  },
  warn: (message: string, data?: unknown) => {
    console.warn(`[xlsx-parser][WARN] ${message}`, data || '');
  },
  error: (message: string, data?: unknown) => {
    console.error(`[xlsx-parser][ERROR] ${message}`, data || '');
  },
};

export class XlsxParser implements FileParser {
  supportedMimeTypes = [
    'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  ];
  supportedExtensions = ['.xlsx'];

  async parse(file: File): Promise<string> {
    try {
      validateFileSize(file);

      logger.debug('Parsing XLSX file', { name: file.name, size: file.size });

      const arrayBuffer = await file.arrayBuffer();
      const workbook = XLSX.read(arrayBuffer, { type: 'array' });

      let allText = '';
      let totalCells = 0;

      for (const sheetName of workbook.SheetNames) {
        const worksheet = workbook.Sheets[sheetName];
        const csvContent = XLSX.utils.sheet_to_csv(worksheet);

        if (csvContent.trim()) {
          allText += `=== Sheet: ${sheetName} ===\n${csvContent}\n\n`;
          totalCells += Object.keys(worksheet).filter(
            (cell) => cell !== '!ref' && cell !== '!margins',
          ).length;
        }
      }

      if (!allText.trim()) {
        throw new ParserError(
          `XLSX file ${file.name} appears to be empty or contains no data`,
          'EMPTY_CONTENT',
          { filename: file.name },
        );
      }

      const result = truncateContent(allText);

      logger.debug('XLSX file parsed successfully', {
        name: file.name,
        sheets: workbook.SheetNames.length,
        totalCells,
        originalLength: allText.length,
        finalLength: result.length,
      });

      return result;
    } catch (error) {
      logger.error('Failed to parse XLSX file', { name: file.name, error });

      if (error instanceof ParserError) {
        throw error;
      }

      throw new ParseFailedError(file.name, error as Error);
    }
  }
}
