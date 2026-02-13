import { useState, useCallback, useEffect } from 'react';
import { useRustBackend, WorkspaceFileItem } from '@/hooks/use-rust-backend';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { useAgentMessageTrigger } from '@/hooks/use-agent-message-trigger';
import { toast } from 'sonner';
import { getLogger } from '@/lib/logger';
import { FileNode } from './types';

const logger = getLogger('AgentWorkspacePanel');

export function useFileTree() {
  const { listWorkspaceFiles } = useRustBackend();
  const { session } = useAgentSessionState();
  const [rootPath] = useState<string>('./');
  const [fileTree, setFileTree] = useState<FileNode[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Helper function to update node children
  const updateNodeChildren = useCallback((
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
  }, []);

  // Helper function to toggle node expansion
  const toggleNodeExpansion = useCallback((
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
          children: toggleNodeExpansion(
            node.children,
            nodeId,
            expanded,
            loading,
          ),
        };
      }
      return node;
    });
  }, []);

  // Load directory contents
  const loadDirectory = useCallback(
    async (path: string, parentNodeId?: string) => {
      setLoading(true);
      setError(null);

      try {
        logger.debug('Loading directory', { path, parentNodeId });
        const files = await listWorkspaceFiles(path, session?.id);
        logger.info('BACKEND RESPONSE', {
          path,
          fileCount: files.length,
          files: files.map((f) => ({
            name: f.name,
            isDirectory: f.isDirectory,
            path: f.path,
          })),
        });

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

          logger.info('CREATING FILENODE', {
            name: file.name,
            path: nodePath,
            isDirectory: file.isDirectory,
            backendIsDirectory: file.isDirectory,
            hasChildren: node.children !== undefined,
          });

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
        toast.error('디렉토리 로드에 실패했습니다');
      } finally {
        setLoading(false);
      }
    },
    [listWorkspaceFiles, session?.id, updateNodeChildren]
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
    [loadDirectory, toggleNodeExpansion]
  );

  // Component lifecycle logging and initial load
  useEffect(() => {
    logger.info('AgentWorkspacePanel initialized', { rootPath });
    loadDirectory(rootPath);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []); // Run once on mount

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
    }
  );

  return {
    rootPath,
    fileTree,
    loading,
    error,
    loadDirectory,
    toggleDirectory,
  };
}
