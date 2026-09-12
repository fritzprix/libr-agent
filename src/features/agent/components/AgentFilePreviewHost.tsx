import { useCallback, useMemo, useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { useAgentFilePreview } from '@/context/AgentFilePreviewContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { getLogger } from '@/lib/logger';
import { WorkspaceFilePreviewSheet } from './workspace-panel/WorkspaceFilePreviewSheet';
import type { FileNode } from './workspace-panel/types';

const logger = getLogger('AgentFilePreviewHost');

/**
 * Single in-app file preview sheet for the agent view.
 * Shared by the workspace file tree and tool-result "Open file" actions.
 */
export function AgentFilePreviewHost() {
  const { previewFile, closeFilePreview } = useAgentFilePreview();
  const { session } = useAgentSessionState();
  const { openWorkspaceFileWithDefaultApp } = useRustBackend();
  const { t } = useTranslation();
  const sessionId = previewFile?.sessionId ?? session?.id;

  const [cachedFile, setCachedFile] = useState<FileNode | null>(null);

  useEffect(() => {
    if (previewFile) {
      setCachedFile({
        id: previewFile.path,
        name: previewFile.name,
        path: previewFile.path,
        isDirectory: false,
        size: previewFile.size ?? null,
      });
    }
  }, [previewFile]);

  const file = useMemo<FileNode | null>(() => {
    if (!previewFile) {
      return cachedFile;
    }
    return {
      id: previewFile.path,
      name: previewFile.name,
      path: previewFile.path,
      isDirectory: false,
      size: previewFile.size ?? null,
    };
  }, [previewFile, cachedFile]);

  const handleOpenInDefaultApp = useCallback(
    async (filePath: string) => {
      try {
        logger.debug('Opening file with default app', { path: filePath });
        await openWorkspaceFileWithDefaultApp(filePath, sessionId);
        toast.success(t('agent.workspace.fileOpened'), {
          description: t('agent.workspace.fileOpenedDescription', {
            name: previewFile?.name ?? filePath,
          }),
        });
      } catch (error) {
        logger.error('Failed to open file', { path: filePath, error });
        const message =
          error instanceof Error ? error.message : 'Unknown error occurred';
        toast.error(t('agent.workspace.fileOpenError'), {
          description: message,
        });
      }
    },
    [openWorkspaceFileWithDefaultApp, previewFile?.name, sessionId, t],
  );

  return (
    <WorkspaceFilePreviewSheet
      file={file}
      sessionId={sessionId}
      isOpen={Boolean(previewFile)}
      onClose={closeFilePreview}
      onOpenInDefaultApp={handleOpenInDefaultApp}
    />
  );
}
