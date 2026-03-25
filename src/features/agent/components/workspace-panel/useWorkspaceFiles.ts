import { useState, useCallback, useEffect } from 'react';
import {
  useRustBackend,
  type WorkspaceFileItem,
} from '@/hooks/use-rust-backend';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { useAgentMessageTrigger } from '@/hooks/use-agent-message-trigger';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import { useTranslation } from 'react-i18next';
import type { FileNode } from './types';

const logger = getLogger('useWorkspaceFiles');

// Helper function to update node children
const updateNodeChildren = (
  nodes: FileNode[],
  nodeId: string,
  children: FileNode[],
): FileNode[] => {
  return nodes.map((node) => {
    if (node.id === nodeId) {
      return { ...node, children, isLoading: false, isExpanded: true };
    }
    if (node.children) {
      return {
        ...node,
        children: updateNodeChildren(node.children, nodeId, children),
      };
    }
    return node;
  });
};

// Helper function to toggle node expansion
const toggleNodeExpansion = (
  nodes: FileNode[],
  nodeId: string,
  expanded: boolean,
  loading: boolean = false,
): FileNode[] => {
  return nodes.map((node) => {
    if (node.id === nodeId) {
      return { ...node, isExpanded: expanded, isLoading: loading };
    }
    if (node.children) {
      return {
        ...node,
        children: toggleNodeExpansion(node.children, nodeId, expanded, loading),
      };
    }
    return node;
  });
};

export function useWorkspaceFiles(rootPath: string) {
  const { t } = useTranslation();
  const { listWorkspaceFiles } = useRustBackend();
  const { session } = useAgentSessionState();

  const [fileTree, setFileTree] = useState<FileNode[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Load directory contents
  const loadDirectory = useCallback(
    async (path: string, parentNodeId?: string) => {
      setLoading(true);
      setError(null);

      try {
        logger.debug('Loading directory', { path, parentNodeId });
        const files = await listWorkspaceFiles(path, session?.id);

        const nodes: FileNode[] = files.map((file: WorkspaceFileItem) => {
          const nodePath = `${path}/${file.name}`.replace('//', '/');
          const node = {
            id: `${path}/${file.name}`,
            name: file.name,
            path: nodePath,
            isDirectory: file.isDirectory,
            isExpanded: false,
            children: file.isDirectory ? [] : undefined,
            parent: parentNodeId,
          };
          return node;
        });

        if (parentNodeId) {
          // Update specific node's children
          setFileTree((prev) => updateNodeChildren(prev, parentNodeId, nodes));
        } else {
          // Update root
          setFileTree(nodes);
        }

        logger.info('Directory loaded successfully', {
          path,
          fileCount: nodes.length,
          parentNodeId,
        });
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : 'Failed to load directory';
        logger.error('Failed to load directory', { path, error: errorMessage });
        setError(errorMessage);
        toast.error(t('agent.workspace.loadError'));
      } finally {
        setLoading(false);
      }
    },
    [listWorkspaceFiles, session?.id],
  );

  // Component lifecycle loading
  useEffect(() => {
    logger.info('AgentWorkspacePanel initialized', { rootPath });
    loadDirectory(rootPath);
  }, [rootPath, loadDirectory]);

  // Message-based automatic file list updates
  useAgentMessageTrigger(
    () => {
      if (rootPath) {
        logger.info('Message-triggered file refresh', { rootPath });
        loadDirectory(rootPath);
      }
    },
    {
      debounceMs: 500, // 500ms debouncing
    },
  );

  // Toggle directory expansion
  const toggleDirectory = useCallback(
    async (node: FileNode) => {
      if (!node.isDirectory) {
        logger.warn('Attempted to toggle non-directory', {
          path: node.path,
          isDirectory: node.isDirectory,
        });
        return;
      }

      logger.debug('Toggling directory', {
        path: node.path,
        isExpanded: node.isExpanded,
      });

      if (node.isExpanded) {
        // Collapse
        setFileTree((prev) => toggleNodeExpansion(prev, node.id, false));
      } else {
        // Expand
        setFileTree((prev) => toggleNodeExpansion(prev, node.id, true, true));
        await loadDirectory(node.path, node.id);
      }
    },
    [loadDirectory],
  );

  return {
    fileTree,
    loading,
    error,
    loadDirectory,
    toggleDirectory,
  };
}
