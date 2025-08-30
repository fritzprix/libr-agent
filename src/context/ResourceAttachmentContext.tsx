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
  addFilesBatch: (
    files: { url: string; mimeType: string; filename?: string }[],
  ) => Promise<AttachmentReference[]>;
  removeFile: (ref: AttachmentReference) => Promise<void>;
  clearFiles: () => void;
  clearPendingFiles: () => void;
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
   * Files being attached but not yet confirmed by server
   */
  pendingFiles: AttachmentReference[];
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
  const [isLoading, setIsLoading] = useState(false);

  // Flag to prevent automatic refresh during file upload operations
  const [isFileOperationInProgress, setIsFileOperationInProgress] =
    useState(false);

  // Session files management states
  const [sessionFiles, setSessionFiles] = useState<AttachmentReference[]>([]);
  const [isSessionFilesLoading, setIsSessionFilesLoading] = useState(false);
  const [sessionFilesError, setSessionFilesError] = useState<string | null>(
    null,
  );

  // Pending files state (files being attached but not yet confirmed by server)
  const [pendingFiles, setPendingFiles] = useState<AttachmentReference[]>([]);

  const { current: currentSession, updateSession } = useSessionContext();
  const { server } = useWebMCPServer<ContentStoreServer>('content-store');

  // Track uploaded filenames per storeId to prevent duplicate uploads within the same store
  const uploadedFilenamesRef = useRef<Map<string, Set<string>>>(new Map());

  // Cache the current session's storeId to avoid race conditions during batch uploads
  const sessionStoreIdRef = useRef<string | undefined>();

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
        storeId: currentSession?.storeId,
        reason: !prevSessionIdRef.current
          ? 'initial_session'
          : 'session_change',
      });
      // Clear uploaded filenames when session changes
      uploadedFilenamesRef.current.clear();

      // Clear and refresh session files
      setSessionFiles([]);
      setPendingFiles([]); // Also clear pending files on session change
      setSessionFilesError(null);

      // Update storeId cache
      sessionStoreIdRef.current = currentSession?.storeId;
      prevSessionIdRef.current = currentSession?.id;
    }
  }, [currentSession?.id]);

  // Update storeId cache when currentSession storeId changes
  useEffect(() => {
    sessionStoreIdRef.current = currentSession?.storeId;
  }, [currentSession?.storeId]);

  // Auto-refresh session files when storeId is available
  // But skip if a file operation is in progress to avoid race conditions
  useEffect(() => {
    if (currentSession?.storeId && !isFileOperationInProgress) {
      logger.debug('Auto-refreshing session files due to storeId change', {
        storeId: currentSession.storeId,
        isFileOperationInProgress,
      });
      refreshSessionFiles();
    } else if (currentSession?.storeId && isFileOperationInProgress) {
      logger.debug('Skipping auto-refresh during file operation', {
        storeId: currentSession.storeId,
        isFileOperationInProgress,
      });
    }
  }, [currentSession?.storeId, refreshSessionFiles, isFileOperationInProgress]);

  // Ensure store exists for current session
  const ensureStoreExists = useCallback(
    async (sessionId: string): Promise<string> => {
      if (!server) {
        throw new Error('Content store server is not initialized.');
      }

      try {
        // First check cached storeId to avoid race conditions
        if (sessionStoreIdRef.current) {
          logger.debug('Using cached store ID', {
            sessionId,
            storeId: sessionStoreIdRef.current,
          });
          return sessionStoreIdRef.current;
        }

        // Check if session already has a storeId
        if (currentSession?.storeId) {
          logger.debug('Using existing store ID from session', {
            sessionId,
            storeId: currentSession.storeId,
          });
          sessionStoreIdRef.current = currentSession.storeId;
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

        // Cache the storeId immediately to prevent race conditions
        sessionStoreIdRef.current = storeId;

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

      // We'll check for store-based duplicates after ensuring store exists
      // This allows the same filename in different stores while preventing race conditions within the same store

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
      setIsFileOperationInProgress(true);
      let blobCleanup: (() => void) | null = null;

      try {
        // 1. Ensure store exists for this session
        const storeId = await ensureStoreExists(currentSession.id);

        // Check if file is already being uploaded in this store to prevent race conditions
        const storeUploads =
          uploadedFilenamesRef.current.get(storeId) || new Set();
        if (storeUploads.has(actualFilename)) {
          logger.warn('File upload already in progress for this store', {
            filename: actualFilename,
            storeId,
          });
          const existingFile = sessionFiles.find(
            (f) => f.filename === actualFilename && f.storeId === storeId,
          );
          if (existingFile) {
            return existingFile;
          }
          throw new Error(
            `File "${actualFilename}" is already being uploaded to this session. Please wait for the current upload to complete before trying again.`,
          );
        }

        // Add filename to uploaded list for this store to prevent duplicates
        if (!uploadedFilenamesRef.current.has(storeId)) {
          uploadedFilenamesRef.current.set(storeId, new Set());
        }
        uploadedFilenamesRef.current.get(storeId)!.add(actualFilename);

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

        // Check if file already exists in current session or pending files (by filename)
        const existingFile = sessionFiles.find(
          (file) => file.filename === actualFilename,
        );
        const existingPendingFile = pendingFiles.find(
          (file) => file.filename === actualFilename,
        );

        if (existingFile) {
          logger.warn('File already exists in current session', {
            filename: actualFilename,
          });
          return existingFile;
        }

        if (existingPendingFile) {
          logger.warn('File already being uploaded', {
            filename: actualFilename,
          });
          return existingPendingFile;
        }

        // Create a temporary attachment reference to show in UI immediately
        const tempAttachment: AttachmentReference = {
          storeId: storeId,
          contentId: `temp_${Date.now()}_${Math.random().toString(36).slice(2, 11)}`,
          filename: actualFilename,
          mimeType: actualMimeType,
          size: blobResult.size || 0,
          lineCount: 0,
          preview: actualFilename,
          uploadedAt: new Date().toISOString(),
          chunkCount: 0,
          lastAccessedAt: new Date().toISOString(),
        };

        // Add to pending files immediately for UI responsiveness
        setPendingFiles((prev) => [...prev, tempAttachment]);

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

        logger.info('File attachment added successfully', {
          filename: result.filename,
          contentId: result.contentId,
          chunkCount: result.chunkCount,
          originalUrl: url,
          wasConverted: !url.startsWith('blob:'),
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
          uploadedAt:
            result.uploadedAt instanceof Date
              ? result.uploadedAt.toISOString()
              : result.uploadedAt,
          chunkCount: result.chunkCount,
          lastAccessedAt: new Date().toISOString(),
        };

        // Update the temporary attachment in pendingFiles with real server data
        // Keep it in pendingFiles until user submits message (UI completion)
        setPendingFiles((prev) =>
          prev.map((file) =>
            file.filename === actualFilename ? attachment : file,
          ),
        );

        // Also add to session files for server state tracking
        setSessionFiles((prev) => {
          // Check if file already exists to avoid duplicates
          const existingIndex = prev.findIndex(
            (f) => f.filename === attachment.filename,
          );
          if (existingIndex >= 0) {
            // Replace existing file
            const updated = [...prev];
            updated[existingIndex] = attachment;
            return updated;
          } else {
            // Add new file
            return [...prev, attachment];
          }
        });

        return attachment;
      } catch (error) {
        // Remove from pending files on error
        setPendingFiles((prev) =>
          prev.filter((file) => file.filename !== actualFilename),
        );

        // Remove filename from uploaded list on error (store-specific)
        try {
          // 1. Ensure store exists for this session (in case it was created during the failed attempt)
          const storeId = await ensureStoreExists(currentSession.id);
          const storeUploads = uploadedFilenamesRef.current.get(storeId);
          if (storeUploads) {
            storeUploads.delete(actualFilename);
          }
        } catch (storeError) {
          logger.warn('Failed to clean up upload tracking on error', {
            filename: actualFilename,
            storeError,
          });
        }

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
            filesInCurrentSession: sessionFiles.length,
          });

          // Check if we already have this file in our current session's state
          const existingFile = sessionFiles.find(
            (f) => f.filename === actualFilename,
          );
          if (existingFile) {
            logger.info('Returning existing file from current session', {
              contentId: existingFile.contentId,
              filename: existingFile.filename,
              sessionId: currentSession?.id,
            });
            return existingFile;
          }

          // File exists in another session but not in current session - inform user
          throw new Error(
            `This file content has already been uploaded in the current session. The same content can only be uploaded once within the same session. File: "${actualFilename}"`,
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
        setIsFileOperationInProgress(false);
      }
    },
    [
      sessionFiles,
      extractFilenameFromUrl,
      convertToBlobUrl,
      server,
      currentSession,
      ensureStoreExists,
      refreshSessionFiles,
    ],
  );

  // Batch file adding to avoid race conditions
  const addFilesBatch = useCallback(
    async (
      files: { url: string; mimeType: string; filename?: string }[],
    ): Promise<AttachmentReference[]> => {
      if (files.length === 0) return [];

      logger.info('Starting batch file upload', { count: files.length });
      setIsFileOperationInProgress(true);

      try {
        // Add all files individually (they'll be added to pending state)
        const results: AttachmentReference[] = [];

        for (const file of files) {
          try {
            const result = await addFile(
              file.url,
              file.mimeType,
              file.filename,
            );
            results.push(result);
          } catch (error) {
            logger.error('Failed to add file in batch', {
              filename: file.filename,
              error: error instanceof Error ? error.message : String(error),
            });
            // Continue with other files even if one fails
          }
        }

        // After all files are processed, do a single refresh to sync state
        if (results.length > 0) {
          logger.info('Batch upload completed, refreshing session files', {
            successful: results.length,
            total: files.length,
          });
          await refreshSessionFiles();
        }

        return results;
      } finally {
        setIsFileOperationInProgress(false);
      }
    },
    [addFile, refreshSessionFiles],
  );

  const removeFile = useCallback(
    async (ref: AttachmentReference): Promise<void> => {
      setIsFileOperationInProgress(true);
      try {
        logger.debug('Removing file attachment', {
          contentId: ref.contentId,
          filename: ref.filename,
        });

        // Remove from uploaded filenames ref for the specific store
        const storeUploads = uploadedFilenamesRef.current.get(ref.storeId);
        if (storeUploads) {
          storeUploads.delete(ref.filename);
        }

        logger.info('File attachment removed successfully', {
          filename: ref.filename,
          contentId: ref.contentId,
        });

        // Refresh session files after removal to reflect the change
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
      } finally {
        setIsFileOperationInProgress(false);
      }
    },
    [refreshSessionFiles],
  );

  const clearFiles = useCallback(() => {
    logger.debug('Clearing all file attachments', {
      sessionCount: sessionFiles.length,
      pendingCount: pendingFiles.length,
    });
    // Clear both pending and session files from UI state
    setPendingFiles([]);
    setSessionFiles([]);
    // Clear uploaded filenames ref as well
    uploadedFilenamesRef.current.clear();
    logger.info('All file attachments cleared');
  }, [sessionFiles.length, pendingFiles.length]);

  const clearPendingFiles = useCallback(() => {
    logger.debug('Clearing pending file attachments', {
      count: pendingFiles.length,
    });
    setPendingFiles([]);
    logger.info('Pending file attachments cleared');
  }, [pendingFiles.length]);

  const getFileById = useCallback(
    (id: string): AttachmentReference | undefined => {
      return sessionFiles.find((file) => file.contentId === id);
    },
    [sessionFiles],
  );

  const contextValue: ResourceAttachmentContextType = useMemo(
    () => ({
      files: sessionFiles, // Use sessionFiles as the single source of truth
      addFile,
      addFilesBatch,
      removeFile,
      clearFiles,
      clearPendingFiles,
      isLoading,
      getFileById,
      sessionFiles,
      pendingFiles,
      isSessionFilesLoading,
      sessionFilesError,
      refreshSessionFiles,
    }),
    [
      sessionFiles,
      addFile,
      addFilesBatch,
      removeFile,
      clearFiles,
      clearPendingFiles,
      isLoading,
      getFileById,
      pendingFiles,
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
