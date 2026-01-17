import { useCallback } from 'react';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { useAgentResourceAttachment } from '@/features/agent/hooks/useAgentResourceAttachment';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { getLogger } from '@/lib/logger';
import {
  validateFileSize,
  createFileSizeErrorMessage,
} from '@/lib/workspace-sync-service';
import type React from 'react';

const logger = getLogger('AgentFileAttachment');

/**
 * Agent V2 file attachment hook
 *
 * Bridges ResourceAttachmentContext (designed for Chat V1) to work with Agent V2.
 * Provides the same interface as useFileAttachment from Chat V1.
 */
export function useAgentFileAttachment() {
  const { session } = useAgentSessionState();
  const {
    pendingFiles,
    addPendingFiles,
    commitPendingFiles,
    removeFile,
    clearPendingFiles,
    isLoading: isAttachmentLoading,
  } = useAgentResourceAttachment();

  const rustBackend = useRustBackend();

  const getMimeType = useCallback((filename: string): string => {
    const ext = filename.toLowerCase().split('.').pop();
    switch (ext) {
      case 'txt':
        return 'text/plain';
      case 'md':
        return 'text/markdown';
      case 'json':
        return 'application/json';
      case 'pdf':
        return 'application/pdf';
      case 'docx':
        return 'application/vnd.openxmlformats-officedocument.wordprocessingml.document';
      case 'xlsx':
        return 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet';
      default:
        return 'application/octet-stream';
    }
  }, []);

  const processFileDrop = useCallback(
    async (filePaths: string[]) => {
      logger.info('processFileDrop called:', {
        filePaths,
        currentSession: session?.id,
        sessionAvailable: !!session,
      });

      if (!session) {
        logger.error('Cannot attach file: session not available.');
        alert('Cannot attach file: session not available.');
        return;
      }

      logger.info('Files dropped, processing batch:', {
        count: filePaths.length,
        paths: filePaths,
      });

      const filesToUpload: Array<{
        url: string;
        mimeType: string;
        filename: string;
        file: File;
        cleanup: () => void;
      }> = [];

      for (const filePath of filePaths) {
        try {
          const filename =
            filePath.split('/').pop() ||
            filePath.split('\\').pop() ||
            'unknown';

          // ALLOW ALL FILES - Validation happens at commit stage for Content Store support
          // const supportedExtensions = /\.(txt|md|json|pdf|docx|xlsx)$/i;

          logger.info('Processing dropped file', {
            filePath,
            filename,
            // supportedExtensions: supportedExtensions.source,
          });

          // if (!supportedExtensions.test(filename)) {
          //   logger.info('Unsupported file format', { filename });
          //   alert(`File "${filename}" format is not supported.`);
          //   continue;
          // }

          logger.info(`Preparing dropped file`, {
            filePath,
            filename,
            sessionId: session?.id,
          });

          logger.info('Calling rustBackend.readDroppedFile...', { filePath });
          const fileData = await rustBackend.readDroppedFile(filePath);
          logger.info('File data received from rustBackend', {
            dataLength: fileData.length,
            filename,
          });

          const uint8Array = new Uint8Array(fileData);
          const mimeType = getMimeType(filename);
          const fileObj = new File([uint8Array], filename, { type: mimeType });

          if (!validateFileSize(fileObj)) {
            logger.warn('Dropped file exceeds size limit', {
              filename,
              fileSize: fileObj.size,
            });
            alert(createFileSizeErrorMessage(filename, fileObj.size));
            continue;
          }

          filesToUpload.push({
            url: `file://${filePath}`,
            mimeType,
            filename,
            file: fileObj,
            cleanup: () => {},
          });

          logger.info(`File prepared for batch upload`, {
            filename,
            filePath,
            mimeType,
            fileUrl: `file://${filePath}`,
          });
        } catch (error) {
          logger.error(`Error preparing dropped file ${filePath}:`, {
            filePath,
            sessionId: session?.id,
            error:
              error instanceof Error
                ? {
                    message: error.message,
                    stack: error.stack,
                    name: error.name,
                  }
                : error,
            errorString: String(error),
          });
          alert(
            `Error processing file "${filePath}": ${error instanceof Error ? error.message : String(error)}`,
          );
        }
      }

      logger.info('Files prepared for upload:', {
        count: filesToUpload.length,
      });

      if (filesToUpload.length > 0) {
        try {
          const batchFiles = filesToUpload.map((file) => ({
            url: file.url,
            mimeType: file.mimeType,
            filename: file.filename,
            file: file.file,
            blobCleanup: file.cleanup,
          }));

          logger.info('Adding files to pending state', {
            count: batchFiles.length,
            files: batchFiles.map((f) => ({
              filename: f.filename,
              mimeType: f.mimeType,
            })),
          });

          addPendingFiles(batchFiles);

          logger.info('Files added to pending state successfully', {
            total: batchFiles.length,
          });
        } catch (error) {
          logger.error('Failed to add files to pending state:', error);
          alert(
            `Error processing files: ${error instanceof Error ? error.message : String(error)}`,
          );
          filesToUpload.forEach((file) => file.cleanup());
        }
      }
    },
    [session, addPendingFiles, getMimeType, rustBackend],
  );

  const handleFileAttachment = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const files = e.target.files;
      if (!files || !session) {
        alert('Cannot attach file: session not available.');
        return;
      }

      for (const file of files) {
        // ALLOW ALL FILES - Validation happens at commit stage for Content Store support
        // const supportedExtensions = /\.(txt|md|json|pdf|docx|xlsx)$/i;
        // if (!supportedExtensions.test(file.name)) {
        //   alert(`File "${file.name}" format is not supported.`);
        //   continue;
        // }

        if (!validateFileSize(file)) {
          alert(createFileSizeErrorMessage(file.name, file.size));
          continue;
        }

        try {
          logger.debug(`Starting file processing`, {
            filename: file.name,
            fileSize: file.size,
            fileType: file.type,
            sessionId: session?.id,
          });

          addPendingFiles([
            {
              url: '',
              mimeType: file.type,
              filename: file.name,
              file: file,
              blobCleanup: () => {},
            },
          ]);

          logger.info(`File processed successfully`, {
            filename: file.name,
            fileSize: file.size,
          });
        } catch (error) {
          logger.error(`Error processing file ${file.name}:`, {
            filename: file.name,
            fileSize: file.size,
            fileType: file.type,
            sessionId: session?.id,
            error:
              error instanceof Error
                ? {
                    message: error.message,
                    stack: error.stack,
                    name: error.name,
                  }
                : error,
            errorString: String(error),
          });
          alert(
            `Error processing file "${file.name}": ${error instanceof Error ? error.message : String(error)}`,
          );
        }
      }

      e.target.value = '';
    },
    [session, addPendingFiles],
  );

  const validateFiles = useCallback((paths: string[]): boolean => {
    // ALLOW ALL FILES
    // const supportedExtensions = /\.(txt|md|json|pdf|docx|xlsx)$/i;
    return paths.every((path: string) => {
      const filename = path.split('/').pop() || path.split('\\').pop() || '';
      // const isValid = supportedExtensions.test(filename);
      const isValid = true;
      logger.info('Validating file extension', {
        path,
        filename,
        isValid,
        // supportedExtensions: supportedExtensions.source,
      });
      return isValid;
    });
  }, []);

  return {
    pendingFiles,
    addPendingFiles,
    commitPendingFiles,
    removeFile,
    clearPendingFiles,
    isAttachmentLoading,
    handleFileAttachment,
    getMimeType,
    processFileDrop,
    validateFiles,
  };
}
