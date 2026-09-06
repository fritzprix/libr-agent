import { useState, useCallback, useEffect, useRef } from 'react';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { useAgentMessageTrigger } from '@/hooks/use-agent-message-trigger';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import { useTranslation } from 'react-i18next';
import type { FileNode } from './types';

const logger = getLogger('useWorkspaceFiles');

// Session-scoped cache for preserving expanded directories across unmount / panel reopen
const expandedPathsCache = new Map<string, Set<string>>();

export function clearWorkspaceExpandedPathsCache() {
  expandedPathsCache.clear();
}

/**
 * Normalizes a path to a consistent format:
 * - strips leading './'
 * - strips trailing '/'
 * - replaces backslashes with forward slashes
 * - returns empty string for root ('.' or './')
 */
export function normalizePath(p: string): string {
  let clean = p.replace(/\\/g, '/').replace(/\/+/g, '/').trim();
  while (clean.startsWith('./')) {
    clean = clean.slice(2);
  }
  while (clean.endsWith('/') && clean.length > 0) {
    clean = clean.slice(0, -1);
  }
  if (clean === '.') return '';
  return clean;
}

/**
 * Checks if a path is in the expanded paths set.
 * Root is never considered an expandable subfolder node.
 */
export function isPathExpanded(path: string, set: Set<string>): boolean {
  const norm = normalizePath(path);
  if (!norm) return false;
  return set.has(norm);
}

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

  const cacheKey = `${session?.id ?? 'global'}:${normalizePath(rootPath)}`;

  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(() => {
    return new Set(expandedPathsCache.get(cacheKey) ?? []);
  });
  const expandedPathsRef = useRef<Set<string>>(expandedPaths);
  expandedPathsRef.current = expandedPaths;

  const [fileTree, setFileTree] = useState<FileNode[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Sync cache if session changes
  useEffect(() => {
    const cached = expandedPathsCache.get(cacheKey);
    const next = new Set(cached ?? []);
    expandedPathsRef.current = next;
    setExpandedPaths(next);
  }, [cacheKey]);

  // Recursively fetch nodes, preserving expanded directories with cycle guard
  const fetchDirectoryNodes = useCallback(
    async (
      dirPath: string,
      parentNodeId: string | undefined,
      activeExpanded: Set<string>,
      visited = new Set<string>(),
    ): Promise<FileNode[]> => {
      const normDir = normalizePath(dirPath);
      if (normDir && visited.has(normDir)) {
        return [];
      }
      if (normDir) {
        visited.add(normDir);
      }

      logger.debug('Loading directory', { dirPath, parentNodeId });
      const files = await listWorkspaceFiles(dirPath, session?.id);

      const nodes: FileNode[] = [];
      for (const file of files) {
        const rawNodePath = `${dirPath}/${file.name}`.replace(/\/+/g, '/');
        const normNodePath = normalizePath(rawNodePath);
        const isDir = file.isDirectory;
        const expanded = isDir && isPathExpanded(normNodePath, activeExpanded);

        let children: FileNode[] | undefined = undefined;
        if (isDir) {
          if (expanded) {
            try {
              children = await fetchDirectoryNodes(
                rawNodePath,
                rawNodePath,
                activeExpanded,
                visited,
              );
            } catch (err) {
              logger.warn('Failed to load expanded directory children', {
                path: rawNodePath,
                err,
              });
              children = [];
            }
          } else {
            children = [];
          }
        }

        nodes.push({
          id: rawNodePath,
          name: file.name,
          path: rawNodePath,
          isDirectory: isDir,
          isExpanded: expanded,
          children,
          parent: parentNodeId,
        });
      }

      return nodes;
    },
    [listWorkspaceFiles, session?.id],
  );

  // Load directory contents
  const loadDirectory = useCallback(
    async (path: string, parentNodeId?: string) => {
      setLoading(true);
      setError(null);

      try {
        const nodes = await fetchDirectoryNodes(
          path,
          parentNodeId,
          expandedPathsRef.current,
        );

        if (parentNodeId) {
          // Update specific node's children
          setFileTree((prev) => updateNodeChildren(prev, parentNodeId, nodes));
        } else {
          // Update root, preserving all re-fetched expanded children
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
    [fetchDirectoryNodes],
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

      const norm = normalizePath(node.path);

      if (node.isExpanded) {
        // Collapse
        const nextExpanded = new Set(expandedPathsRef.current);
        if (norm) {
          nextExpanded.delete(norm);
          const prefix = `${norm}/`;
          for (const p of Array.from(nextExpanded)) {
            if (p.startsWith(prefix)) {
              nextExpanded.delete(p);
            }
          }
        }
        expandedPathsRef.current = nextExpanded;
        setExpandedPaths(nextExpanded);
        expandedPathsCache.set(cacheKey, nextExpanded);
        setFileTree((prev) => toggleNodeExpansion(prev, node.id, false));
      } else {
        // Expand
        const nextExpanded = new Set(expandedPathsRef.current);
        if (norm) {
          nextExpanded.add(norm);
        }
        expandedPathsRef.current = nextExpanded;
        setExpandedPaths(nextExpanded);
        expandedPathsCache.set(cacheKey, nextExpanded);
        setFileTree((prev) => toggleNodeExpansion(prev, node.id, true, true));
        await loadDirectory(node.path, node.id);
      }
    },
    [cacheKey, loadDirectory],
  );

  // Expand a specific directory and all its ancestors, then reload tree
  const expandDirectory = useCallback(
    async (path: string) => {
      const norm = normalizePath(path);
      if (!norm) return;

      const nextExpanded = new Set(expandedPathsRef.current);
      const segments = norm.split('/').filter(Boolean);
      let acc = '';
      for (const seg of segments) {
        acc = acc ? `${acc}/${seg}` : seg;
        nextExpanded.add(acc);
      }

      expandedPathsRef.current = nextExpanded;
      setExpandedPaths(nextExpanded);
      expandedPathsCache.set(cacheKey, nextExpanded);

      await loadDirectory(rootPath);
    },
    [cacheKey, loadDirectory, rootPath],
  );

  return {
    fileTree,
    loading,
    error,
    loadDirectory,
    toggleDirectory,
    expandedPaths,
    expandDirectory,
  };
}
