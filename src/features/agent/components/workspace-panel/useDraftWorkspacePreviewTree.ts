import { useCallback, useEffect, useState } from 'react';

import { listWorkspaceFilePathsForPath } from '@/lib/backend/workspace';
import { getLogger } from '@/lib/logger';

import { buildFileTreeFromPaths } from './buildFileTreeFromPaths';
import type { FileNode } from './types';

const logger = getLogger('useDraftWorkspacePreviewTree');
const PREVIEW_MAX_DEPTH = 32;

function toggleNodeExpansion(
  nodes: FileNode[],
  nodeId: string,
  expanded: boolean,
): FileNode[] {
  return nodes.map((node) => {
    if (node.id === nodeId) {
      return { ...node, isExpanded: expanded };
    }

    if (node.children) {
      return {
        ...node,
        children: toggleNodeExpansion(node.children, nodeId, expanded),
      };
    }

    return node;
  });
}

export function useDraftWorkspacePreviewTree(workspacePath: string) {
  const [fileTree, setFileTree] = useState<FileNode[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!workspacePath) {
      setFileTree([]);
      setError(null);
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const paths = await listWorkspaceFilePathsForPath(
        workspacePath,
        PREVIEW_MAX_DEPTH,
      );
      setFileTree(buildFileTreeFromPaths(paths));
    } catch (err) {
      const message =
        err instanceof Error ? err.message : 'Failed to load workspace preview';
      logger.error('Failed to load draft workspace preview', {
        workspacePath,
        error: message,
      });
      setError(message);
      setFileTree([]);
    } finally {
      setLoading(false);
    }
  }, [workspacePath]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const toggleDirectory = useCallback((node: FileNode) => {
    if (!node.isDirectory) {
      return;
    }

    setFileTree((currentTree) =>
      toggleNodeExpansion(currentTree, node.id, !node.isExpanded),
    );
  }, []);

  return {
    fileTree,
    loading,
    error,
    refresh,
    toggleDirectory,
  };
}
