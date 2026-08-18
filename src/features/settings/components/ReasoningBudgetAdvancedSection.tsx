import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronDown, ChevronUp, ExternalLink } from 'lucide-react';
import { Button, Input, Textarea } from '@/components/ui';
import type { AIServiceProvider } from '@/lib/ai-service';
import { openExternalUrl } from '@/lib/backend';
import {
  DEFAULT_REASONING_BUDGET_MESSAGE,
  MAX_REASONING_BUDGET_TOKENS,
  parseReasoningBudgetInput,
} from '@/lib/ai-service/openai/reasoning-budget';

export const REASONING_BUDGET_DOCS_URL =
  'https://github.com/fritzprix/libr-agent/issues/1768';

export interface ReasoningBudgetAdvancedSectionProps {
  /** Unique id or provider prefix used for HTML element IDs and labels */
  idPrefix: string | AIServiceProvider;
  reasoningBudget?: number;
  reasoningBudgetMessage?: string;
  onChange: (patch: {
    reasoningBudget?: number;
    reasoningBudgetMessage?: string;
  }) => void;
}

/**
 * Reusable Advanced settings section for configuring client-side thinking token caps.
 * Auto-expands on mount if reasoning budget or custom budget message is already set.
 */
export function ReasoningBudgetAdvancedSection({
  idPrefix,
  reasoningBudget,
  reasoningBudgetMessage,
  onChange,
}: ReasoningBudgetAdvancedSectionProps) {
  const { t } = useTranslation('common');
  const [advancedOpen, setAdvancedOpen] = useState(
    () => reasoningBudget != null || Boolean(reasoningBudgetMessage),
  );

  return (
    <div className="min-w-0 border-t pt-3">
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className="w-full justify-between px-0 text-sm font-medium text-foreground hover:bg-transparent"
        onClick={() => setAdvancedOpen((open) => !open)}
        aria-expanded={advancedOpen}
      >
        {t('settings.customProviders.advanced', 'Advanced')}
        {advancedOpen ? (
          <ChevronUp className="h-4 w-4 text-muted-foreground" />
        ) : (
          <ChevronDown className="h-4 w-4 text-muted-foreground" />
        )}
      </Button>

      {advancedOpen ? (
        <div className="mt-3 space-y-3">
          <div className="min-w-0">
            <label
              htmlFor={`reasoning-budget-${idPrefix}`}
              className="block text-muted-foreground mb-2 text-sm font-medium"
            >
              {t(
                'settings.customProviders.reasoningBudget',
                'Reasoning budget (tokens)',
              )}
            </label>
            <Input
              id={`reasoning-budget-${idPrefix}`}
              type="number"
              min={1}
              max={MAX_REASONING_BUDGET_TOKENS}
              step={1}
              placeholder={t(
                'settings.customProviders.reasoningBudgetPlaceholder',
                'Unlimited',
              )}
              value={reasoningBudget ?? ''}
              onChange={(e) => {
                const raw = e.target.value.trim();
                if (raw === '') {
                  onChange({ reasoningBudget: undefined });
                  return;
                }
                const parsed = parseReasoningBudgetInput(raw);
                if (parsed == null) {
                  return;
                }
                onChange({ reasoningBudget: parsed });
              }}
              className="bg-background border text-foreground w-full"
            />
            <div className="text-xs text-muted-foreground mt-1 space-y-1">
              <p>
                {t(
                  'settings.customProviders.reasoningBudgetDescription',
                  'Limits how long the AI can think before giving an answer. Leave empty for unlimited.',
                )}
              </p>
              <p>
                <a
                  href={REASONING_BUDGET_DOCS_URL}
                  target="_blank"
                  rel="noopener noreferrer"
                  onClick={(e) => {
                    e.preventDefault();
                    void openExternalUrl(REASONING_BUDGET_DOCS_URL);
                  }}
                  className="text-primary underline hover:text-primary/80 inline-flex items-center gap-1 cursor-pointer"
                >
                  {t(
                    'settings.customProviders.reasoningBudgetLearnMore',
                    'Learn more about reasoning budget',
                  )}
                  <ExternalLink className="h-3 w-3 inline" />
                </a>
              </p>
            </div>
          </div>

          <div className="min-w-0">
            <label
              htmlFor={`reasoning-budget-message-${idPrefix}`}
              className="block text-muted-foreground mb-2 text-sm font-medium"
            >
              {t(
                'settings.customProviders.reasoningBudgetMessage',
                'Budget exceeded message',
              )}
            </label>
            <Textarea
              id={`reasoning-budget-message-${idPrefix}`}
              placeholder={t(
                'settings.customProviders.reasoningBudgetMessagePlaceholder',
                DEFAULT_REASONING_BUDGET_MESSAGE,
              )}
              value={reasoningBudgetMessage ?? ''}
              onChange={(e) =>
                onChange({
                  reasoningBudgetMessage: e.target.value || undefined,
                })
              }
              className="bg-background border text-foreground w-full min-h-[72px]"
            />
            <p className="text-xs text-muted-foreground mt-1">
              {t(
                'settings.customProviders.reasoningBudgetMessageDescription',
                'Prompt injected to make the AI start answering immediately when the thinking limit is reached.',
              )}
            </p>
          </div>
        </div>
      ) : null}
    </div>
  );
}
