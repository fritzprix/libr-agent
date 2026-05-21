import React, {
  createContext,
  useContext,
  useCallback,
  useEffect,
  useState,
} from 'react';
import useSWR from 'swr';
import { getLogger } from '@/lib/logger';
import { createFileSizeErrorMessage } from '@/lib/workspace-sync-service';
import { useSettings } from '@/hooks/use-settings';
import type { AttachmentItem } from '@/models/attachments';
import { AttachmentReference } from '@/models/chat';
import {
  agentCallBuiltinTool,
  deleteAgentFile,
} from '@/features/agent/api/agent-backend';
import {
  cleanupPendingAttachmentBlobs,
  createPendingAttachmentReferences,
  type PendingFileInput,
} from '../lib/resource-attachment-utils';
import { addAgentAttachment } from '../lib/resource-attachment-operations';

const logger = getLogger('AgentResourceAttachmentContext');

export type { PendingFileInput } from '../lib/resource-attachment-utils';

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
    value: { system, experimental },
  } = useSettings();

  const maxBytes = (system?.maxFileUploadSizeMB ?? 50) * 1024 * 1024;

  const [pendingFiles, setPendingFiles] = useState<AttachmentReference[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  useEffect(() => {
    setPendingFiles((previousFiles) => {
      cleanupPendingAttachmentBlobs(previousFiles);
      return [];
    });
    setIsLoading(false);
  }, [sessionId]);

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

  const addPendingFiles = useCallback(
    (
      files: (PendingFileInput & { status?: AttachmentReference['status'] })[],
    ) => {
      const newPending = createPendingAttachmentReferences(
        files,
        sessionId || '',
      );

      setPendingFiles((prev) => [...prev, ...newPending]);
      return newPending;
    },
    [sessionId],
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
      file?: File,
    ): Promise<AttachmentReference> => {
      if (!sessionId) {
        throw new Error('Session not available');
      }

      return addAgentAttachment({
        sessionId,
        url,
        mimeType,
        filename,
        file,
        inlineAudio: experimental?.inlineAudioAttachment !== false,
      });
    },
    [sessionId, experimental],
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

      cleanupPendingAttachmentBlobs(pendingFiles);

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
        if (fileToRemove) {
          cleanupPendingAttachmentBlobs([fileToRemove]);
        }
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
    cleanupPendingAttachmentBlobs(pendingFiles);
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
