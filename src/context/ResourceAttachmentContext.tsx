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
import useSWR from 'swr';
import { AttachmentReference } from '@/models/chat';
import { getLogger } from '@/lib/logger';
import { useRustMCPServer } from '@/hooks/use-rust-mcp-server';
import { useSessionContext } from './SessionContext';
import { syncFileToWorkspace } from '@/lib/workspace-sync-service';
import {
  ContentStoreServerProxy,
  PendingFileInput,
  ExtendedAttachmentReference,
  CreateStoreArgs,
  AddContentArgs,
  ListContentArgs,
  DeleteContentArgs,
} from '@/models/content-store';

const logger = getLogger('ResourceAttachmentContext');

interface ResourceAttachmentContextType {
  /**
   * All files stored in the current session's store
   */
  sessionFiles: AttachmentReference[];
  /**
   * Files being attached but not yet confirmed by server
   */
  pendingFiles: AttachmentReference[];
  /**
   * Add files to pending state for immediate UI feedback
   * @param files - Array of file objects to add to pending state
   */
  addPendingFiles: (files: PendingFileInput[]) => void;
  /**
   * Commit pending files to server and move to sessionFiles
   * @returns Promise resolving to successfully uploaded attachment references
   */
  commitPendingFiles: () => Promise<AttachmentReference[]>;
  /**
   * Remove a file from the session
   */
  removeFile: (ref: AttachmentReference) => Promise<void>;
  /**
   * Clear pending files from UI state
   */
  clearPendingFiles: () => void;
  /**
   * Loading state for operations
   */
  isLoading: boolean;
  /**
   * Refresh session files from the server using SWR mutate
   */
  mutateSessionFiles: () => Promise<void>;
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

  // Pending files state (files being attached but not yet confirmed by server)
  const [pendingFiles, setPendingFiles] = useState<
    ExtendedAttachmentReference[]
  >([]);

  const { current: currentSession, updateSession } = useSessionContext();

  // Use Rust built-in content-store server exclusively
  const { server, loading: serverLoading } =
    useRustMCPServer<ContentStoreServerProxy>('contentstore');

  // Use SWR for session files management
  const { data: sessionFiles = [], mutate } = useSWR(
    currentSession?.id ? `session-files-${currentSession.id}` : null,
    async (key: string) => {
      const sessionId = key.replace('session-files-', '');
      if (sessionId && server) {
        try {
          logger.info('Proxy: Calling server.listContent for session files', {
            sessionId,
          });
          const listContentArgs: ListContentArgs = {
            sessionId,
          };
          const result = await server.listContent(listContentArgs);
          logger.info('Proxy: server.listContent completed successfully', {
            sessionId,
            contentCount: result?.contents?.length || 0,
          });
          const files =
            result?.contents?.map((content) => ({
              sessionId: content.sessionId,
              contentId: content.contentId,
              filename: content.filename,
              mimeType: content.mimeType,
              size: Number((content as { size?: number | null }).size ?? 0),
              lineCount: content.lineCount || 0,
              preview: content.preview ?? content.filename ?? '',
              uploadedAt: content.uploadedAt || new Date().toISOString(),
              chunkCount: content.chunkCount,
              lastAccessedAt: content.lastAccessedAt,
            })) || [];

          return files;
        } catch (error) {
          logger.warn(
            'Session context not ready yet, will retry on next revalidation',
            { sessionId, error },
          );
          return [];
        }
      }
      return [];
    },
    {
      revalidateOnFocus: false,
      revalidateOnReconnect: true,
      dedupingInterval: 5000, // Dedupe requests within 5 seconds
      shouldRetryOnError: true,
      errorRetryCount: 3,
      errorRetryInterval: 1000,
    },
  );

  // Track uploaded filenames per session to prevent duplicate uploads within the same session
  const uploadedFilenamesRef = useRef<Map<string, Set<string>>>(new Map());

  // Cache the current session ID to avoid race conditions during batch uploads
  const sessionStoreIdRef = useRef<string | undefined>();

  // Reset files when session changes
  const prevSessionIdRef = useRef<string | undefined>();

  // Wrapper for SWR mutate to match interface
  const mutateSessionFiles = useCallback(async (): Promise<void> => {
    await mutate();
  }, [mutate]);

