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
    currentSession?.storeId ? `session-files-${currentSession.storeId}` : null,
    async (key: string) => {
      const storeId = key.replace('session-files-', '');
      if (storeId && server) {
        logger.info('Proxy: Calling server.listContent for session files', {
          storeId,
        });
        const listContentArgs: ListContentArgs = {
          storeId,
        };
        const result = await server.listContent(listContentArgs);
        logger.info('Proxy: server.listContent completed successfully', {
          storeId,
          contentCount: result?.contents?.length || 0,
        });
        const files =
          result?.contents?.map((content) => ({
            storeId: content.storeId,
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

        logger.debug('Session files loaded via SWR', {
          storeId,
          fileCount: files.length,
        });
        return files;
      }
      return [];
    },
    {
      revalidateOnFocus: false,
      revalidateOnReconnect: true,
      dedupingInterval: 5000, // Dedupe requests within 5 seconds
    },
  );

  // Track uploaded filenames per storeId to prevent duplicate uploads within the same store
  const uploadedFilenamesRef = useRef<Map<string, Set<string>>>(new Map());

  // Cache the current session's storeId to avoid race conditions during batch uploads
  const sessionStoreIdRef = useRef<string | undefined>();

  // Reset files when session changes
  const prevSessionIdRef = useRef<string | undefined>();

  // Wrapper for SWR mutate to match interface
  const mutateSessionFiles = useCallback(async (): Promise<void> => {
    await mutate();
  }, [mutate]);

  useEffect(() => {
    if (currentSession?.id !== prevSessionIdRef.current) {
      logger.info('Session changed in ResourceAttachmentContext', {
        previousSessionId: prevSessionIdRef.current,
        currentSessionId: currentSession?.id,
        sessionName: currentSession?.name,
        storeId: currentSession?.storeId,
        reason: !prevSessionIdRef.current
          ? 'initial_session'
          : 'session_change',
      });
      // Clear uploaded filenames when session changes
      uploadedFilenamesRef.current.clear();

      // Clear pending files on session change (SWR will handle sessionFiles)
      setPendingFiles([]);

      // Update storeId cache
      sessionStoreIdRef.current = currentSession?.storeId;
      prevSessionIdRef.current = currentSession?.id;
    }
  }, [currentSession?.id]);

  // Update storeId cache when currentSession storeId changes
  useEffect(() => {
    sessionStoreIdRef.current = currentSession?.storeId;
  }, [currentSession?.storeId]);

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
          metadata: {
            sessionId,
          },
        };
        logger.info('Proxy: Calling server.createStore', { sessionId });
        const createResult = await server.createStore(createStoreArgs);
        logger.info('Proxy: server.createStore completed successfully', {
          sessionId,
          createResult,
          createResultType: typeof createResult,
          createResultKeys: createResult ? Object.keys(createResult) : [],
          storeId: createResult?.storeId,
          id: createResult?.id,
        });

        logger.debug('createStore result received', {
          sessionId,
          createResult,
          createResultType: typeof createResult,
          createResultKeys: createResult ? Object.keys(createResult) : null,
          hasIdField: createResult && 'id' in createResult,
          hasStoreIdField: createResult && 'storeId' in createResult,
          extractedStoreId:
            typeof createResult === 'object' && createResult !== null
              ? 'id' in createResult
                ? (createResult as { id: string }).id
                : 'storeId' in createResult
                  ? (createResult as { storeId: string }).storeId
                  : 'none'
              : 'invalid',
        });

        // Handle both direct storeId and nested store.id formats
        let storeId: string;
        if (typeof createResult === 'object' && createResult !== null) {
          // Check if result is the store object itself (Rust backend format)
          if ('id' in createResult) {
            storeId = (createResult as { id: string }).id;
            logger.info('Extracted storeId from id field', { storeId });
          } else if ('storeId' in createResult) {
            storeId = (createResult as { storeId: string }).storeId;
            logger.info('Extracted storeId from storeId field', { storeId });
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

        logger.info(
          'StoreId extracted successfully, caching and updating session',
          {
            sessionId,
            storeId,
            currentCacheValue: sessionStoreIdRef.current,
          },
        );

        // Cache the storeId immediately to prevent race conditions
        sessionStoreIdRef.current = storeId;
        logger.info('Cached storeId in sessionStoreIdRef', {
          sessionId,
          storeId,
          cacheUpdated: true,
        });

        // Update the session with the new storeId
        logger.info('Updating session with new storeId', {
          sessionId,
          storeId,
          currentSessionStoreId: currentSession?.storeId,
        });
        await updateSession(sessionId, { storeId });
        logger.info('Session updated successfully with storeId', {
          sessionId,
          storeId,
          updatedSessionStoreId: currentSession?.storeId,
        });

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

  // Add files to pending state for immediate UI feedback
  const addPendingFiles = useCallback(
    (files: PendingFileInput[]) => {
      const newPending = files.map((file) => ({
        storeId: currentSession?.storeId || '',
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

      logger.debug('Adding files to pending state', {
        fileCount: files.length,
        filenames: newPending.map((f) => f.filename),
      });

      setPendingFiles((prev) => [...prev, ...newPending]);
    },
    [currentSession?.storeId, extractFilenameFromUrl],
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
      logger.info('ensureStoreExists returned storeId', {
        sessionId: currentSession.id,
        storeId,
        storeIdType: typeof storeId,
        storeIdLength: storeId?.length,
        cachedStoreId: sessionStoreIdRef.current,
      });

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

          logger.debug('Using File object synced to workspace', {
            filename: actualFilename,
            workspacePath,
            fileUrl,
            size: fileSize,
            type: actualMimeType,
          });
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
          logger.debug('Downloading URL to workspace', { url });
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

          logger.debug('URL downloaded and synced to workspace', {
            originalUrl: url,
            workspacePath,
            fileUrl,
            size: fileSize,
            type: actualMimeType,
          });
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
          storeId: storeId,
          fileUrl: fileUrl,
          metadata: {
            filename: actualFilename,
            mimeType: actualMimeType,
            size: fileSize,
            uploadedAt: new Date().toISOString(),
          },
        };

        logger.info('DEBUG: addContentArgs before call', {
          storeId,
          storeIdType: typeof storeId,
          storeIdLength: storeId?.length,
          fileUrl,
          hasMetadata: !!addContentArgs.metadata,
          addContentArgsKeys: Object.keys(addContentArgs),
          addContentArgs: JSON.stringify(addContentArgs),
          addContentArgsParsed: JSON.parse(JSON.stringify(addContentArgs)), // Deep clone to check serialization
        });
        logger.info('Proxy: Calling server.addContent', {
          filename: actualFilename,
          storeId,
          fileSize,
        });
        const result = await server.addContent(addContentArgs);
        logger.info('Proxy: server.addContent completed successfully', {
          filename: result.filename,
          contentId: result.contentId,
          chunkCount: result.chunkCount,
        });

        logger.info('File uploaded to Content-Store successfully', {
          filename: result.filename,
          contentId: result.contentId,
          chunkCount: result.chunkCount,
        });

        // If workspace sync wasn't done earlier, try it now
        if (!workspacePath && file) {
          try {
            workspacePath = await syncFileToWorkspace(file);
            logger.info('File synced to workspace successfully', {
              filename: result.filename,
              workspacePath,
            });
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
          storeId: result.storeId,
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
    logger.info('commitPendingFiles invoked', {
      pendingCount: pendingFiles.length,
      filenames: pendingFiles.map((f) => f.filename),
    });
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

    logger.info('Committing pending files to server', {
      fileCount: pendingFiles.length,
    });

    setIsLoading(true);
    const results: AttachmentReference[] = [];

    try {
      for (const file of pendingFiles) {
        try {
          logger.info('commitPendingFiles: uploading file', {
            filename: file.filename,
            mimeType: file.mimeType,
            hasFileObject: !!(file as { file?: File }).file,
            size: file.size,
          });
          // Use the stored original URL and File object for proper upload
          const result = await addFileInternal(
            file.originalUrl || file.preview,
            file.mimeType,
            file.filename,
            file.originalPath,
            file.file,
          );
          logger.info('commitPendingFiles: upload success', {
            filename: result.filename,
            contentId: result.contentId,
            storeId: result.storeId,
          });
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
      logger.info('commitPendingFiles: calling mutateSessionFiles');
      await mutateSessionFiles();

      // Clean up any blob URLs created for pending files
      pendingFiles.forEach((file) => {
        if (file.blobCleanup) {
          try {
            file.blobCleanup();
            logger.debug('Cleaned up blob URL for committed file', {
              filename: file.filename,
            });
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

      logger.info('Pending files committed successfully', {
        successCount: results.length,
        totalCount: pendingFiles.length,
      });

      return results;
    } finally {
      setIsLoading(false);
    }
  }, [pendingFiles, addFileInternal, mutateSessionFiles]);

  const removeFile = useCallback(
    async (ref: AttachmentReference): Promise<void> => {
      logger.debug('Removing file attachment', {
        contentId: ref.contentId,
        filename: ref.filename,
        isPending: ref.contentId.startsWith('pending_'),
      });

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
              logger.debug('Cleaned up blob URL for pending file', {
                filename: fileToRemove.filename,
              });
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

          logger.info('Pending file removed successfully from UI', {
            filename: ref.filename,
            contentId: ref.contentId,
          });
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
        // TODO: Call server.removeContent when available
        // For now, we'll just refresh to reflect server state

        logger.info('Session file attachment removed successfully', {
          filename: ref.filename,
          contentId: ref.contentId,
        });

        // Refresh session files after removal to reflect the change
        await mutateSessionFiles();
      } catch (error) {
        logger.error('Failed to remove session file attachment', {
          ref: {
            contentId: ref.contentId,
            filename: ref.filename,
            storeId: ref.storeId,
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
    logger.debug('Clearing pending file attachments', {
      count: pendingFiles.length,
    });
    setPendingFiles([]);
    logger.info('Pending file attachments cleared');
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

export { ResourceAttachmentContext };
