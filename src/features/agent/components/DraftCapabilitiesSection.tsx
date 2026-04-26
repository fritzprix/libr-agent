import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import {
  Badge,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui';
import { AgentModelPicker } from './AgentModelPicker';
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

interface DraftCapabilitiesSectionProps {
  assistant: Assistant;
  builtinServices: BuiltinServerInfo[];
  mcpServers: MCPServerDto[];
  currentModel: string;
  currentProvider: string;
  onConfigUpdate: (model: string, provider: string) => void;
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
}: DraftCapabilitiesSectionProps) {
  const { t } = useTranslation();

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

        <Link to="/assistants">
          <Tooltip delayDuration={300}>
            <TooltipTrigger asChild>
              <Badge
                variant="outline"
                className="text-[11px] text-muted-foreground/60 border-dashed font-sans font-normal cursor-pointer hover:opacity-100 hover:bg-muted hover:text-foreground transition-all px-3 py-1.5"
              >
                {t('agent.draft.addTools', '+ Add tools')}
              </Badge>
            </TooltipTrigger>
            <TooltipContent className="mb-1 bg-popover text-popover-foreground border shadow-xl">
              <p className="text-xs">
                {t(
                  'agent.draft.addMoreCapabilities',
                  'Add more capabilities in the settings',
                )}
              </p>
            </TooltipContent>
          </Tooltip>
        </Link>
      </div>

      <div className="flex flex-col items-center gap-5 mt-4 pt-8 border-t border-border/40 w-full max-w-md animate-in fade-in duration-1000">
        <AgentModelPicker
          currentModel={currentModel}
          currentProvider={currentProvider}
          onConfigUpdate={onConfigUpdate}
          className="w-full max-w-xs shadow-sm"
        />
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
