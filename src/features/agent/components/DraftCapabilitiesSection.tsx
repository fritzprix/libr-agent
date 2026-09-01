import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Badge,
  Button,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui';
import { AgentModelPicker } from './AgentModelPicker';
import { useSettings } from '@/hooks/use-settings';
import type { ThinkingEffort } from '@/lib/ai-service/thinking-effort-mapping';
import type { Assistant } from '@/models/chat';
import {
  enforceRuntimeBuiltinAliases,
  OPTIONAL_BUILTIN_SERVICE_ALIASES,
} from '@/lib/assistant/runtime-builtins';
import {
  Bot,
  Brain,
  Database,
  FolderOpen,
  Globe,
  Puzzle,
  Square,
} from 'lucide-react';
import type {
  BuiltinServerInfo,
  MCPServerDto,
} from '../hooks/useAgentDraftChat';
import { ChevronDown, ChevronUp, Shield } from 'lucide-react';

import { WorkspaceIsolationSettings } from './WorkspaceIsolationSettings';

interface DraftCapabilitiesSectionProps {
  assistant: Assistant;
  builtinServices: BuiltinServerInfo[];
  mcpServers: MCPServerDto[];
  currentModel: string;
  currentProvider: string;
  onConfigUpdate: (model: string, provider: string) => void;
  workspaceIsolation: 'host' | 'docker';
  setWorkspaceIsolation: (value: 'host' | 'docker') => void;
  dockerImage: string;
  setDockerImage: (value: string) => void;
  workspaceOverride: string | null;
  onAddTools: () => void;
}

function getIconForService(iconId?: string) {
  switch (iconId) {
    case 'globe':
      return Globe;
    case 'database':
      return Database;
    case 'brain':
      return Brain;
    case 'folder-open':
      return FolderOpen;
    case 'layout':
      return Square;
    case 'server':
      return Puzzle;
    case 'book':
      return Brain;
    case 'bot':
      return Bot;
    default:
      return Square;
  }
}

