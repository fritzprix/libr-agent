import '@testing-library/jest-dom';
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { TokenUsage } from '@/lib/ai-service/types';
import type { PreflightTokenMetrics } from '@/models/agent-ipc';

import { TokenMetricsBadge } from '../TokenMetricsBadge';

vi.mock('@/context/SettingsContext', () => ({
  useSettings: () => ({
    value: {
      display: {
        prefillDisplayFormat: 'time',
        showTokenSpeed: true,
        compactMetrics: false,
      },
    },
  }),
}));

describe('TokenMetricsBadge', () => {
  it('shows cache hit percent and cached token count when cache is used', () => {
    const usage: TokenUsage = {
      promptTokens: 1200,
      completionTokens: 120,
      totalTokens: 1320,
      cachedPromptTokens: 900,
      details: {},
    };

    render(<TokenMetricsBadge usage={usage} />);

    expect(screen.getByTestId('cache-hit-indicator')).toHaveTextContent(
      '75% · 900',
    );
  });

  it('shows a cache activity label even before cached tokens are reported', () => {
    const usage: TokenUsage = {
      promptTokens: 1200,
      completionTokens: 120,
      totalTokens: 1320,
      details: {
        cachedContentTokenCount: 0,
      },
    };

    render(<TokenMetricsBadge usage={usage} />);

    expect(screen.getByTestId('cache-hit-indicator')).toHaveTextContent('cache');
  });

  it('prefers backend preflight estimate as the displayed input metric', () => {
    const usage: TokenUsage = {
      promptTokens: 1200,
      completionTokens: 120,
      totalTokens: 1320,
      details: {},
    };
    const preflight: PreflightTokenMetrics = {
      conservativePromptTokens: 2450,
      promptAnchoredTotalTokens: 2334,
      safeInputTokenLimit: 65536,
      measuredOutputTokensReserve: 1200,
      effectiveInputBudget: 64336,
      totalBudgetTokens: 3650,
      systemPromptTokens: 2048,
      toolsTokens: 1024,
      selectedMessageCount: 12,
      compactSummaryInjected: true,
      preservedCalibrationRatio: 0.92,
    };

    render(<TokenMetricsBadge usage={usage} preflight={preflight} />);

    expect(screen.getByText('2,450')).toBeInTheDocument();
    expect(screen.getByText('/65,536')).toBeInTheDocument();
    expect(screen.queryByText('1,200')).not.toBeInTheDocument();
  });

  it('hides decode speed when evalDuration is missing, zero, or non-finite', () => {
    const cases: Array<number | undefined> = [
      undefined,
      0,
      -50,
      Number.NaN,
      Number.POSITIVE_INFINITY,
    ];
    for (const evalDuration of cases) {
      const { unmount } = render(
        <TokenMetricsBadge
          usage={{
            promptTokens: 500,
            completionTokens: 96,
            totalTokens: 596,
            details: evalDuration !== undefined ? { evalDuration } : {},
          }}
        />,
      );
      expect(screen.queryByTitle('Tokens per second')).not.toBeInTheDocument();
      unmount();
    }
  });

  it('prioritizes provider promptEvalDuration over timeToFirstToken in prefill tooltip', () => {
    render(
      <TokenMetricsBadge
        usage={{
          promptTokens: 1000,
          completionTokens: 100,
          totalTokens: 1100,
          details: {
            evalDuration: 2000,
            promptEvalDuration: 250,
            timeToFirstToken: 900,
          },
        }}
      />,
    );

    expect(screen.getByTitle('Tokens per second')).toHaveTextContent('50.0');
    const promptEl = screen.getByText('1,000').closest('span');
    expect(promptEl?.getAttribute('title')).toContain('Prefill: 250ms');
  });
});
