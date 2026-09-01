import React from 'react';
import { useTranslation } from 'react-i18next';
import { Info } from 'lucide-react';
import { Button, Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui';
import type { ThinkingEffort } from '@/lib/ai-service/thinking-effort-mapping';

const THINKING_EFFORT_PRESETS: ReadonlyArray<{
  label: string;
  value: ThinkingEffort;
}> = [
  { label: 'Off', value: 'off' },
  { label: 'Low', value: 'low' },
  { label: 'Medium', value: 'medium' },
  { label: 'High', value: 'high' },
  { label: 'Auto', value: 'auto' },
];

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
                'Best-effort control over reasoning depth. Unsupported models skip the parameter. Some providers may ignore it. Off still allows thinking tokens in the response if the server sends them. Anthropic uses extended thinking for all non-off levels.',
              )}
            </TooltipContent>
          </Tooltip>
        </div>
        <p className="mt-1 text-xs text-muted-foreground">
          {t(
            'settings.aiModels.thinkingEffortDescription',
            'How deeply the model reasons before answering, when supported.',
          )}
        </p>
      </div>

      <div className="flex flex-wrap gap-2">
        {THINKING_EFFORT_PRESETS.map((preset) => (
          <Button
            key={preset.value}
            type="button"
            variant={preset.value === thinkingEffort ? 'default' : 'outline'}
            className="h-8 px-3 text-xs"
            onClick={() => onThinkingEffortChange(preset.value)}
          >
            {preset.label}
          </Button>
        ))}
      </div>
    </div>
  );
}

export const ThinkingEffortSection = React.memo(ThinkingEffortSectionComponent);
