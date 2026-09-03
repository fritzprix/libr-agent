import React from 'react';
import { useTranslation } from 'react-i18next';
import { Info } from 'lucide-react';
import {
  Button,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui';
import {
  THINKING_EFFORT_VALUES,
  type ThinkingEffort,
} from '@/lib/ai-service/thinking-effort-mapping';

const THINKING_EFFORT_LABELS: Record<ThinkingEffort, string> = {
  off: 'Off',
  low: 'Low',
  medium: 'Medium',
  high: 'High',
  auto: 'Auto',
};

export interface ThinkingEffortSectionProps {
  thinkingEffort: ThinkingEffort;
  onThinkingEffortChange: (effort: ThinkingEffort) => void;
}

function ThinkingEffortSectionComponent({
  thinkingEffort,
  onThinkingEffortChange,
}: ThinkingEffortSectionProps) {
  const { t } = useTranslation('common');

  return (
    <div className="min-w-0 space-y-3">
      <div>
        <div className="flex items-center gap-2">
          <label className="block font-medium text-muted-foreground">
            {t('settings.aiModels.thinkingEffort', 'Thinking Effort')}
          </label>
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
                <Info className="h-3.5 w-3.5" />
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
        <p className="mt-1 text-xs text-muted-foreground">
          {t(
            'settings.aiModels.thinkingEffortDescription',
            'How deeply the model reasons before answering. Unsupported models return a provider error.',
          )}
        </p>
      </div>

      <div className="flex flex-wrap gap-2">
        {THINKING_EFFORT_VALUES.map((preset) => (
          <Button
            key={preset}
            type="button"
            variant={preset === thinkingEffort ? 'default' : 'outline'}
            className="h-8 px-3 text-xs"
            onClick={() => onThinkingEffortChange(preset)}
          >
            {THINKING_EFFORT_LABELS[preset]}
          </Button>
        ))}
      </div>
    </div>
  );
}

export const ThinkingEffortSection = React.memo(ThinkingEffortSectionComponent);
