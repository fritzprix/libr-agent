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
import { toast } from 'sonner';
import { getLogger } from '@/lib/logger';
import {
  useDnDContext,
  type DragAndDropEvent,
  type DragAndDropPayload,
} from '@/context/DnDContext';
import { useSessionContext } from '@/context/SessionContext';
import { useChatActions } from '@/context/ChatContext';
import { createId } from '@paralleldrive/cuid2';

import { createToolMessagePair } from '@/lib/chat-utils';
import { stringToMCPContentArray } from '@/lib/utils';

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
  const { listWorkspaceFiles, downloadWorkspaceFile, callBuiltinTool } =
    useRustBackend();
  const { current: session } = useSessionContext();
  const { submit } = useChatActions();
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
        logger.info('Message-triggered file refresh', { rootPath });
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
      if (!session?.id) return;

      logger.info('External files dropped on workspace', {
        fileCount: paths.length,
        targetPath: rootPath,
      });

      try {
        for (const srcPath of paths) {
          const fileName = srcPath.split('/').pop() || 'unknown';
          const destPath = `${rootPath}/${fileName}`.replace(/\/+/g, '/');
          const destRelPath = destPath.startsWith('./')
            ? destPath.slice(2)
            : destPath;

          // Call builtin workspace tool
          const response = await callBuiltinTool('workspace', 'import_file', {
            src_abs_path: srcPath,
            dest_rel_path: destRelPath,
          });

          // Create tool messages for chat history
          const toolCallId = createId();

          // Build a safe textual result for UI. The backend returns an MCPResponse
          // which may have result.content (array) or an error. Protect against
          // malformed responses so the UI doesn't insert `{ type: 'text' }` with
          // missing `text` field.
          let resultText = '';

          try {
            if (response.error) {
              // Prefer explicit error message when backend indicates an error
              resultText = `❌ ${response.error.message}`;
            } else if (response.result) {
              // If result.content is present and is an array of MCPContent,
              // try to extract any text entries or stringify the result.
              // Handle multiple shapes defensively.
              // (Some older paths returned a plain string in `result`.)
              // Use JSON.stringify fallback so message is never empty.

              const resAsUnknown: unknown = (response.result as unknown);
              const content =
                typeof resAsUnknown === 'object' && resAsUnknown !== null
                  ? (resAsUnknown as Record<string, unknown>)['content']
                  : undefined;
              if (Array.isArray(content) && content.length > 0) {
                const texts: string[] = [];
                for (const item of content) {
                  if (item && typeof item === 'object') {
                    // runtime-safe check for text field
                    if (
                      'text' in (item as Record<string, unknown>) &&
                      typeof (item as Record<string, unknown>)['text'] === 'string'
                    ) {
                      texts.push((item as Record<string, unknown>)['text'] as string);
                    } else if (
                      (item as Record<string, unknown>)['type'] === 'text' &&
                      !('text' in (item as Record<string, unknown>))
                    ) {
                      // explicit text type but missing text field - skip
                    } else {
                      try {
                        texts.push(JSON.stringify(item));
                      } catch {
                        // ignore
                      }
                    }
                  }
                }

                if (texts.length > 0) resultText = texts.join('\n');
                else resultText = JSON.stringify(response.result);
                } else if (typeof resAsUnknown === 'string') {
                resultText = resAsUnknown as string;
              } else {
                resultText = JSON.stringify(response.result);
              }
            } else {
              // No result or error: fallback message
              resultText = 'No result returned from import_file';
            }
          } catch (e) {
            resultText = `Failed to parse tool response: ${
              e instanceof Error ? e.message : String(e)
            }`;
          }

          const [toolCallMessage, toolResultMessage] = createToolMessagePair(
            'import_file',
            { src_abs_path: srcPath, dest_rel_path: destRelPath },
            stringToMCPContentArray(resultText),
            toolCallId,
            session.id,
          );

          await submit([toolCallMessage, toolResultMessage]);
        }

        // Refresh directory after import
        await loadDirectory(rootPath);
      } catch (error) {
        logger.error('File import failed', error);
        // Show a friendly toast for the user
        const message =
          error instanceof Error ? error.message : 'Unknown error occurred';
        toast.error('Failed to import file', {
          description: message,
        });
      }
    },
    [callBuiltinTool, submit, session, rootPath, loadDirectory],
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
      if (node.isDirectory) {
        logger.warn('Attempted to download a directory, ignoring', {
          path: node.path,
          isDirectory: node.isDirectory,
        });
        return;
      }

      try {
        logger.debug('Downloading file', { path: node.path });
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
            logger.info('DIRECTORY CLICK ANALYSIS', {
              path: node.path,
              name: node.name,
              isDirectory: node.isDirectory,
            });

            if (node.isDirectory) {
              logger.info('CALLING toggleDirectory', { path: node.path });
              toggleDirectory(node);
            } else {
              logger.info('CALLING handleDownloadFile', { path: node.path });
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
