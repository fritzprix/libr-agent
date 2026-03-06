import React, { useState, useId } from 'react';
import { ChevronDown, ChevronRight, Copy, Check } from 'lucide-react';
import { useTranslation } from 'react-i18next';

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
      console.error('Failed to copy data: ', err);
    }
  };

  return (
    <div
      className={`mt-4 bg-background rounded-lg border border-border overflow-hidden shadow-lg ${className}`}
    >
      <div className="px-4 py-3 bg-muted border-b border-border flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="flex gap-1.5" aria-hidden="true">
            <div className="w-3 h-3 bg-destructive rounded-full"></div>
            <div className="w-3 h-3 bg-warning rounded-full"></div>
            <div className="w-3 h-3 bg-success rounded-full"></div>
          </div>
          <button
            onClick={() => setIsExpanded(!isExpanded)}
            aria-expanded={isExpanded}
            aria-controls={contentId}
            aria-label={
              toggleAriaLabel || t('baseBubble.toggle', { title, defaultValue: `Toggle ${title}` })
            }
            className="flex items-center gap-2 text-muted-foreground hover:text-foreground transition-colors rounded-sm focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
          >
            {isExpanded ? (
              <ChevronDown size={16} />
            ) : (
              <ChevronRight size={16} />
            )}
            {icon}
            <span className="font-mono text-sm flex items-center gap-2">
              {title}
              {badge}
            </span>
          </button>
        </div>
        {copyData && (
          <button
            onClick={copyToClipboard}
            aria-label={t('baseBubble.copy', { title, defaultValue: `Copy ${title} to clipboard` })}
            className="flex items-center gap-2 px-3 py-1.5 bg-secondary hover:bg-secondary/80 text-secondary-foreground text-xs rounded transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
          >
            {copied ? <Check size={12} /> : <Copy size={12} />}
            {copied ? t('common.copied', 'Copied!') : t('common.copy', 'Copy')}
          </button>
        )}
      </div>

      {!isExpanded && collapsedSummary && (
        <div className="px-4 py-3 bg-background/50 text-muted-foreground text-sm">
          {collapsedSummary}
        </div>
      )}

      <div
        id={contentId}
        className={`p-4 max-h-96 overflow-auto bg-background ${!isExpanded ? 'hidden' : ''}`}
      >
        {children}
      </div>
    </div>
  );
};
