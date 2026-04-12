import '@testing-library/jest-dom';
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { TokenUsage } from '@/lib/ai-service/types';

import { TokenMetricsBadge } from '../TokenMetricsBadge';

vi.mock('@/context/SettingsContext', () => ({
  useSettings: () => ({
    value: {
      display: {
        metricDisplayMode: 'inline',
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

  it('labels the gauge as compaction pressure when Rust SSOT pressure is provided', () => {
    const usage: TokenUsage = {
      promptTokens: 1200,
      completionTokens: 120,
      totalTokens: 1320,
      details: {},
    };

    render(
      <TokenMetricsBadge
        usage={usage}
        compactionPressure={{
          totalTokens: 25822,
          contextWindow: 49152,
          modelMaxContext: 65536,
        }}
      />,
    );

    expect(screen.getByText('Compaction Pressure')).toBeInTheDocument();
  });
});
