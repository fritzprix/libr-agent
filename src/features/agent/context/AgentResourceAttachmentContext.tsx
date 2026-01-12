import React, {
  createContext,
  useContext,
  useCallback,
  useState,
  useRef,
  useEffect,
} from 'react';
import useSWR from 'swr';
import { useRustMCPServer } from '@/hooks/use-rust-mcp-server';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { getLogger } from '@/lib/logger';
import {
  syncFileToWorkspace,
  EFFECTIVE_MAX_SIZE,
  createFileSizeErrorMessage,
} from '@/lib/workspace-sync-service';
import {
  ContentStoreServerProxy,
  ListContentArgs,
  DeleteContentArgs,
} from '@/models/content-store';
import { ContentStoreItem } from '@/models/content-store';
import { AttachmentReference } from '@/models/chat';
import { saveAgentFile } from '@/features/agent/api/agent-backend';

const logger = getLogger('AgentResourceAttachmentContext');

export interface PendingFileInput {
  file?: File;
  url: string;
  filename?: string;
  mimeType: string;
  originalPath?: string;
  blobCleanup?: () => void;
}

interface AgentResourceAttachmentContextType {
  pendingFiles: AttachmentReference[];
  sessionFiles: AttachmentReference[];
  isLoading: boolean;
  addPendingFiles: (files: PendingFileInput[]) => void;
  removeFile: (file: AttachmentReference) => Promise<void>;
  commitPendingFiles: () => Promise<AttachmentReference[]>;
  clearPendingFiles: () => void;
  validateFiles: (files: File[]) => File[];
}

const AgentResourceAttachmentContext = createContext<
  AgentResourceAttachmentContextType | undefined
>(undefined);

export function useAgentResourceAttachment() {
  const context = useContext(AgentResourceAttachmentContext);
  if (!context) {
    throw new Error(
      'useAgentResourceAttachment must be used within an AgentResourceAttachmentProvider',
    );
  }
  return context;
}

