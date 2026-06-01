import { useMemo, useState } from 'react';
import { ChevronDown, ChevronRight, DatabaseZap } from 'lucide-react';
import { useTranslation } from 'react-i18next';

/**
 * Compact event card rendered after the last compacted message. Communicates
 * what slice was summarized without exposing internal IDs.
 */
interface CompactEventDividerProps {
  latestIncludedPreview?: string;
  condensedCount?: number;
  summary?: string;
}

function previewLabel(preview: string | undefined, fallback: string): string {
  if (!preview) {
    return fallback;
  }

  return preview;
}

export function CompactEventDivider({
  latestIncludedPreview,
  condensedCount,
  summary,
}: CompactEventDividerProps) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const countLabel = useMemo(() => {
    if (!condensedCount || condensedCount < 2) {
      return null;
    }

    return t('agent.compactDivider.count', {
      count: condensedCount,
      defaultValue: `${condensedCount} messages condensed`,
    });
  }, [condensedCount, t]);

  return (
    <div
      className="my-2 rounded-lg border border-border bg-muted/30 px-3 py-2"
      aria-label={t(
        'agent.compactDivider.ariaLabel',
        'Conversation context compacted at this point',
      )}
    >
      <button
        type="button"
        onClick={() => setExpanded((current) => !current)}
        className="flex w-full items-start justify-between gap-3 text-left"
        aria-expanded={expanded}
      >
        <div className="flex min-w-0 items-start gap-2">
          <DatabaseZap
            size={14}
            className="mt-0.5 shrink-0 text-muted-foreground"
          />
          <div className="min-w-0">
            <div className="text-sm font-medium text-foreground">
              {t('agent.compactDivider.label', 'Context compacted')}
            </div>
            <div className="mt-1 space-y-1 text-xs text-muted-foreground">
              <div>
                <span className="font-medium">
                  {t('agent.compactDivider.latestIncluded', 'Latest included')}:
                </span>{' '}
                {previewLabel(
                  latestIncludedPreview,
                  t(
                    'agent.compactDivider.latestFallback',
                    'Newest message folded into the summary',
                  ),
                )}
              </div>
              {countLabel ? <div>{countLabel}</div> : null}
            </div>
          </div>
        </div>
        <span className="shrink-0 pt-0.5 text-muted-foreground">
          {expanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
        </span>
      </button>

      {expanded && summary ? (
        <div className="mt-3 rounded-md border border-border bg-background/70 px-3 py-2">
          <div className="mb-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
            {t('agent.compactDivider.summary', 'Summary')}
          </div>
          <div className="whitespace-pre-wrap text-sm text-foreground">
            {summary}
          </div>
        </div>
      ) : null}
    </div>
  );
}
