import React, { useCallback, useEffect, useRef, useState } from 'react';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { useSessionContext } from '@/context/SessionContext';
import { useResourceAttachment } from '@/context/ResourceAttachmentContext';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { getLogger } from '@/lib/logger';

const logger = getLogger('FileAttachment');

export function useFileAttachment() {
  const [dragState, setDragState] = useState<'none' | 'valid' | 'invalid'>(
    'none',
  );
  const dropTimeoutRef = useRef<ReturnType<typeof setTimeout>>();

  const { current: currentSession } = useSessionContext();
  const {
    pendingFiles,
    addPendingFiles,
    commitPendingFiles,
    removeFile,
    clearPendingFiles,
    isLoading: isAttachmentLoading,
  } = useResourceAttachment();

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
        currentSession: currentSession?.id,
        sessionAvailable: !!currentSession,
      });

      if (!currentSession) {
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

          const supportedExtensions = /\.(txt|md|json|pdf|docx|xlsx)$/i;
          if (!supportedExtensions.test(filename)) {
            alert(`File "${filename}" format is not supported.`);
            continue;
          }

          logger.info(`Preparing dropped file`, {
            filePath,
            filename,
            sessionId: currentSession?.id,
          });

          logger.info('Calling rustBackend.readDroppedFile...', { filePath });
          const fileData = await rustBackend.readDroppedFile(filePath);
          logger.info('File data received from rustBackend', {
            dataLength: fileData.length,
            filename,
          });

          const uint8Array = new Uint8Array(fileData);
          const mimeType = getMimeType(filename);
          // Create a File object so commit step can handle both text and binary types reliably
          const fileObj = new File([uint8Array], filename, { type: mimeType });
          const blobUrl = URL.createObjectURL(fileObj);

          filesToUpload.push({
            url: blobUrl,
            mimeType,
            filename,
            file: fileObj,
            cleanup: () => URL.revokeObjectURL(blobUrl),
          });

          logger.info(`File prepared for batch upload`, {
            filename,
            filePath,
            mimeType,
            blobUrl,
          });
        } catch (error) {
          logger.error(`Error preparing dropped file ${filePath}:`, {
            filePath,
            sessionId: currentSession?.id,
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

          // Include File object when available so upload can avoid blob: URL issues in worker
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
    [currentSession, addPendingFiles, getMimeType, rustBackend],
  );

  const handleFileAttachment = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const files = e.target.files;
      if (!files || !currentSession) {
        alert('Cannot attach file: session not available.');
        return;
      }

      for (const file of files) {
        const supportedExtensions = /\.(txt|md|json|pdf|docx|xlsx)$/i;
        if (!supportedExtensions.test(file.name)) {
          alert(`File "${file.name}" format is not supported.`);
          continue;
        }

        if (file.size > 50 * 1024 * 1024) {
          alert(`File "${file.name}" is too large. Maximum size is 50MB.`);
          continue;
        }

        let fileUrl = '';
        try {
          logger.debug(`Starting file processing`, {
            filename: file.name,
            fileSize: file.size,
            fileType: file.type,
            sessionId: currentSession?.id,
          });

          fileUrl = URL.createObjectURL(file);

          addPendingFiles([
            {
              url: fileUrl,
              mimeType: file.type,
              filename: file.name,
              file: file,
              blobCleanup: () => URL.revokeObjectURL(fileUrl),
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
            sessionId: currentSession?.id,
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
          if (fileUrl) {
            URL.revokeObjectURL(fileUrl);
          }
        }
      }

      e.target.value = '';
    },
    [currentSession, addPendingFiles],
  );

  const validateFiles = useCallback((paths: string[]): boolean => {
    const supportedExtensions = /\.(txt|md|json|pdf|docx|xlsx)$/i;
    return paths.every((path: string) => {
      const filename = path.split('/').pop() || path.split('\\').pop() || '';
      return supportedExtensions.test(filename);
    });
  }, []);

  const handleFileDrop = useCallback(
    (paths: string[]) => {
      // Clear existing timeout
      if (dropTimeoutRef.current) {
        clearTimeout(dropTimeoutRef.current);
      }

      // Short delay to prevent duplicate events
      dropTimeoutRef.current = setTimeout(() => {
        logger.info('Files dropped, processing:', paths);
        processFileDrop(paths);
      }, 10);
    },
    [processFileDrop],
  );

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupDragDrop = async () => {
      try {
        logger.debug('Setting up drag and drop listener...');
        const webview = getCurrentWebview();
        unlisten = await webview.onDragDropEvent((event) => {
          logger.debug('Drag drop event received:', event);

          if (event.payload.type === 'enter') {
            const isValid = validateFiles(event.payload.paths);
            setDragState(isValid ? 'valid' : 'invalid');
          } else if (event.payload.type === 'drop') {
            setDragState('none');
            handleFileDrop(event.payload.paths);
          } else if (event.payload.type === 'leave') {
            setDragState('none');
          }
        });
        logger.debug('Drag and drop listener setup complete');
      } catch (error) {
        logger.error('Failed to setup drag and drop listener:', error);
      }
    };

    setupDragDrop();

    return () => {
      logger.debug('Cleaning up drag and drop listener...');
      if (unlisten) {
        unlisten();
      }
      if (dropTimeoutRef.current) {
        clearTimeout(dropTimeoutRef.current);
      }
    };
  }, []);

  return {
    pendingFiles,
    addPendingFiles,
    commitPendingFiles,
    removeFile,
    clearPendingFiles,
    isAttachmentLoading,
    dragState,
    handleFileAttachment,
    getMimeType,
  };
}
