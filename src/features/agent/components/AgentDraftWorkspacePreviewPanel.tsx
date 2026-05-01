import { Button } from '@/components/ui';
import { useTranslation } from 'react-i18next';
import { FolderOpen, RefreshCw, X } from 'lucide-react';

import { FileTreeNode } from './workspace-panel/FileTreeNode';
import { useDraftWorkspacePreviewTree } from './workspace-panel/useDraftWorkspacePreviewTree';

interface AgentDraftWorkspacePreviewPanelProps {
  workspacePath: string;
  onClear: () => void;
}

export function AgentDraftWorkspacePreviewPanel({
  workspacePath,
  onClear,
}: AgentDraftWorkspacePreviewPanelProps) {
  const { t } = useTranslation();
  const { fileTree, loading, error, refresh, toggleDirectory } =
    useDraftWorkspacePreviewTree(workspacePath);

  return (
    <div className="flex h-full w-80 flex-shrink-0 animate-in slide-in-from-left duration-300">
      <div className="flex h-full w-full flex-col border-r border-border/40 bg-background">
        <div className="flex items-center justify-between border-b border-border/40 px-4 py-3">
          <div className="flex items-center gap-2 text-[11px] uppercase tracking-[0.18em] text-muted-foreground">
            <FolderOpen className="h-3.5 w-3.5" />
            <span>{t('agent.workspace.title')}</span>
          </div>
          <div className="flex items-center gap-1">
            <Button
              variant="ghost"
              size="icon"
              className="h-7 w-7 text-muted-foreground hover:text-foreground"
              onClick={() => void refresh()}
              aria-label={t('agent.workspace.refreshAria')}
            >
              <RefreshCw
                className={`h-3.5 w-3.5 ${loading ? 'animate-spin' : ''}`}
              />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="h-7 w-7 text-muted-foreground hover:text-foreground"
              onClick={onClear}
              aria-label={t('common:close', 'Close')}
            >
              <X className="h-3.5 w-3.5" />
            </Button>
          </div>
        </div>

        <div className="flex-1 space-y-4 overflow-auto p-4">
          <div>
            <div className="mb-2 text-[10px] font-bold uppercase tracking-wider text-primary">
              {t(
                'agent.draft.workspaceOverrideActive',
                'Workspace Override Active',
              )}
            </div>
            <div className="rounded-md border border-border/50 bg-muted/50 p-2 font-mono text-[10px] break-all">
              {workspacePath}
            </div>
          </div>

          <div className="rounded-lg border border-border/40 bg-muted/[0.18] p-3 text-xs text-muted-foreground">
            {t(
              'agent.draft.workspacePreviewReadOnly',
              'Read-only preview before session start',
            )}
          </div>

          {error ? (
            <div className="rounded-md border border-destructive/30 bg-destructive/10 p-3 text-xs text-destructive">
              {error}
            </div>
          ) : loading && fileTree.length === 0 ? (
            <div className="flex items-center justify-center rounded-lg border border-border/40 bg-muted/[0.18] py-8">
              <RefreshCw className="mr-2 h-4 w-4 animate-spin" />
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
                />
              ))}

              {fileTree.length === 0 && !loading && (
                <div className="py-8 text-center text-xs text-muted-foreground">
                  {t('agent.workspace.noFilesFound')}
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
