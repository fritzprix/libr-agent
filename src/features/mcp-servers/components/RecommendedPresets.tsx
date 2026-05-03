import React from 'react';
import { Download, Server } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { MCPServerPreset } from '@/lib/backend/mcp-server-config';
import { MCPServerEntity } from '@/models/chat';
import { buttonVariants } from '@/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { cn } from '@/lib/utils';

interface RecommendedPresetsProps {
  presets: MCPServerPreset[] | undefined;
  allServers: MCPServerEntity[];
  onSetupPreset: (preset: MCPServerPreset) => void;
}

export const RecommendedPresets: React.FC<RecommendedPresetsProps> = ({
  presets,
  allServers,
  onSetupPreset,
}) => {
  const { t } = useTranslation('common');

  if (!presets || presets.length === 0) {
    return null;
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-2">
        <Server className="w-4 h-4 text-primary" />
        <h3 className="text-sm font-bold uppercase tracking-widest text-muted-foreground font-sans">
          {t('mcpServer.recommended', 'Recommended Extensions')}
        </h3>
        <div className="h-px bg-border/50 flex-1 ml-2" />
      </div>
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {presets.map((preset) => {
          const isInstalled = allServers.some((s) => s.name === preset.name);
          return (
            <div
              key={preset.name}
              className={cn(
                'group relative flex flex-col justify-between rounded-[1.5rem] border bg-background/50 backdrop-blur-sm p-5 transition-all duration-300 focus-visible:ring-2 focus-visible:ring-primary/20 outline-none',
                isInstalled
                  ? 'opacity-60 cursor-default bg-muted/20 border-border/50'
                  : 'hover:shadow-2xl hover:bg-background hover:-translate-y-1 cursor-pointer border-border/50 hover:border-primary/40',
              )}
              role={isInstalled ? undefined : 'button'}
              tabIndex={isInstalled ? -1 : 0}
              aria-disabled={isInstalled}
              aria-label={
                !isInstalled
                  ? t('mcpServer.installExtension', {
                      name: preset.name,
                      defaultValue: 'Install {{name}} extension',
                    })
                  : undefined
              }
              onClick={() => !isInstalled && onSetupPreset(preset)}
              onKeyDown={(event) => {
                if (isInstalled) return;
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  onSetupPreset(preset);
                }
              }}
            >
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <div className="w-6 h-6 rounded overflow-hidden flex-shrink-0 border border-border/50">
                      {preset.logo ? (
                        <img
                          src={preset.logo}
                          alt={preset.name}
                          className="w-full h-full object-contain"
                          onError={(e) => {
                            (
                              e.currentTarget as HTMLImageElement
                            ).style.display = 'none';
                            (
                              e.currentTarget
                                .nextElementSibling as HTMLElement | null
                            )?.classList.remove('hidden');
                          }}
                        />
                      ) : null}
                      <div
                        className={`w-full h-full bg-muted flex items-center justify-center ${preset.logo ? 'hidden' : ''}`}
                      >
                        <Server className="w-3 h-3 text-muted-foreground" />
                      </div>
                    </div>
                    <h4 className="font-semibold tracking-tight">
                      {preset.name}
                    </h4>
                  </div>
                  {isInstalled ? (
                    <span className="text-[10px] bg-primary/10 text-primary px-1.5 py-0.5 rounded font-medium flex items-center gap-1">
                      {t('mcpServer.installed', 'Installed')}
                    </span>
                  ) : (
                    <span className="text-[10px] bg-muted px-1.5 py-0.5 rounded text-muted-foreground uppercase">
                      {t('assistant.mcp.transport.stdio', 'stdio')}
                    </span>
                  )}
                </div>
                <p className="text-xs text-muted-foreground line-clamp-2">
                  {preset.description ||
                    t('mcpServer.noDescription', 'No description available')}
                </p>
              </div>
              {!isInstalled && (
                <div className="mt-3 pt-3 border-t border-border/50 flex items-center justify-between opacity-60 group-hover:opacity-100 group-focus-visible:opacity-100 group-focus-within:opacity-100 transition-opacity">
                  <code className="text-[10px] bg-muted px-1 py-0.5 rounded font-mono text-muted-foreground">
                    {preset.command} {preset.args?.[0]}
                  </code>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <div
                        className={cn(
                          buttonVariants({ variant: 'ghost', size: 'icon' }),
                          'h-6 w-6 rounded-full group-hover:bg-primary/10 group-hover:text-primary transition-colors',
                        )}
                        aria-hidden="true"
                      >
                        <Download className="w-3.5 h-3.5" />
                      </div>
                    </TooltipTrigger>
                    <TooltipContent>
                      {t('mcpServer.installExtension', {
                        name: preset.name,
                        defaultValue: 'Install {{name}} extension',
                      })}
                    </TooltipContent>
                  </Tooltip>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
};
