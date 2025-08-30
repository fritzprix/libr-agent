/**
 * Content Store Chunker Module
 *
 * Handles text chunking for search and processing
 */

import { logger } from './logger';

interface ChunkOptions {
  chunkSize?: number;
  chunkOverlap?: number;
  preserveLines?: boolean;
}

export class TextChunker {
  private static readonly DEFAULT_CHUNK_SIZE = 1000;
  private static readonly DEFAULT_OVERLAP = 200;

  static chunkText(text: string, options: ChunkOptions = {}): string[] {
    const {
      chunkSize = TextChunker.DEFAULT_CHUNK_SIZE,
      chunkOverlap = TextChunker.DEFAULT_OVERLAP,
      preserveLines = true,
    } = options;

    if (!text || text.trim().length === 0) {
      return [];
    }

    logger.debug('Starting text chunking', {
      textLength: text.length,
      chunkSize,
      chunkOverlap,
      preserveLines,
    });

    const chunks: string[] = [];

    if (preserveLines) {
      const lines = text.split('\n');
      let currentChunk = '';
      let currentSize = 0;

      for (const line of lines) {
        const lineWithNewline = line + '\n';
        const lineSize = lineWithNewline.length;

        // If adding this line would exceed chunk size, save current chunk
        if (currentSize > 0 && currentSize + lineSize > chunkSize) {
          chunks.push(currentChunk.trim());

          // Start new chunk with overlap from previous chunk
          const overlapText = this.getOverlapText(currentChunk, chunkOverlap);
          currentChunk = overlapText + lineWithNewline;
          currentSize = currentChunk.length;
        } else {
          currentChunk += lineWithNewline;
          currentSize += lineSize;
        }
      }

      // Add the last chunk if it has content
      if (currentChunk.trim().length > 0) {
        chunks.push(currentChunk.trim());
      }
    } else {
      // Character-based chunking without line preservation
      for (let i = 0; i < text.length; i += chunkSize - chunkOverlap) {
        const end = Math.min(i + chunkSize, text.length);
        const chunk = text.slice(i, end);
        chunks.push(chunk);
      }
    }

    logger.debug('Text chunking completed', {
      originalLength: text.length,
      chunksCreated: chunks.length,
      averageChunkSize:
        chunks.length > 0
          ? Math.round(
              chunks.reduce((sum, chunk) => sum + chunk.length, 0) /
                chunks.length,
            )
          : 0,
    });

    return chunks;
  }

  private static getOverlapText(text: string, overlapSize: number): string {
    if (overlapSize <= 0 || text.length <= overlapSize) {
      return '';
    }

    // Try to find a good breaking point (end of word/sentence) within overlap
    const overlapText = text.slice(-overlapSize);
    const lastSpaceIndex = overlapText.indexOf(' ');

    if (lastSpaceIndex > 0) {
      return overlapText.slice(lastSpaceIndex + 1);
    }

    return overlapText;
  }

  /**
   * Chunks text with metadata about chunk positions
   */
  static chunkTextWithMetadata(
    text: string,
    options: ChunkOptions = {},
  ): Array<{
    content: string;
    startIndex: number;
    endIndex: number;
    position: number;
  }> {
    const chunks = this.chunkText(text, options);
    const chunksWithMetadata: Array<{
      content: string;
      startIndex: number;
      endIndex: number;
      position: number;
    }> = [];

    let currentIndex = 0;

    chunks.forEach((chunk, position) => {
      const startIndex = currentIndex;
      const endIndex = startIndex + chunk.length;

      chunksWithMetadata.push({
        content: chunk,
        startIndex,
        endIndex,
        position,
      });

      // Move to next position, accounting for overlap
      const overlap = options.chunkOverlap || TextChunker.DEFAULT_OVERLAP;
      currentIndex = Math.max(endIndex - overlap, endIndex);
    });

    return chunksWithMetadata;
  }
}
