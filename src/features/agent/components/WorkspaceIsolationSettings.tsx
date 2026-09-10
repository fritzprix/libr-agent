import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { AlertTriangle, HelpCircle, Shield } from 'lucide-react';

import { Input, Label } from '@/components/ui';
import { Switch } from '@/components/ui/switch';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { DOCKER_IMAGE_PRESETS } from '@/config/docker';
import { checkDockerHealth } from '@/lib/backend/dockerHealth';
import {
  classifyDockerAvailabilityError,
  type DockerAvailabilityIssue,
} from '@/lib/backend/errors';

interface WorkspaceIsolationSettingsProps {
  switchId: string;
  workspaceIsolation: 'host' | 'docker';
  setWorkspaceIsolation: (value: 'host' | 'docker') => void;
  dockerImage: string;
  setDockerImage: (value: string) => void;
  presetActiveClassName?: string;
}

export function WorkspaceIsolationSettings({
  switchId,
  workspaceIsolation,
  setWorkspaceIsolation,
  dockerImage,
  setDockerImage,
  presetActiveClassName = 'border-primary bg-primary/5 text-primary font-medium',
}: WorkspaceIsolationSettingsProps) {
  const { t } = useTranslation();
  const [dockerHealthIssue, setDockerHealthIssue] =
    useState<DockerAvailabilityIssue | null>(null);

  useEffect(() => {
    if (workspaceIsolation !== 'docker') {
      setDockerHealthIssue(null);
      return;
    }

    let cancelled = false;
    setDockerHealthIssue(null);

    void checkDockerHealth()
      .then(() => {
        if (!cancelled) {
          setDockerHealthIssue(null);
        }
      })
      .catch((error: unknown) => {
        if (cancelled) {
          return;
        }
        setDockerHealthIssue(
          classifyDockerAvailabilityError(error) ?? 'not-available',
        );
      });

    return () => {
      cancelled = true;
    };
  }, [workspaceIsolation]);

  const healthWarning =
    dockerHealthIssue === 'not-installed'
      ? t(
          'agent.workspace.dockerNotInstalledWarning',
          'Docker is not installed or is not available on PATH.',
        )
      : dockerHealthIssue === 'not-available'
        ? t(
            'agent.workspace.dockerDaemonOfflineWarning',
            'Docker daemon is not running.',
          )
        : null;

  return (
    <div className="space-y-3 text-left">
      <div className="flex items-center justify-between">
        <span className="text-[10px] font-bold uppercase tracking-wider text-foreground/80 flex items-center gap-1.5">
          <Shield className="h-3.5 w-3.5 text-primary/80" />
          {t('agent.workspace.isolationSettings', 'Isolation Settings')}
        </span>
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
          htmlFor={switchId}
          className="text-xs font-medium cursor-pointer text-muted-foreground"
        >
          {t('agent.workspace.useDockerContainer', 'Use Docker Container')}
        </Label>
        <Switch
          id={switchId}
          checked={workspaceIsolation === 'docker'}
          onCheckedChange={(checked) =>
            setWorkspaceIsolation(checked ? 'docker' : 'host')
          }
        />
      </div>

      {workspaceIsolation === 'docker' ? (
        <div className="space-y-2 pt-2 border-t border-border/20 animate-in fade-in slide-in-from-top-2 duration-200">
          {healthWarning ? (
            <div
              role="status"
              className="flex items-start gap-2 rounded-md border border-amber-500/30 bg-amber-500/10 p-2 text-amber-800 dark:text-amber-200"
            >
              <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
              <div className="min-w-0 space-y-1">
                <p className="text-[11px] font-medium leading-snug">
                  {healthWarning}
                </p>
                <p className="text-[10px] leading-snug text-amber-800/80 dark:text-amber-200/80">
                  {t(
                    'agent.workspace.dockerHealthHint',
                    'Install Docker Desktop, start the engine, or switch back to host mode.',
                  )}
                </p>
              </div>
            </div>
          ) : null}
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
            {DOCKER_IMAGE_PRESETS.map((preset) => (
              <button
                key={preset.val}
                onClick={() => setDockerImage(preset.val)}
                type="button"
                className={`text-[9px] px-2 py-0.5 rounded-full border transition-all ${
                  dockerImage === preset.val
                    ? presetActiveClassName
                    : 'border-border hover:bg-muted text-muted-foreground'
                }`}
              >
                {preset.label}
              </button>
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
}
