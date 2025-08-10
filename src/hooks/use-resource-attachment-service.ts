import { useCallback } from 'react';
import { useResourceAttachment } from '@/context/ResourceAttachmentContext';
import {
  ResourceAttachmentService,
  FileProcessingOptions,
} from '@/lib/resource-attachment-service';
import { AttachmentReference } from '@/models/chat';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useResourceAttachmentService');

/**
 * Hook that combines ResourceAttachmentContext with ResourceAttachmentService
 * Provides higher-level file management operations
 */
export const useResourceAttachmentService = () => {
  const {
    files,
    addFile: addFileToContext,
    removeFile,
    clearFiles,
    isLoading,
    getFileById,
  } = useResourceAttachment();

  /**
   * Add and process a file with full MCP integration
   */
  const addAndProcessFile = useCallback(
    async (
      url: string,
      mimeType: string,
      filename?: string,
      options?: FileProcessingOptions,
    ): Promise<AttachmentReference> => {
      try {
        logger.debug('Adding and processing file', {
          url,
          mimeType,
          filename,
          options,
        });

        // Process file using service layer (this will use MCP tools when implemented)
        const processedAttachment = await ResourceAttachmentService.processFile(
          url,
          mimeType,
          filename,
          options,
        );

        // Add to context state
        await addFileToContext(url, mimeType, filename);

        logger.info('File added and processed successfully', {
          filename: processedAttachment.filename,
          contentId: processedAttachment.contentId,
        });

        return processedAttachment;
      } catch (error) {
        logger.error('Failed to add and process file', {
          url,
          mimeType,
          error,
        });
        throw error;
      }
    },
    [addFileToContext],
  );

  /**
   * Get full file content using service layer
   */
  const getFileContent = useCallback(
    async (attachment: AttachmentReference): Promise<string> => {
      try {
        logger.debug('Getting file content', {
          contentId: attachment.contentId,
        });
        return await ResourceAttachmentService.getFileContent(attachment);
      } catch (error) {
        logger.error('Failed to get file content', { attachment, error });
        throw error;
      }
    },
    [],
  );

  /**
   * Search within a specific file
   */
  const searchInFile = useCallback(
    async (
      attachment: AttachmentReference,
      query: string,
    ): Promise<
      Array<{ lineNumber: number; content: string; match: string }>
    > => {
      try {
        logger.debug('Searching in file', {
          contentId: attachment.contentId,
          query,
        });
        return await ResourceAttachmentService.searchInFile(attachment, query);
      } catch (error) {
        logger.error('Failed to search in file', { attachment, query, error });
        throw error;
      }
    },
    [],
  );

  /**
   * Search across all attached files
   */
  const searchInAllFiles = useCallback(
    async (
      query: string,
    ): Promise<
      Array<{
        attachment: AttachmentReference;
        matches: Array<{ lineNumber: number; content: string; match: string }>;
      }>
    > => {
      try {
        logger.debug('Searching in all files', {
          query,
          fileCount: files.length,
        });

        const results = await Promise.allSettled(
          files.map(async (file) => ({
            attachment: file,
            matches: await ResourceAttachmentService.searchInFile(file, query),
          })),
        );

        // Filter successful results and exclude empty matches
        const successfulResults = results
          .filter(
            (
              result,
            ): result is PromiseFulfilledResult<{
              attachment: AttachmentReference;
              matches: Array<{
                lineNumber: number;
                content: string;
                match: string;
              }>;
            }> => result.status === 'fulfilled',
          )
          .map((result) => result.value)
          .filter((result) => result.matches.length > 0);

        logger.info('Search completed', {
          query,
          totalFiles: files.length,
          filesWithMatches: successfulResults.length,
        });

        return successfulResults;
      } catch (error) {
        logger.error('Failed to search in all files', { query, error });
        throw error;
      }
    },
    [files],
  );

  /**
   * Get file statistics
   */
  const getFileStats = useCallback(() => {
    const totalSize = files.reduce((sum, file) => sum + file.size, 0);
    const totalLines = files.reduce((sum, file) => sum + file.lineCount, 0);
    const mimeTypes = [...new Set(files.map((file) => file.mimeType))];

    return {
      totalFiles: files.length,
      totalSize,
      totalLines,
      mimeTypes,
      averageSize: files.length > 0 ? totalSize / files.length : 0,
    };
  }, [files]);

  return {
    // Context methods
    files,
    removeFile,
    clearFiles,
    isLoading,
    getFileById,

    // Enhanced methods with service integration
    addAndProcessFile,
    getFileContent,
    searchInFile,
    searchInAllFiles,
    getFileStats,
  };
};
