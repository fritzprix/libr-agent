import { DatabaseZap } from 'lucide-react';
import { useTranslation } from 'react-i18next';

/**
 * Visual divider rendered between the last compacted message and the first
 * remaining message. Communicates to the user that older context was summarized.
 */
export function CompactEventDivider() {
  const { t } = useTranslation();

  return (
    <div
      className="flex items-center gap-3 my-2 px-2"
      aria-label={t(
        'agent.compactDivider.ariaLabel',
        'Context compressed at this point',
      )}
    >
      <div className="flex-1 h-px bg-border" />
      <div className="flex items-center gap-1.5 text-xs text-muted-foreground bg-muted/60 border border-border rounded-full px-3 py-1 select-none whitespace-nowrap">
        <DatabaseZap size={11} className="shrink-0" />
        <span>
          {t('agent.compactDivider.label', 'Context compressed above')}
        </span>
      </div>
      <div className="flex-1 h-px bg-border" />
    </div>
  );
}
