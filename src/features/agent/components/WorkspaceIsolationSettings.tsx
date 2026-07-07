import { useTranslation } from 'react-i18next';
import { Shield, HelpCircle } from 'lucide-react';

import { Input, Label } from '@/components/ui';
import { Switch } from '@/components/ui/switch';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { DOCKER_IMAGE_PRESETS } from '@/config/docker';

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
