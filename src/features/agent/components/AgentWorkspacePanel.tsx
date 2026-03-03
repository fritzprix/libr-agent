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
  Upload,
  Terminal,
  AlertTriangle,
} from 'lucide-react';
import { useRustBackend, WorkspaceFileItem } from '@/hooks/use-rust-backend';
import { useAgentMessageTrigger } from '@/hooks/use-agent-message-trigger';
import { toast } from 'sonner';
import { getLogger } from '@/lib/logger';
import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/input';
import { open } from '@tauri-apps/plugin-dialog';
import {
  openWorkspaceInExplorer,
  openWorkspaceInTerminal,
  getWorkspaceOverride,
  setWorkspaceOverride,
  cancelWorkspaceOverride,
} from '@/lib/backend';
import {
  useDnDContext,
  type DragAndDropEvent,
  type DragAndDropPayload,
} from '@/context/DnDContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { useAgentChatActions } from '@/context/AgentChatContext';
import { createId } from '@paralleldrive/cuid2';

import { createToolMessagePair } from '@/lib/chat-utils';
import { stringToMCPContentArray } from '@/lib/utils';

const logger = getLogger('AgentWorkspacePanel');

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

interface FileTreeNodeProps {
  node: FileNode;
  depth?: number;
  onToggle: (node: FileNode) => void;
  onOpen: (node: FileNode) => void;
}

const FileTreeNode = ({
  node,
  depth = 0,
  onToggle,
  onOpen,
}: FileTreeNodeProps) => {
  const Icon = node.isDirectory
    ? node.isExpanded
      ? FolderOpen
      : Folder
    : File;

  return (
    <div className="select-none">
      <div
        className="flex items-center gap-1 px-2 py-1 hover:bg-muted/50 group"
        style={{ paddingLeft: `${8 + depth * 16}px` }}
        onClick={() => {
          // Keep mouse click behavior for padding area
          if (node.isDirectory) {
            onToggle(node);
          } else {
            onOpen(node);
          }
        }}
      >
        {node.isDirectory ? (
          <button
            type="button"
            className="w-4 h-4 flex items-center justify-center focus:outline-none focus:ring-1 focus:ring-ring rounded-sm"
            onClick={(e) => {
              e.stopPropagation();
              onToggle(node);
            }}
            aria-label={node.isExpanded ? 'Collapse' : 'Expand'}
            aria-expanded={node.isExpanded}
          >
            {node.isLoading ? (
              <RefreshCw className="w-3 h-3 animate-spin" />
            ) : node.isExpanded ? (
              <ChevronDown className="w-3 h-3" />
            ) : (
              <ChevronRight className="w-3 h-3" />
            )}
          </button>
        ) : (
          <div className="w-4 h-4" /> // Spacer
        )}

        <div
          role="button"
          tabIndex={0}
          className="flex-1 flex items-center gap-1 min-w-0 cursor-pointer focus:outline-none focus:ring-1 focus:ring-ring rounded-sm px-1"
          onClick={(e) => {
            e.stopPropagation();
            if (node.isDirectory) {
              onToggle(node);
            } else {
              onOpen(node);
            }
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault();
              e.stopPropagation();
              if (node.isDirectory) {
                onToggle(node);
              } else {
                onOpen(node);
              }
            }
          }}
          aria-expanded={node.isDirectory ? node.isExpanded : undefined}
        >
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
      </div>

      {node.isExpanded && node.children && (
        <div>
          {node.children.map((child) => (
            <FileTreeNode
              key={child.id}
              node={child}
              depth={depth + 1}
              onToggle={onToggle}
              onOpen={onOpen}
            />
          ))}
        </div>
      )}
    </div>
  );
};

