import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronDown, Loader2, RefreshCw, ToggleLeft, ToggleRight, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui';
import { cn } from '@/lib/utils';
import type { PluginSourceOverview } from '@/types/skills';

interface PluginSourcesListProps {
  sources: PluginSourceOverview[];
  isBusy: boolean;
  removingId: string | null;
  onUpdate: (sourceUrl: string) => void | Promise<void>;
  onToggleEnabled: (sourceId: string, enabled: boolean) => void | Promise<void>;
  onRemove: (sourceId: string) => void | Promise<void>;
}

const EXECUTED_HOOK_EVENTS = new Set([
  'SessionStart',
  'SessionEnd',
  'UserPromptSubmit',
  'PreToolUse',
  'PostToolUse',
  'Stop',
  'SubagentStop',
  'PreCompact',
  'Notification',
]);

export function PluginSourcesList({
  sources,
  isBusy,
  removingId,
  onUpdate,
  onToggleEnabled,
  onRemove,
}: PluginSourcesListProps) {
  const { t } = useTranslation('common');
  const [expandedId, setExpandedId] = useState<string | null>(null);

  if (sources.length === 0) {
    return null;
  }

  return (
    <div className="rounded-lg border bg-muted/20 p-3 space-y-2">
      <p className="text-sm font-medium">
        {t('settings.skills.pluginSourcesTitle', 'Installed plugin sources')}
      </p>
      <div className="space-y-2">
        {sources.map((source) => {
          const expanded = expandedId === source.id;
          const canUpdate = source.sourceUrl.startsWith('https://github.com');
          return (
            <div key={source.id} className="rounded-md border bg-background p-3">
              <div className="flex flex-wrap items-start justify-between gap-2">
                <div className="min-w-0 space-y-1">
                  <div className="flex min-w-0 flex-wrap items-center gap-2">
                    <p className="text-sm font-medium break-all">
                      {source.pluginName ?? source.id}
                    </p>
                    {source.pluginVersion ? (
                      <span className="text-xs text-muted-foreground">
                        v{source.pluginVersion}
                      </span>
                    ) : source.installedCommit ? (
                      <span className="text-xs text-muted-foreground">
                        {source.installedCommit.slice(0, 7)}
                      </span>
                    ) : null}
                    {source.enabled ? null : (
                      <span className="rounded bg-muted px-1.5 py-0.5 text-xs text-muted-foreground">
                        {t('settings.skills.pluginDisabledBadge', 'disabled')}
                      </span>
                    )}
                  </div>
                  {source.pluginDescription ? (
                    <p className="text-xs text-muted-foreground">
                      {source.pluginDescription}
                    </p>
                  ) : null}
                  <p className="text-xs text-muted-foreground break-all">
                    {source.sourceUrl}
                  </p>
                  <p className="text-xs text-muted-foreground">
                    {source.hooksApproved
                      ? t(
                          'settings.skills.hooksApproved',
                          'Hooks approved and may run',
                        )
                      : t(
                          'settings.skills.hooksNotApproved',
                          'Hooks installed but not approved to run',
                        )}
                  </p>
                </div>
                <div className="flex items-center gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={isBusy || removingId === source.id}
                    aria-pressed={source.enabled}
                    aria-label={
                      source.enabled
                        ? t('settings.skills.disablePlugin', 'Disable plugin')
                        : t('settings.skills.enablePlugin', 'Enable plugin')
                    }
                    title={
                      source.enabled
                        ? t('settings.skills.disablePlugin', 'Disable plugin')
                        : t('settings.skills.enablePlugin', 'Enable plugin')
                    }
                    onClick={() =>
                      void onToggleEnabled(source.id, !source.enabled)
                    }
                  >
                    {source.enabled ? (
                      <ToggleRight className="w-4 h-4" />
                    ) : (
                      <ToggleLeft className="w-4 h-4" />
                    )}
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={isBusy}
                    aria-label={t('settings.skills.pluginDetails', 'Details')}
                    title={t('settings.skills.pluginDetails', 'Details')}
                    onClick={() =>
                      setExpandedId(expanded ? null : source.id)
                    }
                  >
                    <ChevronDown
                      className={cn(
                        'w-4 h-4 transition-transform',
                        expanded && 'rotate-180',
                      )}
                    />
                  </Button>
                  {canUpdate ? (
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      disabled={isBusy}
                      onClick={() => void onUpdate(source.sourceUrl)}
                    >
                      <RefreshCw className="w-4 h-4" />
                    </Button>
                  ) : null}
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={isBusy || removingId === source.id}
                    onClick={() => void onRemove(source.id)}
                  >
                    {removingId === source.id ? (
                      <Loader2 className="w-4 h-4 animate-spin" />
                    ) : (
                      <Trash2 className="w-4 h-4" />
                    )}
                  </Button>
                </div>
              </div>
              {expanded ? (
                <div className="mt-3 space-y-2 text-sm">
                  <div>
                    <p className="font-medium">
                      {t('settings.skills.skillsToInstall', 'Skills')}
                    </p>
                    <ul className="list-disc pl-5 text-muted-foreground">
                      {source.skillNames.map((name) => (
                        <li key={name}>{name}</li>
                      ))}
                    </ul>
                  </div>
                  {source.hookEvents.length > 0 ? (
                    <div>
                      <p className="font-medium">
                        {t('settings.skills.hookEvents', 'Hook events')}
                      </p>
                      <ul className="list-disc pl-5 text-muted-foreground">
                        {source.hookEvents.map((event) => (
                          <li key={event}>
                            {event}
                            <span className="ml-2 text-xs">
                              ({EXECUTED_HOOK_EVENTS.has(event)
                                ? t(
                                    'settings.skills.hookStatusExecuted',
                                    'Executed',
                                  )
                                : t(
                                    'settings.skills.hookStatusStubbed',
                                    'Not supported',
                                  )})
                            </span>
                          </li>
                        ))}
                      </ul>
                    </div>
                  ) : null}
                </div>
              ) : null}
            </div>
          );
        })}
      </div>
    </div>
  );
}