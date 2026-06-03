import type { TokenUsage } from '@/lib/ai-service/types';
import type { PreflightTokenMetrics } from '@/models/agent-ipc';
import { calculateTokensPerSecond } from '@/lib/ai-service/utils';
import { useSettings } from '@/context/SettingsContext';
import { ArrowDown, ArrowUp, Zap, Gauge } from 'lucide-react';
import { calculateCacheHitPercent } from './token-metrics';
import { useTranslation } from 'react-i18next';
import { formatNumber } from '@/lib/utils';

interface TokenMetricsBadgeProps {
  usage: TokenUsage;
  preflight?: PreflightTokenMetrics | null;
  showSpeed?: boolean;
  className?: string;
  compact?: boolean;
}

export function TokenMetricsBadge({
  usage,
  preflight = null,
  showSpeed: showSpeedProp,
  className = '',
  compact: compactProp,
}: TokenMetricsBadgeProps) {
  const { t } = useTranslation();
  // Get display preferences from settings
  const { value: settings } = useSettings();
  const displaySettings = settings.display || {
    metricDisplayMode: 'inline',
    prefillDisplayFormat: 'time',
    showTokenSpeed: true,
    compactMetrics: false,
  };

  // Use settings or props (props take precedence for backward compatibility)
  const showSpeed = showSpeedProp ?? displaySettings.showTokenSpeed;
  const compact = compactProp ?? displaySettings.compactMetrics;
  const showInlineSpeed = compact ? showSpeedProp === true : showSpeed;

  // Calculate speed if duration is available (either from provider or context-estimated)
  const tokensPerSec =
    usage.details?.evalDuration && usage.completionTokens > 0
      ? calculateTokensPerSecond(usage, usage.details.evalDuration)
      : null;

  const tpsFormatted = tokensPerSec ? tokensPerSec.toFixed(1) : null;
  const inputTokens =
    preflight?.conservativePromptTokens ?? usage.promptTokens ?? 0;
  const inputLimit = preflight?.safeInputTokenLimit;
  const inputLimitLabel = inputLimit ? formatNumber(inputLimit) : null;
  const promptDisplayLabel = formatNumber(inputTokens);
  const providerPromptLabel = formatNumber(usage.promptTokens ?? 0);

  const cachedTokens =
    usage.cachedPromptTokens ??
    usage.details?.cacheReadInputTokens ??
    usage.details?.cachedContentTokenCount ??
    usage.details?.prompt_cache_hit_tokens ??
    0;

  // Show indicator if we have explicit cache metadata (even if 0 during pre-calculation)
  // or if cached tokens are > 0
  const isCacheActive =
    usage.cachedPromptTokens !== undefined ||
    usage.details?.cacheReadInputTokens !== undefined ||
    usage.details?.cachedContentTokenCount !== undefined ||
    usage.details?.prompt_cache_hit_tokens !== undefined;

  const hasCacheHit = cachedTokens > 0 || isCacheActive;
  const cacheHitPercent = hasCacheHit
    ? calculateCacheHitPercent(cachedTokens, usage.promptTokens)
    : 0;
  const cacheIndicatorText =
    cachedTokens > 0
      ? `${cacheHitPercent}% · ${formatNumber(cachedTokens)}`
      : 'cache';

  // Calculate prefill tokens per second if both TTFT and prompt tokens are available
  const prefillTPS =
    usage.details?.timeToFirstToken && usage.promptTokens > 0
      ? (usage.promptTokens / (usage.details.timeToFirstToken / 1000)).toFixed(
          1,
        )
      : null;

  // Build prefill timing info based on user preference
  let prefillInfo = '';
  if (
    displaySettings.prefillDisplayFormat === 'tokensPerSecond' &&
    prefillTPS
  ) {
    prefillInfo = ` • Prefill: ${prefillTPS} tok/s`;
  } else if (usage.details?.promptEvalDuration) {
    prefillInfo = ` • Prefill: ${usage.details.promptEvalDuration.toFixed(0)}ms`;
  } else if (usage.details?.timeToFirstToken) {
    prefillInfo = ` • TTFT: ${usage.details.timeToFirstToken.toFixed(0)}ms`;
  }

  return (
    <div
      className={`flex flex-col text-xs font-mono tabular-nums ${className}`}
      data-testid="metrics-badge"
    >
      <div
        className={
          compact ? 'flex items-center gap-1.5' : 'flex items-center gap-2'
        }
      >
        {/* Input Tokens */}
        <span
          className="flex items-center gap-0.5 text-primary"
          title={
            preflight
              ? t('agent.metrics.preflightContext', {
                  display: promptDisplayLabel,
                  limit: inputLimitLabel ? ` / ${inputLimitLabel}` : '',
                  reserved: formatNumber(preflight.measuredOutputTokensReserve),
                  totalBudget: formatNumber(preflight.totalBudgetTokens),
                  effectiveBudget: formatNumber(preflight.effectiveInputBudget),
                  providerTokens: providerPromptLabel,
                  systemPrompt: formatNumber(preflight.systemPromptTokens),
                  tools: formatNumber(preflight.toolsTokens),
                  selectedMessages: formatNumber(
                    preflight.selectedMessageCount,
                  ),
                  prefill: prefillInfo,
                  defaultValue: `Backend preflight context estimate: ${promptDisplayLabel}${inputLimitLabel ? ` / ${inputLimitLabel}` : ''} tokens. Reserved output: ${formatNumber(preflight.measuredOutputTokensReserve)}. Total budget: ${formatNumber(preflight.totalBudgetTokens)}. Effective input budget: ${formatNumber(preflight.effectiveInputBudget)}. Provider prompt tokens: ${providerPromptLabel}. System prompt: ${formatNumber(preflight.systemPromptTokens)}. Tools: ${formatNumber(preflight.toolsTokens)}. Selected messages: ${formatNumber(preflight.selectedMessageCount)}.${prefillInfo}`,
                })
              : (hasCacheHit
                  ? t('agent.metrics.promptTokensCache', {
                      read: formatNumber(cachedTokens),
                      created: formatNumber(
                        usage.details?.cacheCreationInputTokens || 0,
                      ),
                      defaultValue: `Prompt Tokens (Read from Cache: ${formatNumber(cachedTokens)}, Created: ${formatNumber(usage.details?.cacheCreationInputTokens || 0)})`,
                    })
                  : t('agent.metrics.promptTokens', 'Prompt Tokens')) +
                prefillInfo
          }
        >
          <ArrowUp size={10} className="stroke-[3]" />
          {promptDisplayLabel}
          {inputLimitLabel && (
            <span className="text-[10px] text-muted-foreground">
              /{inputLimitLabel}
            </span>
          )}
        </span>

        {/* Output Tokens */}
        <span
          className="flex items-center gap-0.5 text-success"
          title={t('agent.metrics.completionTokens', 'Completion Tokens')}
        >
          <ArrowDown size={10} className="stroke-[3]" />
          {formatNumber(usage.completionTokens ?? 0)}
        </span>

        {/* ✅ Cache Hit Indicator (Independent Placement) */}
        {isCacheActive && (
          <span
            data-testid="cache-hit-indicator"
            className="flex items-center gap-0.5 text-[10px] font-bold text-cyan-400 bg-cyan-400/10 px-1 rounded border border-cyan-400/20 shrink-0"
            title={t(
              'agent.metrics.cacheHit',
              `Cache Hit: ${formatNumber(cachedTokens)} tokens (${cacheHitPercent}%)`,
              {
                countStr: formatNumber(cachedTokens),
                percent: cacheHitPercent,
              },
            )}
          >
            <Zap size={10} className="fill-current" />
            {cacheIndicatorText}
          </span>
        )}

        {/* Speed (if available) - Compact mode only shows it when explicitly requested. */}
        {showInlineSpeed && tpsFormatted && (
          <>
            <span className="text-muted-foreground mx-0.5">•</span>
            <span
              className="flex items-center gap-0.5 text-warning"
              title={t('agent.metrics.tokensPerSecond', 'Tokens per second')}
            >
              <Gauge size={10} className="stroke-[3]" />
              {tpsFormatted}{' '}
              <span className="text-xs text-muted-foreground hidden sm:inline">
                t/s
              </span>
            </span>
          </>
        )}

        {/* Load Duration (Ollama only, for debug) - Optional, maybe hide in production UI */}
        {!compact && usage.details?.loadDuration && (
          <span className="hidden lg:flex items-center gap-0.5 text-muted-foreground text-xs ml-1">
            (Load: {(usage.details.loadDuration / 1000).toFixed(1)}s)
          </span>
        )}
      </div>
    </div>
  );
}