export function DraftCapabilitiesSection({
  assistant,
  builtinServices,
  mcpServers,
  currentModel,
  currentProvider,
  onConfigUpdate,
  workspaceIsolation,
  setWorkspaceIsolation,
  dockerImage,
  setDockerImage,
  workspaceOverride,
  onAddTools,
}: DraftCapabilitiesSectionProps) {
  const { t } = useTranslation();
  const [showAdvanced, setShowAdvanced] = useState(false);
  const { value: settings, update: updateSettings } = useSettings();

  const handleThinkingEffortChange = useCallback(
    async (effort: ThinkingEffort) => {
      await updateSettings({
        advanced: {
          ...settings.advanced,
          thinkingEffort: effort,
        },
      });
    },
    [settings.advanced, updateSettings],
  );

  const effectiveBuiltinAliases = enforceRuntimeBuiltinAliases(
    assistant.allowedBuiltInServiceAliases,
  );
  const enabledOptionalAliases = effectiveBuiltinAliases.filter((alias) =>
    OPTIONAL_BUILTIN_SERVICE_ALIASES.includes(
      alias as (typeof OPTIONAL_BUILTIN_SERVICE_ALIASES)[number],
    ),
  );

  return (
    <>
      <div className="flex flex-wrap gap-2 justify-center max-w-2xl animate-in fade-in slide-in-from-bottom-4 duration-700 delay-150">
        <Tooltip delayDuration={300}>
          <TooltipTrigger asChild>
            <div className="cursor-help">
              <Badge
                variant="secondary"
                className="flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-medium font-sans bg-muted/40 hover:bg-muted transition-colors border border-transparent hover:border-border/50"
              >
                <Square size={12} className="text-primary/70" />
                {t('agent.draft.basicTools', 'Basic Tools')}
              </Badge>
            </div>
          </TooltipTrigger>
          <TooltipContent className="max-w-[250px] text-center mb-1 bg-popover text-popover-foreground shadow-xl border">
            <p className="text-xs">
              {t(
                'agent.draft.basicToolsDescription',
                'Includes core capabilities like reading files, managing tasks, and executing code. Always available to help you!',
              )}
            </p>
          </TooltipContent>
        </Tooltip>

        {enabledOptionalAliases.map((alias) => {
          const info = builtinServices.find(
            (service) => service.name === alias,
          );
          const label = info?.metadata.displayName || alias;
          const Icon = getIconForService(info?.metadata.icon);

          return (
            <Tooltip key={alias} delayDuration={300}>
              <TooltipTrigger asChild>
                <div className="cursor-help">
                  <Badge
                    variant="secondary"
                    className="flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-medium font-sans bg-muted/40 hover:bg-muted transition-colors border border-transparent hover:border-border/50"
                  >
                    <Icon size={12} className="text-primary/70" />
                    {label}
                  </Badge>
                </div>
              </TooltipTrigger>
              {info?.metadata.description && (
                <TooltipContent className="max-w-[250px] text-center mb-1 bg-popover text-popover-foreground shadow-xl border">
                  <p className="text-xs">{info.metadata.description}</p>
                </TooltipContent>
              )}
            </Tooltip>
          );
        })}

        {assistant.mcpServerIds?.map((serverId) => {
          const serverConfig = mcpServers.find(
            (server) => server.id === serverId,
          );
          const label = serverConfig?.name || serverId;

          return (
            <Badge
              key={serverId}
              variant="outline"
              className="flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-medium font-sans border-dashed border-primary/30 text-primary/80 bg-primary/5"
            >
              <Puzzle size={12} />
              {label}
            </Badge>
          );
        })}

        <Tooltip delayDuration={300}>
          <TooltipTrigger asChild>
            <button type="button" onClick={onAddTools} className="inline-flex">
              <Badge
                variant="outline"
                className="text-[11px] text-muted-foreground/60 border-dashed font-sans font-normal cursor-pointer hover:opacity-100 hover:bg-muted hover:text-foreground transition-all px-3 py-1.5"
              >
                {t('agent.draft.addTools', '+ Add tools')}
              </Badge>
            </button>
          </TooltipTrigger>
          <TooltipContent className="mb-1 bg-popover text-popover-foreground border shadow-xl">
            <p className="text-xs">
              {t(
                'agent.draft.addMoreCapabilities',
                'Configure tools for this assistant',
              )}
            </p>
          </TooltipContent>
        </Tooltip>
      </div>

      <div className="flex flex-col items-center gap-5 mt-4 pt-8 border-t border-border/40 w-full max-w-md animate-in fade-in duration-1000">
        <AgentModelPicker
          currentModel={currentModel}
          currentProvider={currentProvider}
          onConfigUpdate={onConfigUpdate}
          className="w-full max-w-xs shadow-sm"
          showThinkingEffort
          thinkingEffort={settings.advanced.thinkingEffort}
          onThinkingEffortChange={(effort) => {
            void handleThinkingEffortChange(effort);
          }}
        />

        {/* Collapsible Advanced Configuration Section */}
        {!workspaceOverride && (
          <div className="w-full max-w-xs space-y-2">
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => setShowAdvanced(!showAdvanced)}
              className="w-full flex items-center justify-between px-3 py-1.5 h-8 text-[11px] font-semibold text-muted-foreground/80 hover:text-foreground hover:bg-muted/50 rounded-md transition-all"
            >
              <span className="flex items-center gap-1.5">
                <Shield
                  size={12}
                  className={
                    workspaceIsolation === 'docker'
                      ? 'text-primary animate-pulse'
                      : 'text-muted-foreground/60'
                  }
                />
                {t(
                  'agent.workspace.advancedConfiguration',
                  'Advanced Configuration',
                )}
              </span>
              {showAdvanced ? (
                <ChevronUp size={12} />
              ) : (
                <ChevronDown size={12} />
              )}
            </Button>

            {showAdvanced && (
              <div className="rounded-lg border border-border/40 bg-muted/[0.04] p-3 animate-in fade-in slide-in-from-top-2 duration-200">
                <WorkspaceIsolationSettings
                  switchId="docker-isolation-draft"
                  workspaceIsolation={workspaceIsolation}
                  setWorkspaceIsolation={setWorkspaceIsolation}
                  dockerImage={dockerImage}
                  setDockerImage={setDockerImage}
                />
              </div>
            )}
          </div>
        )}

        <div className="flex items-center gap-3 text-[10px] uppercase tracking-[0.2em] text-muted-foreground/30 font-bold font-sans">
          <div className="h-px w-6 bg-border/40" />
          <Bot size={14} className="opacity-50" />
          {t('agent.draft.localContext', 'Local Context')}
          <div className="h-px w-6 bg-border/40" />
        </div>
      </div>
    </>
  );
}
