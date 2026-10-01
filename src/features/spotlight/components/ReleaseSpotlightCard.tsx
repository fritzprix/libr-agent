import { Lightbulb, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import type { Spotlight } from '../types';

export interface ReleaseSpotlightCardProps {
  appVersion: string;
  items: Spotlight[];
  onCta: (tip: Spotlight) => void;
  onDismissAll: () => void;
}

export function ReleaseSpotlightCard({
  appVersion,
  items,
  onCta,
  onDismissAll,
}: ReleaseSpotlightCardProps) {
  const { t } = useTranslation();

  if (items.length === 0) {
    return null;
  }

  return (
    <div
      data-testid="release-spotlight-card"
      className="relative rounded-xl border border-emerald-500/25 bg-emerald-500/5 p-5 text-card-foreground shadow-sm animate-in fade-in slide-in-from-top-2 duration-500 dark:border-emerald-400/25 dark:bg-emerald-400/10"
    >
      <Button
        type="button"
        variant="ghost"
        size="icon"
        className="absolute right-2 top-2 h-7 w-7 text-muted-foreground hover:text-foreground"
        aria-label={t('spotlight.dismissAll', { defaultValue: 'Dismiss' })}
        onClick={onDismissAll}
      >
        <X className="h-4 w-4" />
      </Button>

      <div className="flex items-start gap-3 pr-8">
        <Lightbulb className="mt-0.5 h-5 w-5 shrink-0 text-emerald-700 dark:text-emerald-400" />
        <div className="min-w-0 flex-1 space-y-3">
          <h3 className="text-sm font-semibold text-foreground">
            {t('spotlight.releaseTitle', {
              defaultValue: "What's new in v{{version}}",
              version: appVersion,
            })}
          </h3>
          <ul className="space-y-3 list-none">
            {items.map((tip) => (
              <li
                key={tip.id}
                className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between"
              >
                <div className="min-w-0 space-y-0.5">
                  <p className="text-sm font-medium text-foreground">
                    {t(tip.titleKey)}
                  </p>
                  <p className="text-xs text-muted-foreground">
                    {t(tip.bodyKey)}
                  </p>
                </div>
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  className="shrink-0 self-start sm:self-center"
                  onClick={() => onCta(tip)}
                >
                  {t(tip.ctaLabelKey)}
                </Button>
              </li>
            ))}
          </ul>
          <div className="pt-1">
            <Button
              type="button"
              size="sm"
              variant="ghost"
              onClick={onDismissAll}
            >
              {t('spotlight.dismissAll', { defaultValue: 'Dismiss' })}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
