import React, { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Info } from 'lucide-react';

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui';
import {
  THINKING_EFFORT_VALUES,
  type ThinkingEffort,
} from '@/lib/ai-service/thinking-effort-mapping';
import { cn } from '@/lib/utils';

const THINKING_EFFORT_LABEL_KEYS: Record<ThinkingEffort, string> = {
  off: 'settings.aiModels.thinkingEffortOff',
  low: 'settings.aiModels.thinkingEffortLow',
  medium: 'settings.aiModels.thinkingEffortMedium',
  high: 'settings.aiModels.thinkingEffortHigh',
  auto: 'settings.aiModels.thinkingEffortAuto',
};

const THINKING_EFFORT_DEFAULT_LABELS: Record<ThinkingEffort, string> = {
  off: 'Off',
  low: 'Low',
  medium: 'Medium',
  high: 'High',
  auto: 'Auto',
};

export interface ThinkingEffortControlProps {
  thinkingEffort: ThinkingEffort;
  onThinkingEffortChange: (effort: ThinkingEffort) => void;
  disabled?: boolean;
  compact?: boolean;
  className?: string;
  showTooltip?: boolean;
}

function ThinkingEffortControlComponent({
  thinkingEffort,
  onThinkingEffortChange,
  disabled = false,
  compact = false,
  className,
  showTooltip = true,
}: ThinkingEffortControlProps) {
  const { t } = useTranslation('common');

  const effortLabel = useMemo(() => {
    return t(
      THINKING_EFFORT_LABEL_KEYS[thinkingEffort],
      THINKING_EFFORT_DEFAULT_LABELS[thinkingEffort],
    );
  }, [t, thinkingEffort]);

  const select = (
    <Select
      value={thinkingEffort}
      onValueChange={(value) => onThinkingEffortChange(value as ThinkingEffort)}
      disabled={disabled}
    >
      <SelectTrigger
        className={cn(
          'h-6 shrink-0 border-none bg-transparent px-1 text-xs shadow-none gap-1 focus:ring-0 [&>span]:truncate',
          compact ? 'w-[4.5rem]' : 'w-[5.25rem]',
          className,
        )}
      >
        <SelectValue>{effortLabel}</SelectValue>
      </SelectTrigger>
      <SelectContent>
        {THINKING_EFFORT_VALUES.map((value) => (
          <SelectItem key={value} value={value}>
            {t(
              THINKING_EFFORT_LABEL_KEYS[value],
              THINKING_EFFORT_DEFAULT_LABELS[value],
            )}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );

  if (!showTooltip) {
    return select;
  }

  return (
    <div className="flex items-center gap-1">
      {select}
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            className="inline-flex text-muted-foreground hover:text-foreground"
            aria-label={t(
              'settings.aiModels.thinkingEffortTooltipAria',
              'About thinking effort',
            )}
          >
            <Info className="h-3 w-3" />
          </button>
        </TooltipTrigger>
        <TooltipContent className="max-w-xs text-pretty">
          {t(
            'settings.aiModels.thinkingEffortTooltip',
            'Controls how deeply the model reasons before answering. The setting is always sent when enabled. Models or providers that do not support it return an API error — turn effort Off or switch models. Anthropic uses extended thinking for all non-off levels.',
          )}
        </TooltipContent>
      </Tooltip>
    </div>
  );
}

export const ThinkingEffortControl = React.memo(ThinkingEffortControlComponent);