  useEffect(() => {
    if (currentSession?.id !== prevSessionIdRef.current) {
      logger.debug('Session changed, clearing cached data', {
        previousSessionId: prevSessionIdRef.current,
        currentSessionId: currentSession?.id,
      });
      // Clear uploaded filenames when session changes
      uploadedFilenamesRef.current.clear();

      // Clear pending files on session change (SWR will handle sessionFiles)
      setPendingFiles([]);

      // Update sessionId cache
      sessionStoreIdRef.current = currentSession?.id;
      prevSessionIdRef.current = currentSession?.id;
    }
  }, [currentSession?.id]);

  // Update sessionId cache when currentSession id changes
  useEffect(() => {
    sessionStoreIdRef.current = currentSession?.id;
  }, [currentSession?.id]);

  // Ensure store exists for current session
  const ensureStoreExists = useCallback(
    async (sessionId: string): Promise<string> => {
      if (!server) {
        throw new Error('Content store server is not initialized.');
      }

      try {
        // First check cached session store ID to avoid race conditions
        if (sessionStoreIdRef.current) {
          return sessionStoreIdRef.current;
        }

        // Since sessionId = storeId (1:1 relationship), use sessionId directly
        sessionStoreIdRef.current = sessionId;

        // Always try to create the content store (it should be idempotent)
        // Check if server is available
        if (!server) {
          if (serverLoading) {
            throw new Error(
              'Content store server is still loading. Please wait a moment.',
            );
          } else {
            throw new Error(
              'Content store server is not available. Please wait for server initialization.',
            );
          }
        }

        const createStoreArgs: CreateStoreArgs = {
          sessionId,
          metadata: {
            // Optional metadata can be added here
          },
        };
        const createResult = await server.createStore(createStoreArgs);

        // Content store now uses sessionId directly (1:1 relationship)
        let storeId: string;
        if (typeof createResult === 'object' && createResult !== null) {
          // Check if result has sessionId field (new format)
          if ('sessionId' in createResult) {
            storeId = (createResult as { sessionId: string }).sessionId;
            logger.info('Extracted storeId from sessionId field', { storeId });
          } else {
            logger.error(
              'Invalid createStore response: missing both id and storeId fields',
              {
                createResult,
                createResultKeys: Object.keys(createResult),
              },
            );
            throw new Error(
              'Invalid createStore response: missing storeId or id field',
            );
          }
        } else {
          logger.error('Invalid createStore response: not an object', {
            createResult,
            createResultType: typeof createResult,
          });
          throw new Error('Invalid createStore response: expected object');
        }

        // Cache the storeId immediately to prevent race conditions
        sessionStoreIdRef.current = storeId;

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
        const response = await fetch(url);

        if (!response.ok) {
          throw new Error(
            `Failed to fetch ${url}: ${response.status} ${response.statusText}`,
          );
        }

        const blob = await response.blob();
        const blobUrl = URL.createObjectURL(blob);

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

  // Add files to pending state for immediate UI feedback
  const addPendingFiles = useCallback(
    (files: PendingFileInput[]) => {
      const newPending = files.map((file) => ({
        sessionId: currentSession?.id || '',
        contentId: `pending_${Date.now()}_${Math.random().toString(36).slice(2, 11)}`,
        filename: file.filename || extractFilenameFromUrl(file.url),
        mimeType: file.mimeType,
        size: file.file?.size || 0, // Use File object size if available
        lineCount: 0,
        preview: file.filename || extractFilenameFromUrl(file.url),
        uploadedAt: new Date().toISOString(),
        chunkCount: 0,
        lastAccessedAt: new Date().toISOString(),
        // Store original data for proper upload
        originalUrl: file.url,
        originalPath: file.originalPath,
        file: file.file,
        blobCleanup: file.blobCleanup,
      }));

      setPendingFiles((prev) => [...prev, ...newPending]);
    },
    [currentSession?.id, extractFilenameFromUrl],
  );

  // Internal helper function to upload a single file to server
  const addFileInternal = useCallback(
    async (
      url: string,
      mimeType: string,
      filename?: string,
      _originalPath?: string, // Reserved for future Tauri file system access
      file?: File,
    ): Promise<AttachmentReference> => {
      const actualFilename = filename || extractFilenameFromUrl(url);

      if (!server || !currentSession?.id) {
        throw new Error('Content store server or session not available');
      }

      const storeId = await ensureStoreExists(currentSession.id);

      let fileUrl: string;
      let actualMimeType: string;
      let fileSize: number;
      let workspacePath: string | undefined;

      // If we have a File object, sync it to workspace and use file:// URL
      if (file) {
        try {
          workspacePath = await syncFileToWorkspace(file);
          fileUrl = url;
          actualMimeType = file.type || mimeType || 'application/octet-stream';
          fileSize = file.size;
        } catch (syncError) {
          logger.warn('Workspace sync failed, falling back to blob URL', {
            filename: actualFilename,
            error:
              syncError instanceof Error
                ? syncError.message
                : String(syncError),
          });
          // Fallback to blob URL if workspace sync fails
          fileUrl = URL.createObjectURL(file);
          actualMimeType = file.type || mimeType || 'application/octet-stream';
          fileSize = file.size;
        }
      } else {
        // For URLs, try to download and sync to workspace
        try {
          const response = await fetch(url);

          if (!response.ok) {
            throw new Error(
              `Failed to fetch ${url}: ${response.status} ${response.statusText}`,
            );
          }

          const blob = await response.blob();
          const downloadedFile = new File([blob], actualFilename, {
            type: blob.type || mimeType || 'application/octet-stream',
          });

          workspacePath = await syncFileToWorkspace(downloadedFile);
          fileUrl = `file://${workspacePath}`;
          actualMimeType = blob.type || mimeType || 'application/octet-stream';
          fileSize = blob.size;
        } catch (downloadError) {
          logger.warn('URL download failed, falling back to blob URL', {
            url,
            filename: actualFilename,
            error:
              downloadError instanceof Error
                ? downloadError.message
                : String(downloadError),
          });
          // Fallback to blob URL if download/sync fails
          const blobResult = await convertToBlobUrl(url);
          fileUrl = blobResult.blobUrl;
          actualMimeType =
            mimeType || blobResult.type || 'application/octet-stream';
          fileSize = blobResult.size || 0;
        }
      }

      try {
        // Call the content-store server to add content using file URL
        const addContentArgs: AddContentArgs = {
          sessionId: storeId,
          fileUrl: fileUrl,
          metadata: {
            filename: actualFilename,
            mimeType: actualMimeType,
            size: fileSize,
            uploadedAt: new Date().toISOString(),
          },
        };

        const result = await server.addContent(addContentArgs);

        // If workspace sync wasn't done earlier, try it now
        if (!workspacePath && file) {
          try {
            workspacePath = await syncFileToWorkspace(file);
          } catch (error) {
            logger.warn(
              'Workspace sync failed, continuing with content-store only',
              {
                filename: result.filename,
                error: error instanceof Error ? error.message : String(error),
              },
            );
            // Continue without workspace sync - Content-Store upload was successful
          }
        }

        // Convert AddContentOutput to AttachmentReference
        return {
          sessionId: result.sessionId,
          contentId: result.contentId,
          filename: result.filename,
          mimeType: result.mimeType,
          size: Number(
            (result as { size?: number | null }).size ?? fileSize ?? 0,
          ),
          lineCount: result.lineCount,
          preview: result.preview,
          uploadedAt:
            typeof result.uploadedAt === 'string'
              ? result.uploadedAt
              : new Date().toISOString(),
          chunkCount: result.chunkCount,
          lastAccessedAt: new Date().toISOString(),
          workspacePath, // Add the workspace path to the result
        };
      } finally {
        // Clean up blob URL if we created one (fallback case)
        if (fileUrl.startsWith('blob:')) {
          URL.revokeObjectURL(fileUrl);
        }
      }
    },
    [server, extractFilenameFromUrl, convertToBlobUrl, ensureStoreExists],
  );

  // Commit pending files to server and move to sessionFiles
  const commitPendingFiles = useCallback(async (): Promise<
    AttachmentReference[]
  > => {
    if (pendingFiles.length === 0) return [];

    // Check if server is available
    if (!server) {
      if (serverLoading) {
        throw new Error(
          'Content store server is still loading. Please wait a moment.',
        );
      } else {
        throw new Error(
          'Content store server is not available. Please wait for server initialization.',
        );
      }
    }

    setIsLoading(true);
    const results: AttachmentReference[] = [];

    try {
      for (const file of pendingFiles) {
        try {
          // Use the stored original URL and File object for proper upload
          const result = await addFileInternal(
            file.originalUrl || file.preview,
            file.mimeType,
            file.filename,
            file.originalPath,
            file.file,
          );
          results.push(result);
        } catch (error) {
          logger.error('Failed to commit pending file', {
            filename: file.filename,
            error: error instanceof Error ? error.message : String(error),
          });
          // Continue with other files even if one fails
        }
      }

      // Refresh SWR cache to get updated session files
      await mutateSessionFiles();

      // Clean up any blob URLs created for pending files
      pendingFiles.forEach((file) => {
        if (file.blobCleanup) {
          try {
            file.blobCleanup();
          } catch (error) {
            logger.warn('Failed to cleanup blob URL', {
              filename: file.filename,
              error: error instanceof Error ? error.message : String(error),
            });
          }
        }
      });

      // Clear pending files after successful commit
      setPendingFiles([]);

      return results;
    } finally {
      setIsLoading(false);
    }
  }, [pendingFiles, addFileInternal, mutateSessionFiles]);

  const removeFile = useCallback(
    async (ref: AttachmentReference): Promise<void> => {
      // Check if this is a pending file (not yet saved to server)
      if (ref.contentId.startsWith('pending_')) {
        // Handle pending file removal - remove from local state
        const fileToRemove = pendingFiles.find(
          (file) => file.contentId === ref.contentId,
        );

        if (fileToRemove) {
          // Clean up blob URL if it exists
          if (fileToRemove.blobCleanup) {
            try {
              fileToRemove.blobCleanup();
            } catch (error) {
              logger.warn('Failed to cleanup blob URL for pending file', {
                filename: fileToRemove.filename,
                error: error instanceof Error ? error.message : String(error),
              });
            }
          }

          // Remove from pending files array
          setPendingFiles((prev) =>
            prev.filter((file) => file.contentId !== ref.contentId),
          );
        } else {
          logger.warn('Pending file not found in pendingFiles array', {
            contentId: ref.contentId,
            filename: ref.filename,
          });
        }
        return;
      }

      // Handle session file removal from server
      setIsLoading(true);
      try {
        // Check if server is available
        if (!server) {
          if (serverLoading) {
            throw new Error(
              'Content store server is still loading. Please wait a moment.',
            );
          } else {
            throw new Error(
              'Content store server is not available. Please wait for server initialization.',
            );
          }
        }

        // Call server.deleteContent to actually remove the file
        const deleteArgs: DeleteContentArgs = {
          contentId: ref.contentId,
        };

        await server.deleteContent(deleteArgs);

        logger.info('Successfully deleted content from server', {
          contentId: ref.contentId,
          filename: ref.filename,
          sessionId: ref.sessionId,
        });

        // Refresh session files after removal to reflect the change
        await mutateSessionFiles();
      } catch (error) {
        logger.error('Failed to remove session file attachment', {
          ref: {
            contentId: ref.contentId,
            filename: ref.filename,
            sessionId: ref.sessionId,
          },
          error: error instanceof Error ? error.message : String(error),
        });
        throw new Error(
          `Failed to remove file: ${error instanceof Error ? error.message : String(error)}`,
        );
      } finally {
        setIsLoading(false);
      }
    },
    [mutateSessionFiles, pendingFiles],
  );

  const clearPendingFiles = useCallback(() => {
    setPendingFiles([]);
  }, [pendingFiles.length]);

  const contextValue: ResourceAttachmentContextType = useMemo(
    () => ({
      sessionFiles,
      pendingFiles,
      addPendingFiles,
      commitPendingFiles,
      removeFile,
      clearPendingFiles,
      isLoading,
      mutateSessionFiles,
    }),
    [
      sessionFiles,
      pendingFiles,
      addPendingFiles,
      commitPendingFiles,
      removeFile,
      clearPendingFiles,
      isLoading,
      mutateSessionFiles,
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
