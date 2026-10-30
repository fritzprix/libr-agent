import React, { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui';
import {
  KeyRound,
  RefreshCw,
  Trash2,
  ShieldCheck,
  AlertCircle,
} from 'lucide-react';
import {
  listSessionApiKeys,
  revokeSessionApiKey,
  type IssuedApiKeySummary,
} from '@/lib/backend/session-api-keys';
import { getLogger } from '@/lib/logger';

const logger = getLogger('SessionApiKeysSection');

function SessionApiKeysSectionComponent() {
  const { t } = useTranslation('common');
  const [keys, setKeys] = useState<IssuedApiKeySummary[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);

  const fetchKeys = useCallback(async () => {
    setIsLoading(true);
    setLoadError(null);
    try {
      const data = await listSessionApiKeys();
      setKeys(data);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      logger.error('Failed to list session API keys', { err });
      setLoadError(message);
      toast.error(
        t('settings.sessionApi.fetchError', 'Failed to load Session API keys'),
      );
    } finally {
      setIsLoading(false);
    }
  }, [t]);

  useEffect(() => {
    void fetchKeys();
  }, [fetchKeys]);

  const handleDelete = useCallback(
    async (id: string, name: string) => {
      setDeletingId(id);
      try {
        await revokeSessionApiKey(id);
        toast.success(
          t(
            'settings.sessionApi.deleteSuccess',
            'Deleted API key for {{name}}',
            { name },
          ),
        );
        await fetchKeys();
      } catch (err) {
        logger.error('Failed to delete API key', { err, id });
        toast.error(
          t('settings.sessionApi.deleteError', 'Failed to delete API key'),
        );
      } finally {
        setDeletingId(null);
      }
    },
    [fetchKeys, t],
  );

  return (
    <div className="space-y-4 rounded-xl border border-border/70 p-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <KeyRound className="h-4 w-4 text-foreground" />
          <h4 className="text-sm font-medium text-foreground">
            {t('settings.sessionApi.title', 'Session API Keys')}
          </h4>
        </div>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={() => void fetchKeys()}
          disabled={isLoading}
          className="h-8 w-8 p-0"
          aria-label={t('common.refresh', 'Refresh')}
        >
          <RefreshCw
            className={`h-3.5 w-3.5 ${isLoading ? 'animate-spin' : ''}`}
          />
        </Button>
      </div>

      <p className="text-xs text-muted-foreground">
        {t(
          'settings.sessionApi.description',
          'API keys issued to external AI agents via pairing. When any key exists, requests without a valid key are rejected with 401. When no keys exist, localhost requests are allowed without a key.',
        )}
      </p>

      {loadError ? (
        <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-4 text-xs text-destructive space-y-2">
          <div className="flex items-center gap-2 font-medium">
            <AlertCircle className="h-4 w-4 shrink-0" />
            <span>
              {t(
                'settings.sessionApi.errorTitle',
                'Failed to load Session API keys',
              )}
            </span>
          </div>
          <p className="text-[11px] text-destructive/90 break-all font-mono">
            {loadError}
          </p>
          <p className="text-[11px] text-muted-foreground">
            {t(
              'settings.sessionApi.errorHelp',
              'The key file may be damaged or have incorrect permissions. Key verification is locked (fail-closed) until the issue is resolved.',
            )}
          </p>
        </div>
      ) : keys.length === 0 ? (
        <div className="rounded-lg border border-dashed border-border/70 p-6 text-center text-xs text-muted-foreground">
          {t(
            'settings.sessionApi.emptyList',
            'No API keys issued yet. Use "Integrate into Agent Session" in chat to pair an external agent.',
          )}
        </div>
      ) : (
        <div className="divide-y divide-border/50 rounded-lg border border-border/60 overflow-hidden">
          {keys.map((k) => {
            const isBusy = deletingId === k.id;
            return (
              <div
                key={k.id}
                className="flex items-center justify-between p-3 text-xs bg-card/40 hover:bg-card/70 transition-colors"
              >
                <div className="space-y-1 min-w-0 pr-4">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-foreground truncate">
                      {k.name}
                    </span>
                    <Badge
                      variant="outline"
                      className="border-emerald-500/30 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 text-[10px] gap-1 px-1.5 py-0"
                    >
                      <ShieldCheck className="h-3 w-3" />
                      {t('settings.sessionApi.active', 'Active')}
                    </Badge>
                  </div>
                  <div className="text-[11px] text-muted-foreground font-mono truncate">
                    {t('settings.sessionApi.created', 'Created')}:{' '}
                    {new Date(k.createdAt).toLocaleString()}
                  </div>
                </div>

                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  disabled={isBusy}
                  onClick={() => void handleDelete(k.id, k.name)}
                  className="h-7 text-xs text-destructive hover:bg-destructive/10 hover:text-destructive shrink-0 gap-1"
                >
                  <Trash2 className="h-3 w-3" />
                  {t('common.delete', 'Delete')}
                </Button>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

export const SessionApiKeysSection = React.memo(SessionApiKeysSectionComponent);
