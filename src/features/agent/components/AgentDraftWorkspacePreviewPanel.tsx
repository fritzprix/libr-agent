import { Button, Input, Label } from '@/components/ui';
import { Switch } from '@/components/ui/switch';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { useTranslation } from 'react-i18next';
import { FolderOpen, RefreshCw, X, Shield, HelpCircle } from 'lucide-react';

import { FileTreeNode } from './workspace-panel/FileTreeNode';
import { useDraftWorkspacePreviewTree } from './workspace-panel/useDraftWorkspacePreviewTree';

interface AgentDraftWorkspacePreviewPanelProps {
  workspacePath: string;
  workspaceIsolation: 'host' | 'docker';
  setWorkspaceIsolation: (val: 'host' | 'docker') => void;
  dockerImage: string;
  setDockerImage: (val: string) => void;
  onClear: () => void;
}

export function AgentDraftWorkspacePreviewPanel({
  workspacePath,
  workspaceIsolation,
  setWorkspaceIsolation,
  dockerImage,
  setDockerImage,
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
            <Tooltip>
              <TooltipTrigger asChild>
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
              </TooltipTrigger>
              <TooltipContent>
                {t('agent.workspace.refreshAria')}
              </TooltipContent>
            </Tooltip>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-7 w-7 text-muted-foreground hover:text-foreground"
                  onClick={onClear}
                  aria-label={t('common:close', 'Close')}
                >
                  <X className="h-3.5 w-3.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>{t('common:close', 'Close')}</TooltipContent>
            </Tooltip>
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

          {/* Environment Isolation Settings */}
          <div className="rounded-lg border border-border/40 bg-muted/[0.08] p-3 space-y-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-1.5 text-[10px] font-bold uppercase tracking-wider text-foreground/80">
                <Shield className="h-3.5 w-3.5 text-primary/80" />
                <span>
                  {t('agent.workspace.isolationSettings', 'Isolation Settings')}
                </span>
              </div>
              <Tooltip>
                <TooltipTrigger asChild>
                  <div className="cursor-help text-muted-foreground/50 hover:text-muted-foreground">
                    <HelpCircle className="h-3.5 w-3.5" />
                  </div>
                </TooltipTrigger>
                <TooltipContent className="max-w-[220px] text-xs">
                  {t(
                    'agent.workspace.isolationTip',
                    'Docker workspace runs your workspace commands safely inside a container rather than directly on your host machine.',
                  )}
                </TooltipContent>
              </Tooltip>
            </div>

            <div className="flex items-center justify-between pt-1">
              <Label
                htmlFor="docker-isolation"
                className="text-xs font-medium cursor-pointer text-muted-foreground"
              >
                {t(
                  'agent.workspace.useDockerContainer',
                  'Use Docker Container',
                )}
              </Label>
              <Switch
                id="docker-isolation"
                checked={workspaceIsolation === 'docker'}
                onCheckedChange={(checked) =>
                  setWorkspaceIsolation(checked ? 'docker' : 'host')
                }
              />
            </div>

            {workspaceIsolation === 'docker' && (
              <div className="space-y-2 pt-2 border-t border-border/20 animate-in fade-in slide-in-from-top-2 duration-200">
                <div className="space-y-1">
                  <Label className="text-[10px] font-bold text-muted-foreground uppercase">
                    {t('agent.workspace.dockerImage', 'Docker Image')}
                  </Label>
                  <Input
                    value={dockerImage}
                    onChange={(e) => setDockerImage(e.target.value)}
                    placeholder="e.g. python:3.11-slim"
                    className="h-8 text-xs font-mono bg-background"
                  />
                </div>
                <div className="flex flex-wrap gap-1 pt-1">
                  {[
                    { label: 'Python 3', val: 'python:3.11-slim' },
                    { label: 'Node 20', val: 'node:20-alpine' },
                    { label: 'Ubuntu', val: 'ubuntu:latest' },
                    { label: 'Go 1.22', val: 'golang:1.22-alpine' },
                  ].map((preset) => (
                    <button
                      key={preset.val}
                      onClick={() => setDockerImage(preset.val)}
                      type="button"
                      className={`text-[9px] px-2 py-0.5 rounded-full border transition-all ${
                        dockerImage === preset.val
                          ? 'border-primary bg-primary/10 text-primary font-medium'
                          : 'border-border bg-muted/40 hover:bg-muted text-muted-foreground'
                      }`}
                    >
                      {preset.label}
                    </button>
                  ))}
                </div>
              </div>
            )}
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
