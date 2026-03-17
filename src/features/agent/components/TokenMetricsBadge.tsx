import type { TokenUsage } from '@/lib/ai-service/types';
import { calculateTokensPerSecond } from '@/lib/ai-service/utils';
import { useSettings } from '@/context/SettingsContext';
import { ArrowDown, ArrowUp, Zap, Gauge } from 'lucide-react';
import { calculateCacheHitPercent } from './token-metrics';

interface ContextUsage {
  totalTokens: number;
  contextWindow: number;
  modelMaxContext?: number;
}

interface TokenMetricsBadgeProps {
  usage: TokenUsage;
  showSpeed?: boolean;
  className?: string;
  compact?: boolean;
  /** When provided, renders a context-window usage gauge below the metrics. */
  contextUsage?: ContextUsage;
}

function ContextGauge({
  totalTokens,
  contextWindow,
  modelMaxContext,
}: ContextUsage) {
  const pct = Math.min(totalTokens / contextWindow, 1);
  const pctDisplay = (pct * 100).toFixed(0);

  let barColor: string;
  if (pct >= 0.9) {
    // 90% matches the compaction trigger
    barColor = 'bg-destructive';
  } else if (pct >= 0.8) {
    barColor = 'bg-warning';
  } else {
    barColor = 'bg-success';
  }

  const tooltipTitle =
    modelMaxContext && modelMaxContext !== contextWindow
      ? `Context: ${totalTokens.toLocaleString()} / ${contextWindow.toLocaleString()} tokens (Effective Limit)\nModel Max: ${modelMaxContext.toLocaleString()} tokens`
      : `Context: ${totalTokens.toLocaleString()} / ${contextWindow.toLocaleString()} tokens (${pctDisplay}%)`;

  return (
    <div className="flex items-center gap-1.5 mt-0.5" title={tooltipTitle}>
      <div className="h-1 w-20 rounded-full bg-muted overflow-hidden">
        <div
          className={`h-full rounded-full transition-all duration-500 ${barColor}`}
          style={{ width: `${pct * 100}%` }}
        />
      </div>
      <span className="text-[10px] text-muted-foreground tabular-nums">
        {pctDisplay}%
      </span>
    </div>
  );
}

export function TokenMetricsBadge({
  usage,
  showSpeed: showSpeedProp,
  className = '',
  compact: compactProp,
  contextUsage,
}: TokenMetricsBadgeProps) {
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

  // Calculate speed if duration is available (either from provider or context-estimated)
  const tokensPerSec =
    usage.details?.evalDuration && usage.completionTokens > 0
      ? calculateTokensPerSecond(usage, usage.details.evalDuration)
      : null;

  const tpsFormatted = tokensPerSec ? tokensPerSec.toFixed(1) : null;

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
      <div className="flex items-center gap-2">
        {/* Input Tokens */}
        <span
          className="flex items-center gap-0.5 text-primary"
          title={
            (hasCacheHit
              ? `Prompt Tokens (Read from Cache: ${cachedTokens.toLocaleString()}, Created: ${usage.details?.cacheCreationInputTokens?.toLocaleString() || 0})`
              : 'Prompt Tokens') + prefillInfo
          }
        >
          <ArrowUp size={10} className="stroke-[3]" />
          {(usage.promptTokens ?? 0).toLocaleString()}
        </span>

        {/* Output Tokens */}
        <span
          className="flex items-center gap-0.5 text-success"
          title="Completion Tokens"
        >
          <ArrowDown size={10} className="stroke-[3]" />
          {(usage.completionTokens ?? 0).toLocaleString()}
        </span>

        {/* ✅ Cache Hit Indicator (Independent Placement) */}
        {isCacheActive && (
          <span
            className="flex items-center gap-0.5 text-[10px] font-bold text-cyan-400 bg-cyan-400/10 px-1 rounded border border-cyan-400/20 shrink-0"
            title={`Cache Hit: ${cachedTokens.toLocaleString()} tokens (${cacheHitPercent}%)`}
          >
            <Zap size={10} className="fill-current" />
            {cacheHitPercent}%
          </span>
        )}

        {/* Speed (if available) - Hide on compact unless specifically requested */}
        {showSpeed && tpsFormatted && (
          <>
            <span className="text-muted-foreground mx-0.5">•</span>
            <span
              className="flex items-center gap-0.5 text-warning"
              title="Tokens per second"
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

      {/* Context window usage gauge — only shown when contextUsage is provided */}
      {contextUsage && <ContextGauge {...contextUsage} />}
    </div>
  );
}
