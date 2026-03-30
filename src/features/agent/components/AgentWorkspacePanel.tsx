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
      className={`h-full w-80 flex-shrink-0 ${dragState.isOver ? 'ring-2 ring-inset ring-success' : ''}`}
    >
      <Card
        className={`h-full w-full rounded-none border-y-0 border-l-0 border-r border-border/40 bg-background py-0 shadow-none gap-0 ${
          dragState.isOver ? 'border-success bg-success/5' : ''
        }`}
      >
        <CardHeader className="border-b border-border/40 px-4 py-3">
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2 text-[11px] uppercase tracking-[0.18em] text-muted-foreground">
                <Folder className="h-3.5 w-3.5" />
                <span>{t('agent.workspace.title')}</span>
              </div>
              <div className="flex items-center gap-1">
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={handleOpenInExplorer}
                  className="h-7 w-7 p-0 text-muted-foreground hover:text-foreground"
                  title={t('agent.workspace.openInExplorer')}
                  aria-label={t('agent.workspace.openInExplorerAria')}
                  disabled={isOpeningNative}
                >
                  <Folder className="w-3.5 h-3.5" />
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={handleOpenInTerminal}
                  className="h-7 w-7 p-0 text-muted-foreground hover:text-foreground"
                  title={t('agent.workspace.openInTerminal')}
                  aria-label={t('agent.workspace.openInTerminalAria')}
                  disabled={isOpeningNative}
                >
                  <Terminal className="w-3.5 h-3.5" />
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => loadDirectory(rootPath)}
                  className="h-7 w-7 p-0 text-muted-foreground hover:text-foreground"
                  title={t('agent.workspace.refresh')}
                  aria-label={t('agent.workspace.refreshAria')}
                >
                  <RefreshCw
                    className={`w-3.5 h-3.5 ${loading ? 'animate-spin' : ''}`}
                  />
                </Button>
              </div>
            </div>

            <div className="space-y-1">
              <CardTitle
                className="truncate text-sm font-medium"
                title={rootPath}
              >
                {rootPath}
              </CardTitle>
              <p className="text-[11px] text-muted-foreground">
                {isOverrideActive
                  ? t('agent.workspace.usingCustom')
                  : t('agent.workspace.title')}
              </p>
            </div>

            <div className="space-y-2 rounded-lg border border-border/40 bg-muted/[0.18] p-2.5">
              <div className="flex gap-2">
                <Button
                  onClick={handleBrowseFolder}
                  size="sm"
                  variant="outline"
                  className="h-8 shrink-0 border-border/50 bg-background/80 text-xs"
                  disabled={isBrowsing || isOverrideActive}
                >
                  {isBrowsing ? (
                    <Loader2 className="mr-1 h-3 w-3 animate-spin" />
                  ) : null}
                  {t('agent.workspace.browse')}
                </Button>
                <Input
                  type="text"
                  placeholder={t('agent.workspace.overridePlaceholder')}
                  value={workspaceOverride}
                  readOnly
                  className="h-8 flex-1 border-0 bg-transparent px-0 text-xs shadow-none focus-visible:ring-0"
                  disabled={isOverrideActive}
                  aria-label={t('agent.workspace.overrideAria')}
                />
              </div>

              <div className="flex items-center justify-between gap-2">
                {isOverrideActive ? (
                  <p className="flex items-center gap-1 text-[11px] text-warning">
                    <AlertTriangle className="h-3 w-3" />
                    {t('agent.workspace.usingCustom')}
                  </p>
                ) : (
                  <p className="text-[11px] text-muted-foreground">
                    {t('agent.workspace.overridePlaceholder')}
                  </p>
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
            </div>
          </div>
        </CardHeader>

        <CardContent className="flex-1 overflow-auto px-4 py-4">
          {error && (
            <div className="rounded-md border border-destructive/30 bg-destructive/10 p-3 text-xs text-destructive">
              {error}
            </div>
          )}

          {loading && fileTree.length === 0 ? (
            <div className="flex items-center justify-center rounded-lg border border-border/40 bg-muted/[0.18] py-8">
              <RefreshCw className="w-4 h-4 animate-spin mr-2" />
              <span className="text-xs text-muted-foreground">
                {t('agent.workspace.loading')}
              </span>
            </div>
          ) : (
            <div className="overflow-hidden rounded-lg border border-border/40 bg-muted/[0.18]">
              {fileTree.map((node) => (
                <FileTreeNode
                  key={node.id}
                  node={node}
                  onToggle={toggleDirectory}
                  onOpen={handleOpenFile}
                />
              ))}

              {fileTree.length === 0 && !loading && (
                <div className="py-8 text-center text-xs text-muted-foreground">
                  {t('agent.workspace.noFilesFound')}
                </div>
              )}
            </div>
          )}
        </CardContent>

        <div className="border-t border-border/50 px-4 py-3">
          <div
            role="button"
            tabIndex={0}
            className={`rounded-lg border border-dashed border-border/50 bg-muted/[0.18] p-3 text-center text-xs text-muted-foreground transition-colors hover:border-foreground/20 hover:bg-muted/30 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring ${
              isUploading ? 'pointer-events-none opacity-50' : 'cursor-pointer'
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
              <RefreshCw className="mx-auto mb-1 h-4 w-4 animate-spin" />
            ) : (
              <Upload className="mx-auto mb-1 h-4 w-4" />
            )}
            {isUploading
              ? t('agent.workspace.uploading')
              : t('agent.workspace.dropFiles')}
          </div>
        </div>
      </Card>
    </div>
  );
}
