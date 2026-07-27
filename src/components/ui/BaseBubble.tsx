import React, { useState, useId } from 'react';
import { ChevronDown, ChevronRight, Copy, Check } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { getLogger } from '@/lib/logger';
import { cn } from '@/lib/utils';

const logger = getLogger('BaseBubble');

interface BaseBubbleProps {
  title: string;
  icon?: React.ReactNode;
  badge?: React.ReactNode;
  defaultExpanded?: boolean;
  copyData?: string;
  collapsedSummary?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
  toggleAriaLabel?: string;
}

export const BaseBubble: React.FC<BaseBubbleProps> = ({
  title,
  icon,
  badge,
  defaultExpanded = false,
  copyData,
  collapsedSummary,
  children,
  className = '',
  toggleAriaLabel,
}) => {
  const { t } = useTranslation('common');
  const [copied, setCopied] = useState(false);
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);
  const contentId = useId();

  const copyToClipboard = async () => {
    if (!copyData) return;

    try {
      await navigator.clipboard.writeText(copyData);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      logger.error('Failed to copy data: ', err);
    }
  };

  return (
    <div
      className={cn(
        'mt-4 overflow-hidden rounded-lg border border-border bg-background shadow-sm',
        className,
      )}
    >
      <div className="flex items-center justify-between border-b border-border bg-muted/60 px-4 py-3">
        <button
          type="button"
          onClick={() => setIsExpanded(!isExpanded)}
          aria-expanded={isExpanded}
          aria-controls={contentId}
          aria-label={toggleAriaLabel || title}
          className="flex items-center gap-2 rounded-sm text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          {isExpanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
          {icon}
          <span className="flex items-center gap-2 font-mono text-sm">
            {title}
            {badge}
          </span>
        </button>
        {copyData ? (
          <button
            type="button"
            onClick={copyToClipboard}
            aria-label={t('baseBubble.copy', {
              title,
              defaultValue: `Copy ${title} to clipboard`,
            })}
            className="flex items-center gap-2 rounded px-3 py-1.5 text-xs text-secondary-foreground transition-colors bg-secondary hover:bg-secondary/80 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            {copied ? <Check size={12} /> : <Copy size={12} />}
            {copied ? t('common.copied', 'Copied!') : t('common.copy', 'Copy')}
          </button>
        ) : null}
      </div>

      {!isExpanded && collapsedSummary ? (
        <div className="bg-background/50 px-4 py-3 text-sm text-muted-foreground">
          {collapsedSummary}
        </div>
      ) : null}

      <div
        id={contentId}
        className={cn(
          'max-h-96 overflow-auto bg-background p-4',
          !isExpanded && 'hidden',
        )}
      >
        {children}
      </div>
    </div>
  );
};
