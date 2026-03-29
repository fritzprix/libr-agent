import React, {
  createContext,
  useContext,
  useCallback,
  useState,
  useRef,
  useEffect,
} from 'react';
import useSWR from 'swr';
import { getLogger } from '@/lib/logger';
import {
  syncFileToWorkspace,
  createFileSizeErrorMessage,
} from '@/lib/workspace-sync-service';
import { useSettings } from '@/hooks/use-settings';
import type { AttachmentItem } from '@/models/attachments';
import { AttachmentReference } from '@/models/chat';
import {
  saveAgentFile,
  agentCallBuiltinTool,
  deleteAgentFile,
} from '@/features/agent/api/agent-backend';
import { getWorkspaceDir } from '@/lib/backend/workspace';
import { workspacePathToFileUrl } from '@/lib/file-url';
import { getMimeTypeFromFilename } from '@/lib/mime-utils';

const logger = getLogger('AgentResourceAttachmentContext');

export interface PendingFileInput {
  file?: File;
  url: string;
  filename?: string;
  mimeType: string;
  originalPath?: string;
  status?: AttachmentReference['status'];
  blobCleanup?: () => void;
}

interface AgentResourceAttachmentContextType {
  pendingFiles: AttachmentReference[];
  sessionFiles: AttachmentReference[];
  isLoading: boolean;
  addPendingFiles: (files: PendingFileInput[]) => AttachmentReference[];
  updatePendingFile: (
    pendingId: string,
    updates: Partial<AttachmentReference>,
  ) => void;
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
  sessionId,
}: {
  children: React.ReactNode;
  sessionId: string;
}) {
  const {
    value: { system },
  } = useSettings();

  const maxBytes =
    Math.min(
      system?.maxFileUploadSizeMB ?? 50,
      system?.workspaceCapacityMB ?? 10,
    ) *
    1024 *
    1024;

  const [pendingFiles, setPendingFiles] = useState<AttachmentReference[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  // Use SWR to fetch session files via Agent V2 session-specific proxy
  const { data: sessionFiles = [], mutate: mutateSessionFiles } = useSWR(
    sessionId ? ['agent_content_list', sessionId] : null,
    async () => {
      if (!sessionId) {
        logger.warn('[AgentResourceAttachmentContext] No session ID available');
        return [];
      }

      logger.info(
        '[AgentResourceAttachmentContext] Fetching files via Agent V2 proxy',
        { sessionId },
      );

      try {
        // Use Agent V2 session-specific proxy to call list
        const response = await agentCallBuiltinTool<{
          contents: AttachmentItem[];
        }>(sessionId, 'attachments__list', {
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
        let contents: AttachmentItem[] = [];
        if (
          response.structuredContent &&
          typeof response.structuredContent === 'object' &&
          'contents' in response.structuredContent
        ) {
          contents =
            (response.structuredContent as { contents: AttachmentItem[] })
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
          'Agent V2 Attachments listing failed, will retry on next revalidation',
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
  // NOTE: Attachments are auto-created on first use (add/list)
  // No explicit createStore tool is needed
  const sessionStoreIdRef = useRef<string | undefined>();

  // Update sessionId cache when the active session changes
  // Note: pendingFiles are local state and naturally reset on component remount
  // (which happens via key={sessionId} in parent)
  useEffect(() => {
    sessionStoreIdRef.current = sessionId;
  }, [sessionId]);

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
    (
      files: (PendingFileInput & { status?: AttachmentReference['status'] })[],
    ) => {
      const newPending: AttachmentReference[] = files.map((file) => ({
        sessionId: sessionId || '',
        pendingId: `pending_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
        status: file.status || ('pending' as const),
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
      return newPending;
    },
    [sessionId, extractFilenameFromUrl],
  );

  const updatePendingFile = useCallback(
    (pendingId: string, updates: Partial<AttachmentReference>) => {
      setPendingFiles((prev) =>
        prev.map((file) =>
          file.pendingId === pendingId ? { ...file, ...updates } : file,
        ),
      );
    },
    [],
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

      if (!sessionId) {
        throw new Error('Session not available');
      }

      // Store will be auto-created on first add call
      const storeId = sessionId;

      let fileUrl: string;
      let actualMimeType: string;
      let fileSize: number;
      let workspacePath: string | undefined;
      let fetchedBlob: Blob | undefined;

      if (file) {
        try {
          workspacePath = await syncFileToWorkspace(file, sessionId);
          const workspaceDir = await getWorkspaceDir(sessionId);
          fileUrl = workspacePathToFileUrl(workspaceDir, workspacePath);
          actualMimeType =
            file.type || mimeType || getMimeTypeFromFilename(actualFilename);
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
          actualMimeType =
            file.type || mimeType || getMimeTypeFromFilename(actualFilename);
          fileSize = file.size;
        }
      } else {
        try {
          const response = await fetch(url);
          if (!response.ok) throw new Error(`Failed to fetch ${url}`);
          fetchedBlob = await response.blob();
          // Prefer the caller-supplied mimeType (already resolved from filename)
          // over blob.type which is often 'application/octet-stream' for local files.
          // Final fallback: derive from extension (handles Linux WebKitGTK empty type).
          const resolvedMimeType =
            mimeType && mimeType !== 'application/octet-stream'
              ? mimeType
              : fetchedBlob.type ||
                mimeType ||
                getMimeTypeFromFilename(actualFilename);
          const downloadedFile = new File([fetchedBlob], actualFilename, {
            type: resolvedMimeType,
          });
          workspacePath = await syncFileToWorkspace(downloadedFile, sessionId);
          const workspaceDir = await getWorkspaceDir(sessionId);
          fileUrl = workspacePathToFileUrl(workspaceDir, workspacePath);
          actualMimeType = resolvedMimeType;
          fileSize = fetchedBlob.size;
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
            mimeType ||
            blobResult.type ||
            getMimeTypeFromFilename(actualFilename);
          fileSize = blobResult.size || 0;
          fetchedBlob = undefined;
        }
      }

      // --- Inline multimodal handling (image/audio) ---
      // Image and audio files are passed directly to the LLM as base64 instead of
      // being indexed in the attachments store. No workspace sync is needed.
      if (
        actualMimeType === 'application/octet-stream' ||
        actualMimeType === ''
      ) {
        // Extension-based detection was exhausted — log for visibility.
        logger.warn(
          'Could not resolve MIME type from file.type, mimeType param, or extension',
          { filename: actualFilename },
        );
      }
      const isInlineType =
        actualMimeType.startsWith('image/') ||
        actualMimeType.startsWith('audio/');

      if (isInlineType) {
        logger.info(
          'File is image/audio — storing stable inline media reference',
          {
            filename: actualFilename,
            mimeType: actualMimeType,
            hasStableUri: !fileUrl.startsWith('blob:'),
          },
        );

        const inlineType = actualMimeType.startsWith('image/')
          ? ('image' as const)
          : ('audio' as const);

        let fallbackBase64Data: string | undefined;
        if (fileUrl.startsWith('blob:')) {
          try {
            const sourceBlob: Blob =
              file ??
              fetchedBlob ??
              (() => {
                throw new Error('No data source available for inline content');
              })();
            const buffer = await sourceBlob.arrayBuffer();
            const bytes = new Uint8Array(buffer);
            let binary = '';
            for (let i = 0; i < bytes.length; i++) {
              binary += String.fromCharCode(bytes[i]);
            }
            fallbackBase64Data = btoa(binary);
          } catch (readError) {
            logger.error('Failed to read fallback inline media as base64', {
              filename: actualFilename,
              error: readError,
            });
            throw new Error(
              `Failed to read file "${actualFilename}" for inline attachment.`,
            );
          }
        }

        return {
          sessionId,
          status: 'inline',
          filename: actualFilename,
          mimeType: actualMimeType,
          size: fileSize,
          lineCount: 0,
          preview: actualFilename,
          uploadedAt: new Date().toISOString(),
          inlineContent: {
            type: inlineType,
            data: fallbackBase64Data,
            uri: fileUrl,
            mimeType: actualMimeType,
          },
        };
      }

      const SUPPORTED_EXTENSIONS = /\.(txt|md|json|pdf|docx|xlsx)$/i;
      const isSupported = SUPPORTED_EXTENSIONS.test(actualFilename);

      if (!isSupported) {
        logger.info(
          'File type not supported by Attachments, saving to workspace only',
          { filename: actualFilename },
        );

        // Return AttachmentReference without contentId for workspace-only files
        return {
          sessionId,
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
        // Use session-specific saveAgentFile instead of global server.add
        // This ensures the file is associated with the correct agent session
        const result = await saveAgentFile(storeId, actualFilename, {
          content: undefined, // Content is handled via fileUrl or direct upload
          fileUrl: fileUrl,
          metadata: {
            mimeType: actualMimeType,
            size: fileSize,
            uploadedAt: new Date().toISOString(),
            filename: actualFilename,
          },
        });

        if (!workspacePath && file) {
          try {
            workspacePath = await syncFileToWorkspace(file, sessionId);
          } catch (error) {
            logger.warn(
              'Workspace sync failed (retry), continuing with content-store only',
              { error },
            );
          }
        }

        // Debug logging to verify backend response
        logger.debug('[AgentResourceAttachmentContext] saveAgentFile result:', {
          result,
        });

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
    [sessionId, extractFilenameFromUrl, convertToBlobUrl],
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
        if (!sessionId) return;
        if (!ref.contentId) {
          logger.error('Cannot delete committed file without contentId', {
            filename: ref.filename,
          });
          return;
        }

        try {
          await deleteAgentFile(sessionId, ref.contentId);
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
    [pendingFiles, sessionId, mutateSessionFiles],
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

  const validateFiles = useCallback(
    (files: File[]): File[] => {
      return files.filter((file) => {
        if (file.size > maxBytes) {
          // In a real app we might toast here, but for now we just filter
          const msg = createFileSizeErrorMessage(
            file.name,
            file.size,
            maxBytes,
          );
          logger.warn(msg);
          return false;
        }
        return true;
      });
    },
    [maxBytes],
  );

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
        updatePendingFile,
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
