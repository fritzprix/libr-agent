import React, {
  createContext,
  useContext,
  useCallback,
  useState,
  useRef,
  useEffect,
} from 'react';
import useSWR from 'swr';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { getLogger } from '@/lib/logger';
import {
  syncFileToWorkspace,
  EFFECTIVE_MAX_SIZE,
  createFileSizeErrorMessage,
} from '@/lib/workspace-sync-service';
import type { ContentStoreItem } from '@/models/content-store';
import { AttachmentReference } from '@/models/chat';
import {
  saveAgentFile,
  agentCallBuiltinTool,
} from '@/features/agent/api/agent-backend';

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
  refetchSessionFiles: () => Promise<void>;
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
  // Use Agent V2 Session State
  const { session: currentSession } = useAgentSessionState();

  const [pendingFiles, setPendingFiles] = useState<AttachmentReference[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  // Use SWR to fetch session files via Agent V2 session-specific proxy
  const { data: sessionFiles = [], mutate: mutateSessionFiles } = useSWR(
    currentSession?.id ? ['agent_content_list', currentSession.id] : null,
    async () => {
      if (!currentSession?.id) {
        logger.warn('[AgentResourceAttachmentContext] No session ID available');
        return [];
      }

      const sessionId = currentSession.id;
      logger.info(
        '[AgentResourceAttachmentContext] Fetching files via Agent V2 proxy',
        { sessionId },
      );

      try {
        // Use Agent V2 session-specific proxy to call listContent
        const response = await agentCallBuiltinTool<{
          contents: ContentStoreItem[];
        }>(sessionId, 'builtin_content_store__listContent', {
          sessionId,
        });

        logger.info(
          '[AgentResourceAttachmentContext] Agent V2 proxy response',
          {
            sessionId,
            hasStructuredContent: !!response.structuredContent,
            responseType: typeof response,
          },
        );

        // Extract contents from structuredContent or fallback
        let contents: ContentStoreItem[] = [];
        if (
          response.structuredContent &&
          typeof response.structuredContent === 'object' &&
          'contents' in response.structuredContent
        ) {
          contents =
            (response.structuredContent as { contents: ContentStoreItem[] })
              .contents || [];
        }

        logger.info('[AgentResourceAttachmentContext] Parsed contents', {
          sessionId,
          contentCount: contents.length,
          filenames: contents.map((c) => c.filename),
        });

        // Map to AttachmentReference format with explicit committed status
        const files: AttachmentReference[] = contents.map((content) => ({
          sessionId: content.sessionId,
          contentId: content.contentId,
          status: 'committed',
          filename: content.filename,
          mimeType: content.mimeType,
          size: Number((content as { size?: number | null }).size ?? 0),
          lineCount: content.lineCount || 0,
          preview: content.preview ?? content.filename ?? '',
          uploadedAt: content.uploadedAt || new Date().toISOString(),
          chunkCount: content.chunkCount,
          lastAccessedAt: content.lastAccessedAt,
        }));

        logger.info('[AgentResourceAttachmentContext] Mapped files result', {
          filesCount: files.length,
          files,
        });

        return files;
      } catch (error) {
        logger.warn(
          'Agent V2 Content Store listing failed, will retry on next revalidation',
          { sessionId, error },
        );
        return [];
      }
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
  // NOTE: Content stores are auto-created on first use (addContent/listContent)
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
        pendingId: `pending_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
        status: 'pending' as const,
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

      if (!currentSession?.id) {
        throw new Error('Session not available');
      }

      // Store will be auto-created on first addContent call
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

      const SUPPORTED_EXTENSIONS = /\.(txt|md|json|pdf|docx|xlsx)$/i;
      const isSupported = SUPPORTED_EXTENSIONS.test(actualFilename);

      if (!isSupported) {
        logger.info(
          'File type not supported by ContentStore, saving to workspace only',
          { filename: actualFilename },
        );

        // Return AttachmentReference without contentId for workspace-only files
        return {
          sessionId: currentSession.id,
          status: 'workspace-only',
          filename: actualFilename,
          mimeType: actualMimeType,
          size: fileSize,
          lineCount: 0,
          preview: actualFilename,
          uploadedAt: new Date().toISOString(),
          workspacePath,
        };
      }

      try {
        // Use session-specific saveAgentFile instead of global server.addContent
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
          status: 'committed',
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
    [currentSession, extractFilenameFromUrl, convertToBlobUrl],
  );

  const commitPendingFiles = useCallback(async (): Promise<
    AttachmentReference[]
  > => {
    if (pendingFiles.length === 0) return [];

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

      // Optimistic update: immediately update the local state with new files
      await mutateSessionFiles(
        (currentFiles) => [...(currentFiles || []), ...results],
        { revalidate: true },
      );

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
  }, [pendingFiles, addFileInternal, mutateSessionFiles]);

  const removeFile = useCallback(
    async (ref: AttachmentReference): Promise<void> => {
      // Handle pending files - check by pendingId
      if (ref.status === 'pending') {
        const fileToRemove = pendingFiles.find(
          (file) => file.pendingId === ref.pendingId,
        );
        if (fileToRemove?.blobCleanup) fileToRemove.blobCleanup();
        setPendingFiles((prev) =>
          prev.filter((p) => p.pendingId !== ref.pendingId),
        );
        return;
      }

      // Handle workspace-only files - no server deletion needed
      if (ref.status === 'workspace-only') {
        logger.info('Workspace-only file removal (no server deletion)', {
          filename: ref.filename,
        });
        // Note: Actual workspace file deletion would require additional implementation
        return;
      }

      // Handle committed files - delete from server
      if (ref.status === 'committed') {
        if (!currentSession?.id) return;
        if (!ref.contentId) {
          logger.error('Cannot delete committed file without contentId', {
            filename: ref.filename,
          });
          return;
        }

        try {
          await agentCallBuiltinTool(
            currentSession.id,
            'builtin_content_store__deleteContent',
            { contentId: ref.contentId },
          );
          await mutateSessionFiles();
        } catch (error) {
          logger.error('Failed to remove file from server', {
            contentId: ref.contentId,
            error,
          });
          throw error;
        }
      }
    },
    [pendingFiles, currentSession, mutateSessionFiles],
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

  const refetchSessionFiles = useCallback(async () => {
    logger.info('Manually refetching session files');
    await mutateSessionFiles();
  }, [mutateSessionFiles]);

  logger.info('AgentResourceAttachmentContext state', {
    pendingFile: pendingFiles,
    sessionFile: sessionFiles,
    isLoading,
  });

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
        refetchSessionFiles,
      }}
    >
      {children}
    </AgentResourceAttachmentContext.Provider>
  );
}
