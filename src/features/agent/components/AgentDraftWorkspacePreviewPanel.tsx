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
    <div className="w-80 h-full border-r bg-background/95 backdrop-blur flex flex-col animate-in slide-in-from-left duration-300">
      <div className="px-4 py-3 border-b flex items-center justify-between">
        <div className="flex items-center gap-2 text-sm font-medium">
          <FolderOpen className="w-4 h-4 text-primary" />
          <span>{t('agent.workspace.title')}</span>
        </div>
        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6 text-muted-foreground hover:text-foreground"
            onClick={() => void refresh()}
            aria-label={t('agent.workspace.refreshAria')}
          >
            <RefreshCw className={`w-3 h-3 ${loading ? 'animate-spin' : ''}`} />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6 text-muted-foreground hover:text-foreground"
            onClick={onClear}
            aria-label={t('common:close', 'Close')}
          >
            <X className="w-3 h-3" />
          </Button>
        </div>
      </div>

      <div className="p-4 flex-1 overflow-auto space-y-4">
        <div>
          <div className="text-[10px] text-primary font-bold uppercase tracking-wider mb-2">
            {t(
              'agent.draft.workspaceOverrideActive',
              'Workspace Override Active',
            )}
          </div>
          <div className="bg-muted/50 p-2 rounded-md font-mono text-[10px] break-all border border-border/50">
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
  );
}
