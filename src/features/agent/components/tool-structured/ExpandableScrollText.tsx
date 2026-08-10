import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronDown, ChevronUp } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

export interface ExpandableScrollTextProps {
  text: string;
  /** Collapsed preview line clamp (default 3). */
  collapsedLines?: number;
  /** Max height class when expanded (default max-h-64). */
  expandedMaxHeightClassName?: string;
  /** Expand control label (default: Show result). */
  showLabel?: string;
  /** Collapse control label (default: Hide result). */
  hideLabel?: string;
  className?: string;
  'data-testid'?: string;
}

/**
 * Human skim pattern: short clamp → expand into a fixed-height scroll panel.
 * Does not grow the chat row without bound.
 */
export const ExpandableScrollText: React.FC<ExpandableScrollTextProps> = ({
  text,
  collapsedLines = 3,
  expandedMaxHeightClassName = 'max-h-64',
  showLabel,
  hideLabel,
  className,
  'data-testid': testId,
}) => {
  const { t } = useTranslation('common');
  const [expanded, setExpanded] = useState(false);
  const trimmed = text.trim();
  if (!trimmed) return null;

  const lineCount = trimmed.split(/\r?\n/).length;
  const roughlyLong = trimmed.length > 180 || lineCount > collapsedLines;
  const showToggle = roughlyLong;
  const expandLabel =
    showLabel ?? t('agent.toolStructured.agentCard.showResult', 'Show result');
  const collapseLabel =
    hideLabel ?? t('agent.toolStructured.agentCard.hideResult', 'Hide result');

  return (
    <div data-testid={testId} className={cn('space-y-1', className)}>
      <div
        className={cn(
          'rounded border bg-muted/40 px-2.5 py-2 text-sm whitespace-pre-wrap break-words',
          expanded
            ? cn(expandedMaxHeightClassName, 'overflow-y-auto')
            : 'overflow-hidden',
        )}
        style={
          expanded
            ? undefined
            : {
                display: '-webkit-box',
                WebkitLineClamp: collapsedLines,
                WebkitBoxOrient: 'vertical',
              }
        }
      >
        {trimmed}
      </div>
      {showToggle ? (
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="h-7 px-2 text-xs text-muted-foreground"
          onClick={() => setExpanded((prev) => !prev)}
          aria-expanded={expanded}
        >
          {expanded ? (
            <>
              <ChevronUp className="mr-1 h-3.5 w-3.5" />
              {collapseLabel}
            </>
          ) : (
            <>
              <ChevronDown className="mr-1 h-3.5 w-3.5" />
              {expandLabel}
            </>
          )}
        </Button>
      ) : null}
    </div>
  );
};
