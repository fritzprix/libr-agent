import { Puzzle } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Badge } from '@/components/ui/badge';
import { useSessionPlugins } from '@/features/agent/hooks/useSessionPlugins';
import type { SessionPluginEntry } from '@/types/skills';

const timeFormatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: 'medium',
  timeStyle: 'short',
});

function formatTime(iso: string): string {
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? iso : timeFormatter.format(date);
}

function PluginRow({ plugin }: { plugin: SessionPluginEntry }) {
  const { t } = useTranslation();
  return (
    <li className="min-w-0 space-y-2 rounded-xl border border-border/40 bg-muted/[0.16] px-4 py-3">
      <div className="flex min-w-0 items-start justify-between gap-3">
        <span
          className="min-w-0 break-all font-mono text-sm text-foreground"
          title={plugin.sourceUrl}
        >
          {plugin.sourceUrl}
        </span>
        <Badge
          variant={plugin.scope === 'session' ? 'default' : 'outline'}
          className="shrink-0 text-xs uppercase tracking-wide"
        >
          {plugin.scope === 'session'
            ? t('agent.toolsModal.plugins.scopeSession')
            : t('agent.toolsModal.plugins.scopeGlobal')}
        </Badge>
      </div>
      <p className="text-xs text-muted-foreground">
        {t('agent.toolsModal.plugins.installedAt', {
          time: formatTime(plugin.installedAt),
        })}
      </p>
      {plugin.skillNames.length > 0 ? (
        <p className="break-words text-xs text-muted-foreground">
          <span className="font-medium text-foreground">
            {t('agent.toolsModal.plugins.skills')}:
          </span>{' '}
          {plugin.skillNames.join(', ')}
        </p>
      ) : null}
      {plugin.hookEvents.length > 0 ? (
        <p className="break-words text-xs text-muted-foreground">
          <span className="font-medium text-foreground">
            {t('agent.toolsModal.plugins.hooks')}:
          </span>{' '}
          {plugin.hookEvents.join(', ')}
          {plugin.hooksApproved
            ? ''
            : ` (${t('agent.toolsModal.plugins.hooksNotApproved')})`}
        </p>
      ) : null}
    </li>
  );
}

/** Active plugins (global + session scope) and the session install history. */
export function SessionPluginsSection({
  sessionId,
}: {
  sessionId: string | undefined;
}) {
  const { t } = useTranslation();
  const { overview } = useSessionPlugins(sessionId);
  const active = overview?.active ?? [];
  const history = overview?.history ?? [];

  return (
    <section
      className="min-w-0 space-y-3 pb-5"
      aria-label={t('agent.toolsModal.plugins.title')}
    >
      <h3 className="flex items-center gap-2 text-sm font-semibold text-foreground">
        <Puzzle className="h-4 w-4 text-muted-foreground" />
        {t('agent.toolsModal.plugins.title')}
        {active.length > 0 ? (
          <span className="text-sm font-normal text-muted-foreground">
            {active.length}
          </span>
        ) : null}
      </h3>
      {active.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          {t('agent.toolsModal.plugins.empty')}
        </p>
      ) : (
        <ul className="min-w-0 space-y-2">
          {active.map((plugin) => (
            <PluginRow key={`${plugin.scope}:${plugin.id}`} plugin={plugin} />
          ))}
        </ul>
      )}
      {history.length > 0 ? (
        <details className="min-w-0 rounded-lg border border-border/35 bg-background/70 px-3 py-2">
          <summary className="cursor-pointer select-none text-xs font-medium text-muted-foreground">
            {t('agent.toolsModal.plugins.history')} ({history.length})
          </summary>
          <ul className="mt-2 space-y-1">
            {history.map((entry) => (
              <li
                key={`${entry.sourceId}:${entry.action}:${entry.at}`}
                className="break-all text-xs text-muted-foreground"
              >
                <span className="font-medium text-foreground">
                  {entry.action === 'install'
                    ? t('agent.toolsModal.plugins.actionInstall')
                    : t('agent.toolsModal.plugins.actionUninstall')}
                </span>{' '}
                {entry.sourceUrl} · {formatTime(entry.at)}
              </li>
            ))}
          </ul>
        </details>
      ) : null}
    </section>
  );
}
