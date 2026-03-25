import { useState, useCallback, useEffect, useRef } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import {
  Folder,
  RefreshCw,
  Upload,
  Terminal,
  AlertTriangle,
  Loader2,
} from 'lucide-react';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { toast } from 'sonner';
import { getLogger } from '@/lib/logger';
import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/input';
import { open } from '@tauri-apps/plugin-dialog';
import {
  openWorkspaceInExplorer,
  openWorkspaceInTerminal,
} from '@/lib/backend';
import {
  useDnDContext,
  type DragAndDropEvent,
  type DragAndDropPayload,
} from '@/context/DnDContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';

import { FileTreeNode } from './workspace-panel/FileTreeNode';
import { useWorkspaceFiles } from './workspace-panel/useWorkspaceFiles';
import { useWorkspaceOverride } from './workspace-panel/useWorkspaceOverride';
import { useWorkspaceFileDrop } from './workspace-panel/useWorkspaceFileDrop';
import type { FileNode } from './workspace-panel/types';

const logger = getLogger('AgentWorkspacePanel');

export function AgentWorkspacePanel() {
  const { t } = useTranslation();
  const { openWorkspaceFileWithDefaultApp } = useRustBackend();
  const { session } = useAgentSessionState();

  const [rootPath] = useState<string>('./');
  const panelRef = useRef<HTMLDivElement>(null);
  const { subscribe } = useDnDContext();
  const [dragState, setDragState] = useState<{ isOver: boolean }>({
    isOver: false,
  });

  const [isUploading, setIsUploading] = useState(false);
  const [isOpeningNative, setIsOpeningNative] = useState(false);
  const openingNativeLock = useRef(false);

  // Extracted hooks
  const { fileTree, loading, error, loadDirectory, toggleDirectory } =
    useWorkspaceFiles(rootPath);

  const handleOverrideChanged = useCallback(() => {
    loadDirectory(rootPath);
  }, [loadDirectory, rootPath]);

  const {
    workspaceOverride,
    isOverrideActive,
    isSettingOverride,
    isCancelingOverride,
    isBrowsing,
    handleSetOverride,
    handleCancelOverride,
    handleBrowseFolder,
  } = useWorkspaceOverride(handleOverrideChanged);

  const handleDropComplete = useCallback(() => {
    loadDirectory(rootPath);
  }, [loadDirectory, rootPath]);

  const { handleWorkspaceFileDrop } = useWorkspaceFileDrop(
    rootPath,
    handleDropComplete,
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
          description: t('agent.workspace.fileOpenedDescription', {
            name: node.name,
          }),
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
    [openWorkspaceFileWithDefaultApp, session?.id, t],
  );

  if (!session) return null;

  return (
    <div
      ref={panelRef}
      className={`w-80 h-full ${dragState.isOver ? 'ring-2 ring-success' : ''}`}
    >
      <Card
        className={`w-full h-full flex flex-col bg-background/95 backdrop-blur border-border/50 ${
          dragState.isOver ? 'border-success bg-success/10' : ''
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
                  disabled={isBrowsing}
                >
                  {isBrowsing ? (
                    <Loader2 className="w-3 h-3 animate-spin mr-1" />
                  ) : null}
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
                  {isSettingOverride
                    ? t('agent.workspace.setting')
                    : t('agent.workspace.set')}
                </Button>
              ) : (
                <Button
                  onClick={handleCancelOverride}
                  size="sm"
                  variant="destructive"
                  className="h-7 text-xs"
                  disabled={isCancelingOverride}
                >
                  {isCancelingOverride
                    ? t('agent.workspace.canceling')
                    : t('agent.workspace.cancel')}
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
              <span className="text-xs text-muted-foreground">
                {t('agent.workspace.loading')}
              </span>
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
          className={`border-2 border-dashed border-muted-foreground/25 rounded m-2 p-2 text-center text-xs text-muted-foreground hover:border-muted-foreground/50 transition-colors cursor-pointer hover:bg-muted/50 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none ${
            isUploading ? 'opacity-50 pointer-events-none' : ''
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
          {isUploading
            ? t('agent.workspace.uploading')
            : t('agent.workspace.dropFiles')}
        </div>
      </Card>
    </div>
  );
}
