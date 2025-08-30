import React, {
  createContext,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { AttachmentReference } from '@/models/chat';
import { getLogger } from '@/lib/logger';
import { useWebMCPServer } from '@/hooks/use-web-mcp-server';
import {
  AddContentOutput,
  ContentStoreServer,
} from '@/lib/web-mcp/modules/content-store';
import { useSessionContext } from './SessionContext';

const logger = getLogger('ResourceAttachmentContext');

interface ResourceAttachmentContextType {
  files: AttachmentReference[];
  /**
   * Add a file to the attachment list
   *
   * @param url - Any file URL (blob:, http:, https:, data:, etc.)
   *              External URLs will be automatically converted to blob URLs to avoid CORS issues
   * @param mimeType - MIME type of the file (optional, will be auto-detected for external URLs)
   * @param filename - Optional filename, will be extracted from URL if not provided
   *
   * Note: Files are managed per session. Duplicate content is detected globally by the server.
   * Same content cannot be uploaded multiple times across different sessions.
   *
   * CORS handling: External URLs are automatically fetched and converted to blob URLs,
   * eliminating CORS issues that would occur in Web Workers.
   */
  addFile: (
    url: string,
    mimeType: string,
    filename?: string,
  ) => Promise<AttachmentReference>;
  removeFile: (ref: AttachmentReference) => Promise<void>;
  clearFiles: () => void;
  isLoading: boolean;
  /**
   * Get file by its unique content ID
   * @param id - The contentId of the file to find
   */
  getFileById: (id: string) => AttachmentReference | undefined;

  // Session files management
  /**
   * All files stored in the current session's store
   */
  sessionFiles: AttachmentReference[];
  /**
   * Loading state for session files
   */
  isSessionFilesLoading: boolean;
  /**
   * Error state for session files
   */
  sessionFilesError: string | null;
  /**
   * Refresh session files from the server
   */
  refreshSessionFiles: () => Promise<void>;
}

const ResourceAttachmentContext = createContext<
  ResourceAttachmentContextType | undefined
>(undefined);

interface ResourceAttachmentProviderProps {
  children: ReactNode;
}

export const ResourceAttachmentProvider: React.FC<
  ResourceAttachmentProviderProps
> = ({ children }) => {
  const [files, setFiles] = useState<AttachmentReference[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  // Session files management states
  const [sessionFiles, setSessionFiles] = useState<AttachmentReference[]>([]);
  const [isSessionFilesLoading, setIsSessionFilesLoading] = useState(false);
  const [sessionFilesError, setSessionFilesError] = useState<string | null>(
    null,
  );

  const { current: currentSession, updateSession } = useSessionContext();
  const { server } = useWebMCPServer<ContentStoreServer>('content-store');

  // Track uploaded filenames to prevent duplicate uploads
  const uploadedFilenamesRef = useRef<string[]>([]);

  // Reset files when session changes
  const prevSessionIdRef = useRef<string | undefined>();

  // Refresh session files from the server
  const refreshSessionFiles = useCallback(async () => {
    if (!server || !currentSession?.storeId) {
      setSessionFiles([]);
      return;
    }

    setIsSessionFilesLoading(true);
    setSessionFilesError(null);

    try {
      logger.debug('Refreshing session files', {
        storeId: currentSession.storeId,
        sessionId: currentSession.id,
      });

      const result = await server.listContent({
        storeId: currentSession.storeId,
      });

      const allSessionFiles: AttachmentReference[] =
        result?.contents?.map((content) => ({
          storeId: content.storeId,
          contentId: content.contentId,
          filename: content.filename,
          mimeType: content.mimeType,
          size: content.size,
          lineCount: content.lineCount || 0,
          preview: content.preview,
          uploadedAt: content.uploadedAt || new Date().toISOString(),
          chunkCount: content.chunkCount,
          lastAccessedAt: content.lastAccessedAt,
        })) || [];

      setSessionFiles(allSessionFiles);

      logger.info('Session files refreshed successfully', {
        storeId: currentSession.storeId,
        fileCount: allSessionFiles.length,
      });
    } catch (error) {
      logger.error('Failed to refresh session files', {
        storeId: currentSession.storeId,
        error:
          error instanceof Error
            ? {
                message: error.message,
                stack: error.stack,
                name: error.name,
              }
            : error,
      });
      setSessionFilesError(
        error instanceof Error ? error.message : 'Failed to load session files',
      );
      setSessionFiles([]);
    } finally {
      setIsSessionFilesLoading(false);
    }
  }, [server, currentSession?.storeId, currentSession?.id]);

  useEffect(() => {
    if (currentSession?.id !== prevSessionIdRef.current) {
      logger.info('Session changed in ResourceAttachmentContext', {
        previousSessionId: prevSessionIdRef.current,
        currentSessionId: currentSession?.id,
        sessionName: currentSession?.name,
        assistants: currentSession?.assistants?.map((a) => a.name),
        reason: !prevSessionIdRef.current
          ? 'initial_session'
          : 'session_change',
      });
      setFiles([]);
      // Clear uploaded filenames when session changes
      uploadedFilenamesRef.current = [];

      // Clear and refresh session files
      setSessionFiles([]);
      setSessionFilesError(null);

      prevSessionIdRef.current = currentSession?.id;
    }
  }, [currentSession?.id]);

  // Auto-refresh session files when storeId is available
  useEffect(() => {
    if (currentSession?.storeId) {
      refreshSessionFiles();
    }
  }, [currentSession?.storeId, refreshSessionFiles]);

  // Ensure store exists for current session
  const ensureStoreExists = useCallback(
    async (sessionId: string): Promise<string> => {
      if (!server) {
        throw new Error('Content store server is not initialized.');
      }

      try {
        // Check if session already has a storeId
        if (currentSession?.storeId) {
          logger.debug('Using existing store ID from session', {
            sessionId,
            storeId: currentSession.storeId,
          });
          return currentSession.storeId;
        }

        // Create a new store
        logger.debug('Creating new content store', { sessionId });
        const createResult = await server.createStore({
          metadata: {
            sessionId,
          },
        });

        logger.debug('createStore result received', {
          sessionId,
          createResult,
          createResultType: typeof createResult,
          createResultKeys: createResult ? Object.keys(createResult) : null,
          storeIdValue: createResult?.storeId,
          storeIdType: typeof createResult?.storeId,
        });

        const storeId = createResult.storeId;

        // Update the session with the new storeId
        await updateSession(sessionId, { storeId });

        logger.info('Content store created and session updated', {
          sessionId,
          storeId,
        });

        return storeId;
      } catch (error) {
        logger.error('Failed to ensure content store exists', {
          sessionId,
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
        throw new Error(
          `Failed to ensure content store exists: ${error instanceof Error ? error.message : String(error)}`,
        );
      }
    },
    [server, currentSession, updateSession],
  );

  // Helper function to extract filename from URL
  const extractFilenameFromUrl = useCallback((url: string): string => {
    try {
      const urlObj = new URL(url);
      const pathname = urlObj.pathname;
      const filename = pathname.split('/').pop() || 'unknown_file';
      return filename;
    } catch {
      return `file_${Date.now()}`;
    }
  }, []);

  // Helper function to convert any URL to blob URL
  const convertToBlobUrl = useCallback(
    async (
      url: string,
    ): Promise<{
      blobUrl: string;
      cleanup: () => void;
      size: number;
      type: string;
    }> => {
      try {
        // If it's already a blob URL, return as is
        if (url.startsWith('blob:')) {
          return {
            blobUrl: url,
            cleanup: () => {}, // No cleanup needed for existing blob URLs
            size: 0, // Size unknown for existing blob URLs
            type: '', // Type unknown for existing blob URLs
          };
        }

        // For external URLs, fetch and convert to blob
        logger.debug('Converting external URL to blob', { url });
        const response = await fetch(url);

        if (!response.ok) {
          throw new Error(
            `Failed to fetch ${url}: ${response.status} ${response.statusText}`,
          );
        }

        const blob = await response.blob();
        const blobUrl = URL.createObjectURL(blob);

        logger.debug('Successfully converted to blob URL', {
          originalUrl: url,
          blobUrl,
          size: blob.size,
          type: blob.type,
        });

        return {
          blobUrl,
          cleanup: () => URL.revokeObjectURL(blobUrl),
          size: blob.size,
          type: blob.type,
        };
      } catch (error) {
        logger.error('Failed to convert URL to blob', {
          url,
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
        throw new Error(
          `Failed to process URL "${url}": ${error instanceof Error ? error.message : String(error)}`,
        );
      }
    },
    [],
  );

  const addFile = useCallback(
    async (
      url: string,
      mimeType: string,
      filename?: string,
    ): Promise<AttachmentReference> => {
      // Use the provided filename or extract from original URL
      const actualFilename = filename || extractFilenameFromUrl(url);

      // Check if file is already being uploaded or uploaded to prevent duplicates
      if (uploadedFilenamesRef.current.includes(actualFilename)) {
        logger.warn('File upload already in progress or completed', {
          filename: actualFilename,
        });
        const existingFile = files.find((f) => f.filename === actualFilename);
        if (existingFile) {
          return existingFile;
        }
        throw new Error(`File "${actualFilename}" is already being uploaded`);
      }

      // Log current state for debugging
      logger.debug('Adding file', {
        filename: actualFilename,
        sessionId: currentSession?.id,
      });

      if (!server || !currentSession?.id) {
        const errorDetails = {
          serverAvailable: !!server,
          sessionAvailable: !!currentSession,
          sessionId: currentSession?.id,
          filename: actualFilename,
        };

        logger.error(
          'Content store server or session not available',
          errorDetails,
        );

        if (!server) {
          throw new Error(
            'Content store server is not initialized. Please check WebMCP server status.',
          );
        }
        if (!currentSession?.id) {
          throw new Error(
            'No active session found. Please create or select a session first.',
          );
        }

        throw new Error('Content store server or session not available');
      }

      setIsLoading(true);
      let blobCleanup: (() => void) | null = null;

      try {
        // Add filename to uploaded list to prevent duplicates
        uploadedFilenamesRef.current.push(actualFilename);

        // 1. Ensure store exists for this session
        const storeId = await ensureStoreExists(currentSession.id);

        logger.debug('Adding file to store', {
          filename: actualFilename,
          storeId,
        });

        // Convert any URL to blob URL for consistent processing
        const blobResult = await convertToBlobUrl(url);
        blobCleanup = blobResult.cleanup;

        // Use detected MIME type if not provided or if we have more accurate info
        const actualMimeType =
          mimeType || blobResult.type || 'application/octet-stream';

        // Check if file already exists in current session (by filename)
        // Note: This prevents duplicate files with the same name regardless of content
        const existingFile = files.find(
          (file) => file.filename === actualFilename,
        );

        if (existingFile) {
          logger.warn('File already exists in current session', {
            filename: actualFilename,
          });
          return existingFile;
        }

        // Call the content-store server to add content using blob URL
        // This ensures CORS issues are avoided and processing is consistent
        const result: AddContentOutput = await server.addContent({
          storeId: storeId,
          fileUrl: blobResult.blobUrl,
          metadata: {
            filename: actualFilename,
            mimeType: actualMimeType,
            size: blobResult.size || 0,
            uploadedAt: new Date().toISOString(),
          },
        });

        // Convert AddContentOutput to AttachmentReference
        const attachment: AttachmentReference = {
          storeId: result.storeId,
          contentId: result.contentId,
          filename: result.filename,
          mimeType: result.mimeType,
          size: result.size,
          lineCount: result.lineCount,
          preview: result.preview,
          uploadedAt: result.uploadedAt instanceof Date ? result.uploadedAt.toISOString() : result.uploadedAt,
          chunkCount: result.chunkCount,
          lastAccessedAt: new Date().toISOString(),
        };

        // Add to files array
        setFiles((prevFiles) => [...prevFiles, attachment]);

        logger.info('File attachment added successfully', {
          filename: attachment.filename,
          contentId: attachment.contentId,
          chunkCount: attachment.chunkCount,
          originalUrl: url,
          wasConverted: !url.startsWith('blob:'),
        });

        // Refresh session files after successful file addition
        await refreshSessionFiles();

        return attachment;
      } catch (error) {
        // Remove filename from uploaded list on error
        uploadedFilenamesRef.current = uploadedFilenamesRef.current.filter(
          (f) => f !== actualFilename,
        );

        const errorMsg = error instanceof Error ? error.message : String(error);

        // Handle duplicate document errors gracefully
        if (
          errorMsg.includes('Duplicate document encountered') ||
          errorMsg.includes('winkBM25S') ||
          errorMsg.includes('Duplicate document')
        ) {
          logger.warn('Duplicate file content detected by server', {
            filename: actualFilename,
            sessionId: currentSession?.id,
            error: errorMsg,
            filesInCurrentSession: files.length,
          });

          // Check if we already have this file in our current session's state
          const existingFile = files.find((f) => f.filename === actualFilename);
          if (existingFile) {
            logger.info('Returning existing file from current session', {
              contentId: existingFile.contentId,
              filename: existingFile.filename,
              sessionId: currentSession?.id,
            });
            return existingFile;
          }

          // File exists globally but not in current session - inform user
          throw new Error(
            `이 파일은 다른 세션에서 이미 업로드되었습니다. 같은 내용의 파일은 한 번만 업로드할 수 있습니다. 파일명: "${actualFilename}"`,
          );
        }

        logger.error('Failed to add file attachment', {
          url,
          mimeType,
          filename: actualFilename,
          error:
            error instanceof Error
              ? {
                  message: error.message,
                  stack: error.stack,
                  name: error.name,
                }
              : error,
          errorString: String(error),
          sessionId: currentSession?.id,
          serverAvailable: !!server,
        });
        throw new Error(
          `Failed to add file: ${error instanceof Error ? error.message : String(error)}`,
        );
      } finally {
        // Clean up blob URL if we created it
        if (blobCleanup) {
          blobCleanup();
        }
        setIsLoading(false);
      }
    },
    [
      files,
      extractFilenameFromUrl,
      convertToBlobUrl,
      server,
      currentSession,
      ensureStoreExists,
      refreshSessionFiles,
    ],
  );

  const removeFile = useCallback(
    async (ref: AttachmentReference): Promise<void> => {
      try {
        logger.debug('Removing file attachment', {
          contentId: ref.contentId,
          filename: ref.filename,
        });

        setFiles((prevFiles) =>
          prevFiles.filter((file) => file.contentId !== ref.contentId),
        );

        // Remove from uploaded filenames ref as well
        uploadedFilenamesRef.current = uploadedFilenamesRef.current.filter(
          (f) => f !== ref.filename,
        );

        logger.info('File attachment removed successfully', {
          filename: ref.filename,
          contentId: ref.contentId,
        });

        // Refresh session files after removal
        await refreshSessionFiles();
      } catch (error) {
        logger.error('Failed to remove file attachment', {
          ref: {
            contentId: ref.contentId,
            filename: ref.filename,
            storeId: ref.storeId,
          },
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
        throw new Error(
          `Failed to remove file: ${error instanceof Error ? error.message : String(error)}`,
        );
      }
    },
    [refreshSessionFiles],
  );

  const clearFiles = useCallback(() => {
    logger.debug('Clearing all file attachments', { count: files.length });
    setFiles([]);
    // Clear uploaded filenames ref as well
    uploadedFilenamesRef.current = [];
    logger.info('All file attachments cleared');

    // Refresh session files after clearing
    refreshSessionFiles();
  }, [files.length, refreshSessionFiles]);

  const getFileById = useCallback(
    (id: string): AttachmentReference | undefined => {
      return files.find((file) => file.contentId === id);
    },
    [files],
  );

  const contextValue: ResourceAttachmentContextType = useMemo(
    () => ({
      files,
      addFile,
      removeFile,
      clearFiles,
      isLoading,
      getFileById,
      sessionFiles,
      isSessionFilesLoading,
      sessionFilesError,
      refreshSessionFiles,
    }),
    [
      files,
      addFile,
      removeFile,
      clearFiles,
      isLoading,
      getFileById,
      sessionFiles,
      isSessionFilesLoading,
      sessionFilesError,
      refreshSessionFiles,
    ],
  );

  return (
    <ResourceAttachmentContext.Provider value={contextValue}>
      {children}
    </ResourceAttachmentContext.Provider>
  );
};

// Custom hook to use the ResourceAttachmentContext
export const useResourceAttachment = () => {
  const context = useContext(ResourceAttachmentContext);
  if (context === undefined) {
    throw new Error(
      'useResourceAttachment must be used within a ResourceAttachmentProvider',
    );
  }
  return context;
};

export { ResourceAttachmentContext };
