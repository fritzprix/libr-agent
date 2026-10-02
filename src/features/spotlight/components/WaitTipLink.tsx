import { useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { ExternalLink } from 'lucide-react';
import { hrefToPath } from '../spotlight-state';
import type { Spotlight } from '../types';

export interface WaitTipLinkProps {
  tip: Spotlight;
  className?: string;
}

export function WaitTipLink({ tip, className = '' }: WaitTipLinkProps) {
  const { t } = useTranslation('common');
  const navigate = useNavigate();

  const label = t('spotlight.waitLine', {
    defaultValue: 'Tip · {{title}}',
    title: t(tip.titleKey),
  });

  const handleClick = useCallback(() => {
    navigate(hrefToPath(tip.href));
  }, [navigate, tip.href]);

  return (
    <button
      type="button"
      data-testid="composer-busy-tip"
      className={`inline-flex max-w-full items-center gap-1 text-left text-xs text-muted-foreground underline-offset-2 hover:underline hover:text-foreground transition-colors animate-in fade-in duration-300 ${className}`}
      onClick={handleClick}
      title={t('spotlight.waitOpenHint', {
        defaultValue: 'Open: {{detail}}',
        detail: t(tip.bodyKey),
      })}
    >
      <span className="truncate">{label}</span>
      <ExternalLink className="h-3 w-3 shrink-0 opacity-70" aria-hidden />
    </button>
  );
}
