import { Lightbulb, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import type { Spotlight } from '../types';

export interface HubHintCardProps {
  tip: Spotlight;
  onCta: (tip: Spotlight) => void;
  onNext: () => void;
  onHide: () => void;
}

export function HubHintCard({ tip, onCta, onNext, onHide }: HubHintCardProps) {
  const { t } = useTranslation();

  return (
    <div
      data-testid="hub-hint-card"
      className="relative flex flex-col gap-4 rounded-xl border border-border/80 bg-card/60 p-5 text-card-foreground shadow-sm animate-in fade-in slide-in-from-top-2 duration-500 sm:flex-row sm:items-center sm:justify-between"
    >
      <Button
        type="button"
        variant="ghost"
        size="icon"
        className="absolute right-2 top-2 h-7 w-7 text-muted-foreground hover:text-foreground"
        aria-label={t('spotlight.hide', { defaultValue: 'Hide' })}
        onClick={onHide}
      >
        <X className="h-4 w-4" />
      </Button>

      <div className="flex min-w-0 items-start gap-3 pr-8">
        <Lightbulb className="mt-0.5 h-5 w-5 shrink-0 text-amber-600 dark:text-amber-400" />
        <div className="min-w-0 space-y-1">
          <p className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            {t('spotlight.hubLabel', { defaultValue: 'Tip' })}
          </p>
          <h3 className="text-sm font-semibold text-foreground">
            {t(tip.titleKey)}
          </h3>
          <p className="text-xs text-muted-foreground">{t(tip.bodyKey)}</p>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-2 self-end sm:self-center">
        <Button type="button" size="sm" onClick={() => onCta(tip)}>
          {t(tip.ctaLabelKey)}
        </Button>
        <Button type="button" size="sm" variant="outline" onClick={onNext}>
          {t('spotlight.nextTip', { defaultValue: 'Next tip' })}
        </Button>
      </div>
    </div>
  );
}
