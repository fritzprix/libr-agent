import { useState, useCallback, useEffect, useRef } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import {
  ChevronRight,
  ChevronDown,
  File,
  Folder,
  FolderOpen,
  RefreshCw,
  Home,
  Upload,
} from 'lucide-react';
import { useRustBackend, WorkspaceFileItem } from '@/hooks/use-rust-backend';
import { useMessageTrigger } from '@/hooks/use-message-trigger';
import { getLogger } from '@/lib/logger';
import {
  useDnDContext,
  type DragAndDropEvent,
  type DragAndDropPayload,
} from '@/context/DnDContext';

const logger = getLogger('WorkspaceFilesPanel');

interface FileNode {
  id: string;
  name: string;
  path: string;
  isDirectory: boolean;
  children?: FileNode[];
  isExpanded?: boolean;
  isLoading?: boolean;
  parent?: string;
}

export function WorkspaceFilesPanel() {
  const { listWorkspaceFiles, downloadWorkspaceFile } = useRustBackend();
  const [rootPath, setRootPath] = useState<string>('./');
  const [fileTree, setFileTree] = useState<FileNode[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const { subscribe } = useDnDContext();
  const [dragState, setDragState] = useState<{ isOver: boolean }>({
    isOver: false,
  });

  // Component lifecycle logging
  useEffect(() => {
    logger.info('WorkspaceFilesPanel initialized', { rootPath });
    loadDirectory(rootPath);
  }, []);

  // Message-based automatic file list updates
  useMessageTrigger(
    () => {
      if (rootPath) {
        logger.debug('Message-triggered file refresh', { rootPath });
        loadDirectory(rootPath);
      }
    },
    {
      debounceMs: 100, // 500ms debouncing
    },
  );

  // Load directory contents
  const loadDirectory = useCallback(
    async (path: string, parentNodeId?: string) => {
      setLoading(true);
      setError(null);

      try {
        logger.debug('Loading directory', { path, parentNodeId });
        const files = await listWorkspaceFiles(path);

        const nodes: FileNode[] = files.map((file: WorkspaceFileItem) => ({
          id: `${path}/${file.name}`,
          name: file.name,
          path: `${path}/${file.name}`.replace('//', '/'),
          isDirectory: file.isDirectory,
          isExpanded: false,
          children: file.isDirectory ? [] : undefined,
          parent: parentNodeId,
        }));

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
      } finally {
        setLoading(false);
      }
    },
    [listWorkspaceFiles],
  );

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

  // Toggle directory expansion
  const toggleDirectory = useCallback(
    async (node: FileNode) => {
      if (!node.isDirectory) return;

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
  };

  // Handle external file drops from DnDContext
  const handleWorkspaceFileDrop = useCallback(
    async (paths: string[]) => {
      logger.info('External files dropped on workspace', {
        fileCount: paths.length,
        targetPath: rootPath,
      });

      // For now, just show a notification about the drop
      // In a real implementation, you might want to move/copy these files
      alert(`Files dropped on workspace: ${paths.join(', ')}`);

      // Refresh the directory
      await loadDirectory(rootPath);
    },
    [loadDirectory, rootPath],
  );

  // Subscribe to DnD events
  useEffect(() => {
    logger.debug('Setting up DnD subscription for WorkspaceFilesPanel');

    const handler = (event: DragAndDropEvent, payload: DragAndDropPayload) => {
      logger.debug('DnD event received in WorkspaceFilesPanel', {
        event,
        paths: payload.paths,
      });

      if (event === 'drag-over') {
        setDragState({ isOver: true });
      } else if (event === 'drop') {
        setDragState({ isOver: false });
        if (payload.paths) {
          handleWorkspaceFileDrop(payload.paths);
        }
      } else if (event === 'leave') {
        setDragState({ isOver: false });
      }
    };

    const unsub = subscribe(panelRef, handler, { priority: 5 });

    return () => {
      logger.debug('Cleaning up DnD subscription for WorkspaceFilesPanel');
      unsub();
    };
  }, [subscribe, handleWorkspaceFileDrop]);

  // Navigate to directory
  const navigateToDirectory = useCallback(
    (path: string) => {
      setRootPath(path);
      loadDirectory(path);
    },
    [loadDirectory],
  );

  // Download file
  const handleDownloadFile = useCallback(
    async (node: FileNode) => {
      if (node.isDirectory) return;

      try {
        await downloadWorkspaceFile(node.path);
        logger.info('File download initiated', { path: node.path });
      } catch (error) {
        logger.error('Failed to download file', { path: node.path, error });
      }
    },
    [downloadWorkspaceFile],
  );

  // Render file tree node
  const renderNode = (node: FileNode, depth: number = 0) => {
    const Icon = node.isDirectory
      ? node.isExpanded
        ? FolderOpen
        : Folder
      : File;

    return (
      <div key={node.id} className="select-none">
        <div
          className="flex items-center gap-1 px-2 py-1 hover:bg-muted/50 cursor-pointer group"
          style={{ paddingLeft: `${8 + depth * 16}px` }}
          onClick={() => {
            if (node.isDirectory) {
              toggleDirectory(node);
            } else {
              handleDownloadFile(node);
            }
          }}
        >
          {node.isDirectory && (
            <div
              className="w-4 h-4 flex items-center justify-center"
              onClick={(e) => {
                e.stopPropagation();
                toggleDirectory(node);
              }}
            >
              {node.isLoading ? (
                <RefreshCw className="w-3 h-3 animate-spin" />
              ) : node.isExpanded ? (
                <ChevronDown className="w-3 h-3" />
              ) : (
                <ChevronRight className="w-3 h-3" />
              )}
            </div>
          )}

          <Icon className="w-4 h-4 flex-shrink-0" />

          <span className="text-xs truncate flex-1" title={node.name}>
            {node.name}
          </span>

          {node.isDirectory && (
            <Badge
              variant="secondary"
              className="text-xs px-1 opacity-0 group-hover:opacity-100"
            >
              {node.children?.length || 0}
            </Badge>
          )}
        </div>

        {node.isExpanded && node.children && (
          <div>
            {node.children.map((child) => renderNode(child, depth + 1))}
          </div>
        )}
      </div>
    );
  };

  return (
    <div
      ref={panelRef}
      className={`w-80 h-full ${
        dragState.isOver ? 'ring-2 ring-green-500' : ''
      }`}
    >
      <Card
        className={`w-full h-full flex flex-col bg-background/95 backdrop-blur border-border/50 ${
          dragState.isOver ? 'border-green-500 bg-green-500/10' : ''
        }`}
      >
        <CardHeader className="pb-3">
          <div className="flex items-center justify-between">
            <CardTitle className="text-sm font-medium flex items-center gap-2">
              <Folder className="w-4 h-4" />
              Workspace Files
            </CardTitle>
            <div className="flex items-center gap-1">
              <Button
                variant="ghost"
                size="sm"
                onClick={() => navigateToDirectory('/')}
                className="h-6 w-6 p-0"
                title="Go to root"
              >
                <Home className="w-3 h-3" />
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => loadDirectory(rootPath)}
                className="h-6 w-6 p-0"
                title="Refresh"
              >
                <RefreshCw
                  className={`w-3 h-3 ${loading ? 'animate-spin' : ''}`}
                />
              </Button>
            </div>
          </div>

          <div
            className="text-xs text-muted-foreground truncate"
            title={rootPath}
          >
            {rootPath}
          </div>
        </CardHeader>

        <CardContent className="flex-1 overflow-auto px-0">
          {error && (
            <div className="text-xs text-destructive p-2 mx-2 rounded bg-destructive/10">
              {error}
            </div>
          )}

          {loading && fileTree.length === 0 ? (
            <div className="flex items-center justify-center py-8">
              <RefreshCw className="w-4 h-4 animate-spin mr-2" />
              <span className="text-xs text-muted-foreground">Loading...</span>
            </div>
          ) : (
            <div className="space-y-0">
              {fileTree.map((node) => renderNode(node))}

              {fileTree.length === 0 && !loading && (
                <div className="text-xs text-muted-foreground text-center py-8">
                  No files found
                </div>
              )}
            </div>
          )}
        </CardContent>

        <div className="border-2 border-dashed border-muted-foreground/25 rounded m-2 p-2 text-center text-xs text-muted-foreground hover:border-muted-foreground/50 transition-colors">
          <Upload className="w-4 h-4 mx-auto mb-1" />
          Drop files here to upload
        </div>
      </Card>
    </div>
  );
}
