import { useCallback } from 'react';
import { useSettings } from '@/hooks/use-settings';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { useAgentResourceAttachment } from '@/features/agent/hooks/useAgentResourceAttachment';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { getLogger } from '@/lib/logger';
import {
  validateFileSize,
  createFileSizeErrorMessage,
} from '@/lib/workspace-sync-service';
import { getMimeTypeFromFilename } from '@/lib/mime-utils';
import { toast } from 'sonner';
import { useTranslation } from 'react-i18next';
import type React from 'react';

const logger = getLogger('AgentFileAttachment');

/**
 * Agent V2 file attachment hook
 *
 * Bridges ResourceAttachmentContext (designed for Chat V1) to work with Agent V2.
 * Provides the same interface as useFileAttachment from Chat V1.
 */
export function useAgentFileAttachment() {
  const { t } = useTranslation();
  const { session } = useAgentSessionState();
  const {
    pendingFiles,
    addPendingFiles,
    updatePendingFile,
    commitPendingFiles,
    removeFile,
    clearPendingFiles,
    isLoading: isAttachmentLoading,
    refetchSessionFiles,
  } = useAgentResourceAttachment();

  const {
    value: { system },
  } = useSettings();

  const maxBytes = (system?.maxFileUploadSizeMB ?? 50) * 1024 * 1024;

  const rustBackend = useRustBackend();

  const getMimeType = getMimeTypeFromFilename;

  const processFileDrop = useCallback(
    async (filePaths: string[]) => {
      logger.info('processFileDrop called:', {
        filePaths,
        currentSession: session?.id,
        sessionAvailable: !!session,
      });

      if (!session) {
        logger.error('Cannot attach file: session not available.');
        toast.error(t('agent.attachment.sessionError'));
        return;
      }

      logger.info('Files dropped, processing batch:', {
        count: filePaths.length,
        paths: filePaths,
      });

      // --- STEP 1: Optimistic UI - Add placeholders immediately ---
      const placeholders = filePaths.map((filePath) => {
        const filename =
          filePath.split('/').pop() || filePath.split('\\').pop() || 'unknown';
        return {
          url: `file://${filePath}`,
          filename,
          mimeType: getMimeType(filename),
          originalPath: filePath,
          status: 'processing' as const,
        };
      });

      const addedItems = addPendingFiles(placeholders);
      logger.info('Optimistic placeholders added', {
        count: addedItems.length,
      });

      // --- STEP 2: Parallel Backend Registration ---
      try {
        await rustBackend.registerDroppedFiles(filePaths);
      } catch (error) {
        logger.error('Failed to register dropped file paths in backend', {
          error,
        });
        toast.error(t('agent.attachment.validationError'));
        // Cleanup placeholders on fatal error
        addedItems.forEach((item) => removeFile(item));
        return;
      }

      // --- STEP 3: Parallel File Processing ---
      await Promise.all(
        filePaths.map(async (filePath, index) => {
          const pendingId = addedItems[index].pendingId!;
          const filename = addedItems[index].filename;

          try {
            logger.info(`Processing dropped file: ${filename}`, { filePath });

            const fileData = await rustBackend.readDroppedFile(filePath);
            const uint8Array = new Uint8Array(fileData);
            const mimeType = getMimeType(filename);
            const fileObj = new File([uint8Array], filename, {
              type: mimeType,
            });

            if (!validateFileSize(fileObj, maxBytes)) {
              logger.warn('Dropped file exceeds size limit', {
                filename,
                fileSize: fileObj.size,
                maxBytes,
              });
              toast.error(
                createFileSizeErrorMessage(filename, fileObj.size, maxBytes),
              );
              // Remove the specific placeholder if validation fails
              removeFile(addedItems[index]);
              return;
            }

            // Update the placeholder with actual file data and set to 'pending'
            updatePendingFile(pendingId, {
              file: fileObj,
              size: fileObj.size,
              status: 'pending',
            });

            logger.info(`File processing complete: ${filename}`);
          } catch (error) {
            logger.error(`Error processing dropped file ${filename}:`, error);
            toast.error(
              t('agent.attachment.processingFileError', {
                filePath: filename,
                error: error instanceof Error ? error.message : String(error),
              }),
            );
            // Remove the specific placeholder if processing fails
            removeFile(addedItems[index]);
          }
        }),
      );

      logger.info('Batch file drop processing complete');
    },
    [
      session,
      addPendingFiles,
      updatePendingFile,
      removeFile,
      getMimeType,
      rustBackend,
      maxBytes,
      t,
    ],
  );

  const handleFileAttachment = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const files = e.target.files;
      if (!files || !session) {
        toast.error(t('agent.attachment.sessionError'));
        return;
      }

      const fileList = Array.from(files);

      // --- STEP 1: Optimistic UI - Add placeholders immediately ---
      const placeholders = fileList.map((file) => ({
        url: '',
        filename: file.name,
        mimeType: file.type,
        status: 'processing' as const,
      }));

      const addedItems = addPendingFiles(placeholders);

      // --- STEP 2: Parallel Processing ---
      await Promise.all(
        fileList.map(async (file, index) => {
          const pendingId = addedItems[index].pendingId!;

          if (!validateFileSize(file, maxBytes)) {
            toast.error(
              createFileSizeErrorMessage(file.name, file.size, maxBytes),
            );
            removeFile(addedItems[index]);
            return;
          }

          try {
            logger.debug(`Starting file processing`, {
              filename: file.name,
              fileSize: file.size,
              fileType: file.type,
              sessionId: session?.id,
            });

            // Update the placeholder with actual file data and set to 'pending'
            updatePendingFile(pendingId, {
              file: file,
              size: file.size,
              status: 'pending',
            });

            logger.info(`File processed successfully`, {
              filename: file.name,
              fileSize: file.size,
            });
          } catch (error) {
            logger.error(`Error processing file ${file.name}:`, error);
            toast.error(
              t('agent.attachment.processingFileError', {
                filePath: file.name,
                error: error instanceof Error ? error.message : String(error),
              }),
            );
            removeFile(addedItems[index]);
          }
        }),
      );

      e.target.value = '';
    },
    [session, addPendingFiles, updatePendingFile, removeFile, maxBytes, t],
  );

  const validateFiles = useCallback((paths: string[]): boolean => {
    return paths.every((path: string) => {
      const filename = path.split('/').pop() || path.split('\\').pop() || '';
      const isValid = true;
      logger.info('Validating file extension', {
        path,
        filename,
        isValid,
      });
      return isValid;
    });
  }, []);

  return {
    pendingFiles,
    addPendingFiles,
    updatePendingFile,
    commitPendingFiles,
    removeFile,
    clearPendingFiles,
    isAttachmentLoading,
    handleFileAttachment,
    getMimeType,
    processFileDrop,
    validateFiles,
    refetchSessionFiles,
  };
}
