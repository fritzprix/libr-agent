import React from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { Settings } from 'lucide-react';
import { Button } from '@/components/ui/button';

/** Error codes for a turn stopped by long or looping reasoning. */
export const REASONING_STOP_CODES = [
  'REASONING_BUDGET_EXCEEDED',
  'REPEATED_THINKING_LOOP',
  'REPEATED_TEXT_LOOP',
  'THINKING_ONLY_COMPLETION',
] as const;

export type ReasoningStopCode = (typeof REASONING_STOP_CODES)[number];

export function isReasoningStopCode(
  code: string | undefined,
): code is ReasoningStopCode {
  return (REASONING_STOP_CODES as readonly string[]).includes(code ?? '');
}

/** Reads a positive finite number from the Rust error details, if present. */
function readNumber(details: unknown, key: string): number | undefined {
  if (typeof details !== 'object' || details === null) {
    return undefined;
  }
  const value = (details as Record<string, unknown>)[key];
  return typeof value === 'number' && Number.isFinite(value) && value > 0
    ? value
    : undefined;
}

function readString(details: unknown, key: string): string | undefined {
  if (typeof details !== 'object' || details === null) {
    return undefined;
  }
  const value = (details as Record<string, unknown>)[key];
  return typeof value === 'string' ? value : undefined;
}

interface ReasoningStopGuideProps {
  code: ReasoningStopCode;
  /** `error.details.originalError`: the numbers Rust attached to the stop. */
  details: unknown;
}

/**
 * Explains why a turn stopped for long or looping reasoning, shows the numbers
 * behind the stop, and lists next steps the product actually supports.
 *
 * Max Input Context is never suggested here: it does not affect these cutoffs.
 */
export const ReasoningStopGuide: React.FC<ReasoningStopGuideProps> = ({
  code,
  details,
}) => {
  const { t } = useTranslation('common');
  const navigate = useNavigate();
  const format = (value: number) => value.toLocaleString();

  const lines: string[] = [];
  let settingsTab: 'chat-interface' | 'advanced' | undefined;
  let settingsLabel = '';

  const outputTokensLabel = t(
    'errorBubble.reasoningStop.openOutputTokenSettings',
    'Open Max Output Tokens Settings',
  );

  if (code === 'REASONING_BUDGET_EXCEEDED') {
    const budget = readNumber(details, 'reasoningBudgetTokens');
    const used = readNumber(details, 'estimatedThinkingTokens');
    const max = readNumber(details, 'maxOutputTokens');
    const source = readString(details, 'maxOutputTokensSource');

    lines.push(
      t(
        'errorBubble.reasoningStop.budgetReason',
        'The model spent its output budget on reasoning without giving an answer or calling a tool, so the turn was stopped.',
      ),
    );
    if (budget !== undefined && used !== undefined) {
      lines.push(
        t(
          'errorBubble.reasoningStop.budgetNumbers',
          'Used about {{used}} tokens. The cutoff is {{budget}} tokens, which is 90% of Max Output Tokens.',
          { used: format(used), budget: format(budget) },
        ),
      );
    }
    if (max !== undefined && source === 'assistant') {
      lines.push(
        t(
          'errorBubble.reasoningStop.sourceAssistant',
          'Max Output Tokens is {{max}}, set on this assistant. Changing the Settings default will not change this session.',
          { max: format(max) },
        ),
      );
    } else if (max !== undefined && source === 'settings') {
      lines.push(
        t(
          'errorBubble.reasoningStop.sourceSettings',
          'Max Output Tokens is {{max}}, from Settings → Chat Interface → Max Output Tokens (Default).',
          { max: format(max) },
        ),
      );
    } else if (max !== undefined && source === 'fallback') {
      lines.push(
        t(
          'errorBubble.reasoningStop.sourceFallback',
          'Max Output Tokens is {{max}}, the built-in default, because no value is set in Settings.',
          { max: format(max) },
        ),
      );
    }

    if (source === 'assistant') {
      lines.push(
        t(
          'errorBubble.reasoningStop.budgetNextStepsAssistant',
          'Next: try again, or narrow the request so it needs less reasoning. Trying again with nothing changed often stops the same way.',
        ),
      );
    } else {
      lines.push(
        t(
          'errorBubble.reasoningStop.budgetNextSteps',
          'Next: raise Max Output Tokens (Default) in Settings, then try again. The new value applies to the next request. Trying again with nothing changed often stops the same way.',
        ),
      );
      settingsTab = 'chat-interface';
      settingsLabel = outputTokensLabel;
    }
  } else if (code === 'THINKING_ONLY_COMPLETION') {
    lines.push(
      t(
        'errorBubble.reasoningStop.thinkingOnlyReason',
        'The model finished with only internal reasoning: no answer and no tool call.',
      ),
      t(
        'errorBubble.reasoningStop.thinkingOnlyNextSteps',
        'Next: try again, or ask for a direct answer. If the model may have hit its output limit, raise Max Output Tokens (Default) in Settings first.',
      ),
    );
    settingsTab = 'chat-interface';
    settingsLabel = outputTokensLabel;
  } else {
    const patternLength = readNumber(details, 'patternLength');
    const repetitionCount = readNumber(details, 'repetitionCount');
    const isThinkingLoop = code === 'REPEATED_THINKING_LOOP';

    lines.push(
      isThinkingLoop
        ? t(
            'errorBubble.reasoningStop.thinkingLoopReason',
            'The model kept repeating the same reasoning, so the turn was stopped.',
          )
        : t(
            'errorBubble.reasoningStop.textLoopReason',
            'The model kept repeating the same text, so the turn was stopped.',
          ),
    );
    if (patternLength !== undefined && repetitionCount !== undefined) {
      lines.push(
        t(
          'errorBubble.reasoningStop.loopNumbers',
          'Detected a {{patternLength}}-character pattern repeated {{repetitionCount}} times.',
          {
            patternLength: format(patternLength),
            repetitionCount: format(repetitionCount),
          },
        ),
      );
    }
    if (isThinkingLoop) {
      lines.push(
        t(
          'errorBubble.reasoningStop.thinkingLoopNextSteps',
          'Next: try again, or rephrase or narrow the request. If this was a false alarm, raise Thinking Loop Min Pattern Length or Thinking Loop Min Repetitions in Settings → Advanced.',
        ),
      );
      settingsTab = 'advanced';
      settingsLabel = t(
        'errorBubble.reasoningStop.openLoopSettings',
        'Open Advanced Settings',
      );
    } else {
      lines.push(
        t(
          'errorBubble.reasoningStop.textLoopNextSteps',
          'Next: try again, or rephrase or narrow the request.',
        ),
      );
    }
  }

  const maxRetries = readNumber(details, 'maxRetries');
  if (maxRetries !== undefined) {
    lines.push(
      t(
        'errorBubble.reasoningStop.autoRetries',
        'Automatic retries already used: {{retries}}.',
        { retries: maxRetries },
      ),
    );
  }

  return (
    <>
      <div className="space-y-2 text-muted-foreground break-words">
        {lines.map((line) => (
          <p key={line}>{line}</p>
        ))}
      </div>
      {settingsTab ? (
        <Button
          onClick={() => navigate(`/settings?tab=${settingsTab}`)}
          variant="outline"
          size="sm"
          className="flex items-center gap-2"
        >
          <Settings className="w-4 h-4" />
          {settingsLabel}
        </Button>
      ) : null}
    </>
  );
};
