import { describe, expect, it } from 'vitest';
import type React from 'react';
import type { Settings } from '@/context/SettingsContext';
import { DEFAULT_SETTING } from '@/context/SettingsContext';
import { StreamAccumulator } from '../stream-accumulator';
import { validateAndFinalizeMessage } from '../completion-validators';
import {
  calculateTokensPerSecond,
  formatUsageMetrics,
} from '@/lib/ai-service/utils/general';

const settingsRef = {
  current: DEFAULT_SETTING,
} as React.MutableRefObject<Settings>;

describe('Backend timing SSOT', () => {
  it('preserves backend evalDuration and TTFT across a burst of chunks without overwriting them', () => {
    const accumulator = new StreamAccumulator('sess-1', 'msg-1', settingsRef, 0);

    // Simulate a burst of chunks arriving within <1ms in the webview
    for (let i = 0; i < 20; i++) {
      accumulator.processChunk({ content: `tok${i} ` });
    }
    accumulator.processChunk({
      usage: {
        promptTokens: 500,
        completionTokens: 96,
        totalTokens: 596,
        details: {
          evalDuration: 2400, // 40.0 t/s from backend
          promptEvalDuration: 300,
          timeToFirstToken: 300,
        },
      },
    });

    const msg = validateAndFinalizeMessage({
      sessionId: 'sess-1',
      responseMessageId: 'msg-1',
      content: accumulator.content,
      streamingToolCalls: [],
      finalUsage: accumulator.finalUsage,
    });

    expect(msg.usage?.details?.evalDuration).toBe(2400);
    expect(msg.usage?.details?.promptEvalDuration).toBe(300);
    expect(msg.usage?.details?.timeToFirstToken).toBe(300);

    const formatted = formatUsageMetrics(msg.usage!);
    expect(formatted.speed).toBe('40.0 t/s');
  });

  it('does not synthesize frontend durations when backend timing is missing or zero', () => {
    const accumulator = new StreamAccumulator('sess-1', 'msg-2', settingsRef, 0);
    accumulator.processChunk({ content: 'hello' });
    accumulator.processChunk({
      usage: {
        promptTokens: 100,
        completionTokens: 50,
        totalTokens: 150,
      },
    });

    const msg = validateAndFinalizeMessage({
      sessionId: 'sess-1',
      responseMessageId: 'msg-2',
      content: accumulator.content,
      streamingToolCalls: [],
      finalUsage: accumulator.finalUsage,
    });

    expect(msg.usage?.details?.evalDuration).toBeUndefined();
    expect(msg.usage?.details?.promptEvalDuration).toBeUndefined();
    expect(msg.usage?.details?.timeToFirstToken).toBeUndefined();
    expect(formatUsageMetrics(msg.usage!).speed).toBeUndefined();

    // Zero or negative durations must never produce a speed
    expect(
      calculateTokensPerSecond(
        { promptTokens: 100, completionTokens: 50, totalTokens: 150 },
        0,
      ),
    ).toBe(0);
    expect(
      formatUsageMetrics({
        promptTokens: 100,
        completionTokens: 50,
        totalTokens: 150,
        details: { evalDuration: 0 },
      }).speed,
    ).toBeUndefined();
  });
});
