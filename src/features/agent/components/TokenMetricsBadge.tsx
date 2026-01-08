import type { TokenUsage } from '@/lib/ai-service/types';
import { calculateTokensPerSecond } from '@/lib/ai-service/utils';
import { ArrowDown, ArrowUp, Zap } from 'lucide-react';

interface TokenMetricsBadgeProps {
  usage: TokenUsage;
  showSpeed?: boolean;
  className?: string;
  compact?: boolean;
}

export function TokenMetricsBadge({
  usage,
  showSpeed = true,
  className = '',
  compact = false,
}: TokenMetricsBadgeProps) {
  // Calculate speed if duration is available (either from provider or context-estimated)
  const tokensPerSec =
    usage.details?.evalDuration && usage.completionTokens > 0
      ? calculateTokensPerSecond(usage, usage.details.evalDuration)
      : null;

  const tpsFormatted = tokensPerSec ? tokensPerSec.toFixed(1) : null;

  // Build prefill timing info (prioritize native metric over client-side TTFT)
  const prefillInfo = usage.details?.promptEvalDuration
    ? ` • Prefill: ${usage.details.promptEvalDuration.toFixed(0)}ms`
    : usage.details?.timeToFirstToken
      ? ` • TTFT: ${usage.details.timeToFirstToken.toFixed(0)}ms`
      : '';

  return (
    <div
      className={`flex items-center gap-2 text-xs font-mono tabular-nums ${className}`}
      data-testid="metrics-badge"
    >
      {/* Input Tokens */}
      <div className="flex items-center gap-2">
        <span
          className="flex items-center gap-0.5 text-blue-400"
          title={
            (usage.details?.cacheReadInputTokens
              ? `Prompt Tokens (Read from Cache: ${usage.details.cacheReadInputTokens.toLocaleString()}, Created: ${usage.details.cacheCreationInputTokens?.toLocaleString() || 0})`
              : 'Prompt Tokens') + prefillInfo
          }
        >
          <ArrowUp size={10} className="stroke-[3]" />
          {usage.promptTokens.toLocaleString()}
        </span>
        {/* Cache Hit Indicator */}
        {usage.details?.cacheReadInputTokens ? (
          <span
            className="flex items-center gap-0.5 text-xs text-blue-300/70"
            title="Cached Input Tokens"
          >
            <Zap size={10} className="fill-current" />
            {(
              (usage.details.cacheReadInputTokens / usage.promptTokens) *
              100
            ).toFixed(0)}
            %
          </span>
        ) : null}
      </div>

      {/* Output Tokens */}
      <span
        className="flex items-center gap-0.5 text-green-400"
        title="Completion Tokens"
      >
        <ArrowDown size={10} className="stroke-[3]" />
        {usage.completionTokens.toLocaleString()}
      </span>

      {/* Speed (if available) - Hide on compact unless specifically requested */}
      {showSpeed && tpsFormatted && (
        <>
          <span className="text-gray-600 mx-0.5">•</span>
          <span
            className="flex items-center gap-0.5 text-yellow-500"
            title="Tokens per second"
          >
            <Zap size={10} className="stroke-[3]" />
            {tpsFormatted}{' '}
            <span className="text-[10px] text-gray-500 hidden sm:inline">
              t/s
            </span>
          </span>
        </>
      )}

      {/* Load Duration (Ollama only, for debug) - Optional, maybe hide in production UI */}
      {!compact && usage.details?.loadDuration && (
        <span className="hidden lg:flex items-center gap-0.5 text-gray-500 text-[10px] ml-1">
          (Load: {(usage.details.loadDuration / 1000).toFixed(1)}s)
        </span>
      )}
    </div>
  );
}
