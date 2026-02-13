import { useCallback } from 'react';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import {
  openWorkspaceInExplorer,
  openWorkspaceInTerminal,
} from '@/lib/backend';
import { toast } from 'sonner';
import { getLogger } from '@/lib/logger';
import { FileNode } from './types';

const logger = getLogger('AgentWorkspacePanel');

export function useWorkspaceActions() {
  const { openWorkspaceFileWithDefaultApp } = useRustBackend();
  const { session } = useAgentSessionState();

  const handleOpenInExplorer = async () => {
    if (!session?.id) return;
    try {
      await openWorkspaceInExplorer(session.id);
    } catch (error) {
      logger.error('Failed to open explorer', error);
      toast.error(`Failed to open explorer: ${error}`);
    }
  };

  const handleOpenInTerminal = async () => {
    if (!session?.id) return;
    try {
      await openWorkspaceInTerminal(session.id);
    } catch (error) {
      logger.error('Failed to open terminal', error);
      toast.error(`Failed to open terminal: ${error}`);
    }
  };

  // Open file with system default app
  const handleOpenFile = useCallback(
    async (node: FileNode) => {
      if (node.isDirectory) {
        logger.warn('Attempted to open a directory, ignoring', {
          path: node.path,
          isDirectory: node.isDirectory,
        });
        return;
      }

      try {
        logger.debug('Opening file with default app', { path: node.path });
        await openWorkspaceFileWithDefaultApp(node.path, session?.id);
        logger.info('File opened successfully', { path: node.path });
        toast.success('File opened', {
          description: `Opened ${node.name} with system default app`,
        });
      } catch (error) {
        logger.error('Failed to open file', { path: node.path, error });
        const message =
          error instanceof Error ? error.message : 'Unknown error occurred';
        toast.error('Failed to open file', {
          description: message,
        });
      }
    },
    [openWorkspaceFileWithDefaultApp, session?.id]
  );

  return {
    handleOpenInExplorer,
    handleOpenInTerminal,
    handleOpenFile,
  };
}