export function AgentResourceAttachmentProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  const { server, loading: serverLoading } =
    useRustMCPServer<ContentStoreServerProxy>('contentstore');

  // Use Agent V2 Session State
  const { session: currentSession } = useAgentSessionState();

  const [pendingFiles, setPendingFiles] = useState<AttachmentReference[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  // Use SWR to fetch session files
  const { data: sessionFiles = [], mutate: mutateSessionFiles } = useSWR(
    currentSession?.id && server
      ? ['agent_content_list', currentSession.id]
      : null,
    async () => {
      logger.info('[AgentResourceAttachmentContext] SWR fetcher called', {
        hasServer: !!server,
        currentSessionId: currentSession?.id,
        serverLoading,
      });

      if (server && currentSession?.id) {
        const sessionId = currentSession.id;
        try {
          // Check if store exists by trying to list content
          // If it fails with "store not found", we return empty list
          // We don't auto-create store here to avoid side effects in read-only operations
          const listContentArgs: ListContentArgs = {
            sessionId,
          };
          logger.info('[AgentResourceAttachmentContext] Calling listContent', {
            sessionId,
          });
          const result = await server.listContent(listContentArgs);
          logger.info('Proxy: server.listContent completed successfully', {
            sessionId,
            contentCount: result?.contents?.length || 0,
            contents: result?.contents,
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

          logger.info('[AgentResourceAttachmentContext] Mapped files result', {
            filesCount: files.length,
            files,
          });
          return files;
        } catch (error) {
          logger.warn(
            'Session context not ready yet or store missing, will retry on next revalidation',
            { sessionId, error },
          );
          return [];
        }
      }
      logger.warn(
        '[AgentResourceAttachmentContext] SWR fetcher: No server or session',
        {
          hasServer: !!server,
          sessionId: currentSession?.id,
        },
      );
      return [];
    },
    {
      revalidateOnFocus: false,
      revalidateOnReconnect: true,
      dedupingInterval: 5000,
      shouldRetryOnError: true,
      errorRetryCount: 3,
      errorRetryInterval: 1000,
    },
  );

  // Track session ID for caching store IDs
  // NOTE: Content stores are auto-created on first use (saveKnowledge/listContent)
  // No explicit createStore tool is needed
  const sessionStoreIdRef = useRef<string | undefined>();
  // Reset files when session changes
  const prevSessionIdRef = useRef<string | undefined>();
  const uploadedFilenamesRef = useRef<Set<string>>(new Set());

  useEffect(() => {
    if (currentSession?.id !== prevSessionIdRef.current) {
      logger.debug('Session changed, clearing cached data', {
        previousSessionId: prevSessionIdRef.current,
        currentSessionId: currentSession?.id,
      });
      // Clear uploaded filenames when session changes
      uploadedFilenamesRef.current.clear();
      // Clear pending files on session change
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

  const extractFilenameFromUrl = useCallback((url: string): string => {
    try {
      const urlObj = new URL(url);
      const pathname = urlObj.pathname;
      return pathname.split('/').pop() || 'unknown_file';
    } catch {
      return `file_${Date.now()}`;
    }
  }, []);

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
        if (url.startsWith('blob:')) {
          return {
            blobUrl: url,
            cleanup: () => {},
            size: 0,
            type: '',
          };
        }

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
        logger.error('Failed to convert URL to blob', { url, error });
        throw new Error(
          `Failed to process URL "${url}": ${error instanceof Error ? error.message : String(error)}`,
        );
      }
    },
    [],
  );

  const addPendingFiles = useCallback(
    (files: PendingFileInput[]) => {
      const newPending = files.map((file) => ({
        sessionId: currentSession?.id || '',
        contentId: `pending_${Date.now()}_${Math.random().toString(36).slice(2, 11)}`,
        filename: file.filename || extractFilenameFromUrl(file.url),
        mimeType: file.mimeType,
        size: file.file?.size || 0,
        lineCount: 0,
        preview: file.filename || extractFilenameFromUrl(file.url),
        uploadedAt: new Date().toISOString(),
        chunkCount: 0,
        lastAccessedAt: new Date().toISOString(),
        originalUrl: file.url,
        originalPath: file.originalPath,
        file: file.file,
        blobCleanup: file.blobCleanup,
      }));

      setPendingFiles((prev) => [...prev, ...newPending]);
    },
    [currentSession?.id, extractFilenameFromUrl],
  );

  const addFileInternal = useCallback(
    async (
      url: string,
      mimeType: string,
      filename?: string,
      _originalPath?: string,
      file?: File,
    ): Promise<AttachmentReference> => {
      const actualFilename = filename || extractFilenameFromUrl(url);

      if (!server || !currentSession?.id) {
        throw new Error('Content store server or session not available');
      }

      // Store will be auto-created on first saveKnowledge call
      const storeId = currentSession.id;

      let fileUrl: string;
      let actualMimeType: string;
      let fileSize: number;
      let workspacePath: string | undefined;

      if (file) {
        try {
          workspacePath = await syncFileToWorkspace(file, currentSession.id);
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
          fileUrl = URL.createObjectURL(file);
          actualMimeType = file.type || mimeType || 'application/octet-stream';
          fileSize = file.size;
        }
      } else {
        try {
          const response = await fetch(url);
          if (!response.ok) throw new Error(`Failed to fetch ${url}`);
          const blob = await response.blob();
          const downloadedFile = new File([blob], actualFilename, {
            type: blob.type || mimeType || 'application/octet-stream',
          });
          workspacePath = await syncFileToWorkspace(
            downloadedFile,
            currentSession.id,
          );
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
          const blobResult = await convertToBlobUrl(url);
          fileUrl = blobResult.blobUrl;
          actualMimeType =
            mimeType || blobResult.type || 'application/octet-stream';
          fileSize = blobResult.size || 0;
        }
      }

      try {
        // Use session-specific saveAgentFile instead of global server.saveKnowledge
        // This ensures the file is associated with the correct agent session
        const result = (await saveAgentFile(storeId, actualFilename, {
          content: undefined, // Content is handled via fileUrl or direct upload
          fileUrl: fileUrl,
          metadata: {
            mimeType: actualMimeType,
            size: fileSize,
            uploadedAt: new Date().toISOString(),
            filename: actualFilename,
          },
        })) as ContentStoreItem;

        if (!workspacePath && file) {
          try {
            workspacePath = await syncFileToWorkspace(file, currentSession.id);
          } catch (error) {
            logger.warn(
              'Workspace sync failed (retry), continuing with content-store only',
              { error },
            );
          }
        }

        // Debug logging to verify backend response
        console.log(
          '[AgentResourceAttachmentContext] saveAgentFile result:',
          result,
        );

        return {
          sessionId: result.sessionId,
          contentId: result.contentId,
          filename: result.filename ?? actualFilename, // Fallback to computed filename
          mimeType: result.mimeType,
          size: Number(result.size ?? fileSize ?? 0),
          lineCount: result.lineCount, // Correct: Rust returns 'lineCount'
          preview: result.preview,
          uploadedAt: result.uploadedAt ?? new Date().toISOString(),
          chunkCount: result.chunkCount,
          lastAccessedAt: new Date().toISOString(),
          workspacePath,
        };
      } finally {
        if (fileUrl.startsWith('blob:')) {
          URL.revokeObjectURL(fileUrl);
        }
      }
    },
    [server, currentSession, extractFilenameFromUrl, convertToBlobUrl],
  );

  const commitPendingFiles = useCallback(async (): Promise<
    AttachmentReference[]
  > => {
    if (pendingFiles.length === 0) return [];
    if (!server) throw new Error('Content store server not available');

    setIsLoading(true);
    const results: AttachmentReference[] = [];

    try {
      for (const file of pendingFiles) {
        try {
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
        }
      }

      await mutateSessionFiles();

      pendingFiles.forEach((file) => {
        if (file.blobCleanup) {
          try {
            file.blobCleanup();
          } catch (e) {
            logger.warn('Blob cleanup failed', e);
          }
        }
      });

      setPendingFiles([]);
      return results;
    } finally {
      setIsLoading(false);
    }
  }, [pendingFiles, addFileInternal, mutateSessionFiles, server]);

  const removeFile = useCallback(
    async (ref: AttachmentReference): Promise<void> => {
      if (ref.contentId.startsWith('pending_')) {
        const fileToRemove = pendingFiles.find(
          (file) => file.contentId === ref.contentId,
        );
        if (fileToRemove?.blobCleanup) fileToRemove.blobCleanup();
        setPendingFiles((prev) =>
          prev.filter((p) => p.contentId !== ref.contentId),
        );
        return;
      }

      if (!server) return;

      try {
        const deleteArgs: DeleteContentArgs = { contentId: ref.contentId };
        await server.deleteContent(deleteArgs);
        await mutateSessionFiles();
      } catch (error) {
        logger.error('Failed to remove file from server', {
          contentId: ref.contentId,
          error,
        });
        throw error;
      }
    },
    [pendingFiles, server, mutateSessionFiles],
  );

  const clearPendingFiles = useCallback(() => {
    pendingFiles.forEach((file) => {
      if (file.blobCleanup)
        try {
          file.blobCleanup();
        } catch (e) {
          logger.warn('Blob cleanup failed', e);
        }
    });
    setPendingFiles([]);
  }, [pendingFiles]);

  const validateFiles = useCallback((files: File[]): File[] => {
    return files.filter((file) => {
      if (file.size > EFFECTIVE_MAX_SIZE) {
        // In a real app we might toast here, but for now we just filter
        const msg = createFileSizeErrorMessage(file.name, file.size);
        logger.warn(msg);
        return false;
      }
      return true;
    });
  }, []);

  return (
    <AgentResourceAttachmentContext.Provider
      value={{
        pendingFiles,
        sessionFiles,
        isLoading,
        addPendingFiles,
        removeFile,
        commitPendingFiles,
        clearPendingFiles,
        validateFiles,
      }}
    >
      {children}
    </AgentResourceAttachmentContext.Provider>
  );
}
