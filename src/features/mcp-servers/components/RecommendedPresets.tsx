import React from 'react';
import { Download } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { MCPServerPreset } from '@/lib/backend/mcp-server-config';
import { MCPServerEntity } from '@/models/chat';
import { Button, Separator } from '@/components/ui';

interface RecommendedPresetsProps {
  presets: MCPServerPreset[] | undefined;
  servers: MCPServerEntity[];
  onSetupPreset: (preset: MCPServerPreset) => void;
}

export const RecommendedPresets: React.FC<RecommendedPresetsProps> = ({
  presets,
  servers,
  onSetupPreset,
}) => {
  const { t } = useTranslation('common');

  if (!presets || presets.length === 0) {
    return null;
  }

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2">
        <h3 className="text-sm font-medium text-muted-foreground">
          {t('mcpServer.recommended', 'Recommended Extensions')}
        </h3>
        <Separator className="flex-1" />
      </div>
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
        {presets.map((preset) => {
          const isInstalled = servers.some((s) => s.name === preset.name);
          return (
            <div
              key={preset.name}
              className={`group relative flex flex-col justify-between rounded-lg border bg-card p-4 transition-all focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] outline-none ${
                isInstalled
                  ? 'opacity-60 cursor-default bg-muted/20'
                  : 'hover:bg-accent/50 cursor-pointer'
              }`}
              role={isInstalled ? undefined : 'button'}
              tabIndex={isInstalled ? -1 : 0}
              aria-disabled={isInstalled}
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
                  <h4 className="font-semibold tracking-tight">
                    {preset.name}
                  </h4>
                  {isInstalled ? (
                    <span className="text-[10px] bg-primary/10 text-primary px-1.5 py-0.5 rounded font-medium flex items-center gap-1">
                      {t('mcpServer.installed', 'Installed')}
                    </span>
                  ) : (
                    <span className="text-[10px] bg-muted px-1.5 py-0.5 rounded text-muted-foreground uppercase">
                      stdio
                    </span>
                  )}
                </div>
                <p className="text-xs text-muted-foreground line-clamp-2">
                  {preset.description ||
                    t('mcpServer.noDescription', 'No description available')}
                </p>
              </div>
              {!isInstalled && (
                <div className="mt-3 pt-3 border-t border-border/50 flex items-center justify-between opacity-60 group-hover:opacity-100 transition-opacity">
                  <code className="text-[10px] bg-muted px-1 py-0.5 rounded font-mono text-muted-foreground">
                    {preset.command} {preset.args?.[0]}
                  </code>
                  <Button
                    type="button"
                    size="icon"
                    variant="ghost"
                    className="h-6 w-6 rounded-full hover:bg-primary/10 hover:text-primary"
                    aria-label={t('mcpServer.installExtension', {
                      name: preset.name,
                      defaultValue: 'Install {{name}} extension',
                    })}
                    onClick={(e) => {
                      e.stopPropagation();
                      onSetupPreset(preset);
                    }}
                  >
                    <Download className="w-3.5 h-3.5" />
                  </Button>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
};