export function AgentWorkspacePanel() {
  const { t } = useTranslation();
  const {
    listWorkspaceFiles,
    openWorkspaceFileWithDefaultApp,
    agentCallBuiltinTool,
  } = useRustBackend();
  const { session } = useAgentSessionState();
  const { submit, injectMessages } = useAgentChatActions();
  const [rootPath] = useState<string>('./');
  const [workspaceOverride, setWorkspaceOverridePath] = useState<string>('');
  const [isOverrideActive, setIsOverrideActive] = useState(false);
  const [fileTree, setFileTree] = useState<FileNode[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const { subscribe } = useDnDContext();
  const [dragState, setDragState] = useState<{ isOver: boolean }>({
    isOver: false,
  });

  const [isSettingOverride, setIsSettingOverride] = useState(false);
  const [isCancelingOverride, setIsCancelingOverride] = useState(false);
  const [isUploading, setIsUploading] = useState(false);
  const [isOpeningNative, setIsOpeningNative] = useState(false);
  const openingNativeLock = useRef(false);

  // Component lifecycle logging
  useEffect(() => {
    logger.info('AgentWorkspacePanel initialized', { rootPath });
    loadDirectory(rootPath);
  }, []);

  // Load current workspace override
  useEffect(() => {
    if (session?.id) {
      getWorkspaceOverride(session.id)
        .then((path) => {
          if (path) {
            setWorkspaceOverridePath(path);
            setIsOverrideActive(true);
          } else {
            setWorkspaceOverridePath('');
            setIsOverrideActive(false);
          }
        })
        .catch((err) => logger.error('Failed to load workspace override', err));
    }
  }, [session?.id]);

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
        toast.error(t('agent.workspace.loadError'));
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
          // OS-agnostic path parsing: support both / and \\ separators
          const fileName = srcPath.split(/[/\\]/).pop() || 'unknown';
          const destPath = `${rootPath}/${fileName}`.replace(/\/+/g, '/');
          const destRelPath = destPath.startsWith('./')
            ? destPath.slice(2)
            : destPath;

          // Call builtin workspace tool (returns MCPResult directly after fix)
          const response = (await agentCallBuiltinTool(
            session.id,
            'workspace__importFile',
            {
              src_abs_path: srcPath,
              dest_rel_path: destRelPath,
            },
          )) as {
            content?: Array<{ type: string; text?: string }>;
            structuredContent?: unknown;
            isError?: boolean;
          };

          // Create tool messages for chat history
          const toolCallId = createId();

          // Build a safe textual result for UI.
          let resultText = '';

          try {
            // Check if this is an error result
            if (response.isError === true) {
              // Extract error message from content
              const errorContent = response.content?.[0];
              if (
                errorContent &&
                typeof errorContent === 'object' &&
                'text' in errorContent
              ) {
                resultText = `${errorContent.text}`;
              } else {
                resultText = 'Tool execution failed';
              }
            } else if (response.content && Array.isArray(response.content)) {
              // Extract text from content array
              const texts: string[] = [];
              for (const item of response.content) {
                if (item && typeof item === 'object') {
                  if (
                    'text' in (item as Record<string, unknown>) &&
                    typeof (item as Record<string, unknown>)['text'] ===
                    'string'
                  ) {
                    texts.push(
                      (item as Record<string, unknown>)['text'] as string,
                    );
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
              else resultText = JSON.stringify(response.content);
            } else {
              resultText = 'No result returned from importFile';
            }
          } catch (e) {
            resultText = `Failed to parse tool response: ${e instanceof Error ? e.message : String(e)
              }`;
          }

          const [toolCallMessage, toolResultMessage] = createToolMessagePair(
            'workspace__importFile',
            { src_abs_path: srcPath, dest_rel_path: destRelPath },
            stringToMCPContentArray(resultText),
            toolCallId,
            session.id,
            undefined,
            session.assistant?.id,
            'ui',
          );

          // Submit messages atomically using injectMessages
          await injectMessages([toolCallMessage, toolResultMessage], true);
        }

        // Refresh directory after import
        await loadDirectory(rootPath);
      } catch (error) {
        logger.error('File import failed', error);
        const message =
          error instanceof Error ? error.message : 'Unknown error occurred';
        toast.error(t('agent.workspace.importFileError'), {
          description: message,
        });
      }
    },
    [agentCallBuiltinTool, submit, session, rootPath, loadDirectory],
  );

  // Subscribe to DnD events
  useEffect(() => {
    logger.debug('Setting up DnD subscription for AgentWorkspacePanel');

    const handler = (event: DragAndDropEvent, payload: DragAndDropPayload) => {
      logger.debug('DnD event received in AgentWorkspacePanel', {
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
      logger.debug('Cleaning up DnD subscription for AgentWorkspacePanel');
      unsub();
    };
  }, [subscribe, handleWorkspaceFileDrop]);

  const handleSetOverride = async () => {
    if (!workspaceOverride.trim() || !session?.id || isSettingOverride) return;

    setIsSettingOverride(true);
    try {
      await setWorkspaceOverride(session.id, workspaceOverride);
      setIsOverrideActive(true);
      toast.success(t('agent.workspace.setOverrideSuccess'));
      loadDirectory('./');
    } catch (error) {
      logger.error('Failed to set workspace override', error);
      toast.error(t('agent.workspace.setOverrideError', { error }));
    } finally {
      setIsSettingOverride(false);
    }
  };

  const handleCancelOverride = async () => {
    if (!session?.id || isCancelingOverride) return;

    setIsCancelingOverride(true);
    try {
      await cancelWorkspaceOverride(session.id);
      setWorkspaceOverridePath('');
      setIsOverrideActive(false);
      toast.success(t('agent.workspace.cancelOverrideSuccess'));
      loadDirectory('./');
    } catch (error) {
      logger.error('Failed to cancel workspace override', error);
      toast.error(t('agent.workspace.cancelOverrideError', { error }));
    } finally {
      setIsCancelingOverride(false);
    }
  };

  const handleOpenInExplorer = async () => {
    if (!session?.id || isOpeningNative || openingNativeLock.current) return;
    openingNativeLock.current = true;
    setIsOpeningNative(true);
    try {
      await openWorkspaceInExplorer(session.id);
    } catch (error) {
      logger.error('Failed to open explorer', error);
      toast.error(t('agent.workspace.openExplorerError', { error }));
    } finally {
      setIsOpeningNative(false);
      openingNativeLock.current = false;
    }
  };

  const handleOpenInTerminal = async () => {
    if (!session?.id || isOpeningNative || openingNativeLock.current) return;
    openingNativeLock.current = true;
    setIsOpeningNative(true);
    try {
      await openWorkspaceInTerminal(session.id);
    } catch (error) {
      logger.error('Failed to open terminal', error);
      toast.error(t('agent.workspace.openTerminalError', { error }));
    } finally {
      setIsOpeningNative(false);
      openingNativeLock.current = false;
    }
  };

  const handleBrowseFolder = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('agent.workspace.selectDirectoryTitle'),
      });

      if (selected && typeof selected === 'string') {
        setWorkspaceOverridePath(selected);
      }
    } catch (error) {
      logger.error('Failed to open folder dialog', error);
      toast.error(t('agent.workspace.openFolderDialogError', { error }));
    }
  };

  const handleUploadClick = async () => {
    if (isUploading) return;
    setIsUploading(true);
    try {
      const selected = await open({
        multiple: true,
        title: t('agent.workspace.selectFilesTitle'),
      });

      if (selected) {
        const files = Array.isArray(selected) ? selected : [selected];
        // handleWorkspaceFileDrop expects string[]
        await handleWorkspaceFileDrop(files);
      }
    } catch (error) {
      logger.error('Failed to open file dialog', error);
      toast.error(t('agent.workspace.selectFilesError', { error }));
    } finally {
      setIsUploading(false);
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
        toast.success(t('agent.workspace.fileOpened'), {
          description: t('agent.workspace.fileOpenedDescription', { name: node.name }),
        });
      } catch (error) {
        logger.error('Failed to open file', { path: node.path, error });
        const message =
          error instanceof Error ? error.message : 'Unknown error occurred';
        toast.error(t('agent.workspace.fileOpenError'), {
          description: message,
        });
      }
    },
    [openWorkspaceFileWithDefaultApp, session?.id],
  );

  if (!session) return null;

  return (
    <div
      ref={panelRef}
      className={`w-80 h-full ${dragState.isOver ? 'ring-2 ring-success' : ''}`}
    >
      <Card
        className={`w-full h-full flex flex-col bg-background/95 backdrop-blur border-border/50 ${dragState.isOver ? 'border-success bg-success/10' : ''
          }`}
      >
        <CardHeader className="pb-3">
          <div className="flex items-center justify-between">
            <CardTitle className="text-sm font-medium flex items-center gap-2">
              <Folder className="w-4 h-4" />
              {t('agent.workspace.title')}
            </CardTitle>
            <div className="flex items-center gap-1">
              <Button
                variant="ghost"
                size="sm"
                onClick={handleOpenInExplorer}
                className="h-6 px-2 text-xs"
                title={t('agent.workspace.openInExplorer')}
                aria-label={t('agent.workspace.openInExplorerAria')}
                disabled={isOpeningNative}
              >
                <Folder className="w-3 h-3" />
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={handleOpenInTerminal}
                className="h-6 px-2 text-xs"
                title={t('agent.workspace.openInTerminal')}
                aria-label={t('agent.workspace.openInTerminalAria')}
                disabled={isOpeningNative}
              >
                <Terminal className="w-3 h-3" />
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => loadDirectory(rootPath)}
                className="h-6 w-6 p-0"
                title={t('agent.workspace.refresh')}
                aria-label={t('agent.workspace.refreshAria')}
              >
                <RefreshCw
                  className={`w-3 h-3 ${loading ? 'animate-spin' : ''}`}
                />
              </Button>
            </div>
          </div>

          {/* Workspace Override UI */}
          <div className="px-0 py-2 space-y-2 border-b border-border/50 mb-2">
            <div className="flex gap-2">
              <Input
                type="text"
                placeholder={t('agent.workspace.overridePlaceholder')}
                value={workspaceOverride}
                readOnly
                className="h-7 text-xs flex-1"
                disabled={isOverrideActive}
                aria-label={t('agent.workspace.overrideAria')}
              />
              {!isOverrideActive && (
                <Button
                  onClick={handleBrowseFolder}
                  size="sm"
                  variant="outline"
                  className="h-7 text-xs whitespace-nowrap"
                >
                  {t('agent.workspace.browse')}
                </Button>
              )}
              {!isOverrideActive ? (
                <Button
                  onClick={handleSetOverride}
                  size="sm"
                  className="h-7 text-xs"
                  disabled={!workspaceOverride.trim() || isSettingOverride}
                >
                  {isSettingOverride ? t('agent.workspace.setting') : t('agent.workspace.set')}
                </Button>
              ) : (
                <Button
                  onClick={handleCancelOverride}
                  size="sm"
                  variant="destructive"
                  className="h-7 text-xs"
                  disabled={isCancelingOverride}
                >
                  {isCancelingOverride ? t('agent.workspace.canceling') : t('agent.workspace.cancel')}
                </Button>
              )}
            </div>
            {isOverrideActive && (
              <p className="text-xs text-warning flex items-center gap-1">
                <AlertTriangle className="w-3 h-3" />
                {t('agent.workspace.usingCustom')}
              </p>
            )}
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
              <span className="text-xs text-muted-foreground">{t('agent.workspace.loading')}</span>
            </div>
          ) : (
            <div className="space-y-0">
              {fileTree.map((node) => (
                <FileTreeNode
                  key={node.id}
                  node={node}
                  onToggle={toggleDirectory}
                  onOpen={handleOpenFile}
                />
              ))}

              {fileTree.length === 0 && !loading && (
                <div className="text-xs text-muted-foreground text-center py-8">
                  {t('agent.workspace.noFilesFound')}
                </div>
              )}
            </div>
          )}
        </CardContent>

        <div
          role="button"
          tabIndex={0}
          className={`border-2 border-dashed border-muted-foreground/25 rounded m-2 p-2 text-center text-xs text-muted-foreground hover:border-muted-foreground/50 transition-colors cursor-pointer hover:bg-muted/50 ${isUploading ? 'opacity-50 pointer-events-none' : ''
            }`}
          onClick={handleUploadClick}
          onKeyDown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault();
              handleUploadClick();
            }
          }}
          aria-label={t('agent.workspace.uploadAria')}
          aria-disabled={isUploading}
        >
          {isUploading ? (
            <RefreshCw className="w-4 h-4 mx-auto mb-1 animate-spin" />
          ) : (
            <Upload className="w-4 h-4 mx-auto mb-1" />
          )}
          {isUploading ? t('agent.workspace.uploading') : t('agent.workspace.dropFiles')}
        </div>
      </Card>
    </div>
  );
}
