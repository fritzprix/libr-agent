import { useCallback, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { ExternalLink, Loader2 } from 'lucide-react';
import { toast } from 'sonner';
import { dismissSpotlight, loadSpotlightState, saveSpotlightState } from '../spotlight-state';
import { runSpotlightCta } from '../run-spotlight-cta';
import type { Spotlight } from '../types';

export interface WaitTipLinkProps {
  tip: Spotlight;
  className?: string;
}

export function WaitTipLink({ tip, className = '' }: WaitTipLinkProps) {
  const { t } = useTranslation('common');
  const navigate = useNavigate();
  const [pending, setPending] = useState(false);

  const label = t('spotlight.waitLine', {
    defaultValue: 'Tip · {{title}}',
    title: t(tip.titleKey),
  });

  const handleClick = useCallback(async () => {
    if (pending) {
      return;
    }
    setPending(true);
    try {
      const result = await runSpotlightCta(tip, { t });
      if (result.successMessage) {
        toast.success(result.successMessage);
      }
      if (result.dismissTipId) {
        const next = dismissSpotlight(
          loadSpotlightState(),
          result.dismissTipId,
        );
        saveSpotlightState(next);
      }
      navigate(result.path);
    } catch (error) {
      const message =
        error instanceof Error
          ? error.message
          : t('scheduledTasks.setupKnowledgeDistill.failed', {
              defaultValue: 'Could not create the scheduled task.',
            });
      toast.error(message);
    } finally {
      setPending(false);
    }
  }, [navigate, pending, t, tip]);

  return (
    <button
      type="button"
      data-testid="composer-busy-tip"
      className={`inline-flex max-w-full items-center gap-1 text-left text-xs text-muted-foreground underline-offset-2 hover:underline hover:text-foreground transition-colors animate-in fade-in duration-300 disabled:opacity-60 ${className}`}
      onClick={() => void handleClick()}
      disabled={pending}
      title={t('spotlight.waitOpenHint', {
        defaultValue: 'Open: {{detail}}',
        detail: t(tip.bodyKey),
      })}
    >
      <span className="truncate">{label}</span>
      {pending ? (
        <Loader2 className="h-3 w-3 shrink-0 animate-spin opacity-70" aria-hidden />
      ) : (
        <ExternalLink className="h-3 w-3 shrink-0 opacity-70" aria-hidden />
      )}
    </button>
  );
}
