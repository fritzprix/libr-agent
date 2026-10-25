import { describe, expect, it } from 'vitest';
import type { Message } from '@/models/chat';
import { AIServiceProvider } from './types';
import {
  compactMessages,
  getCompactionPartialOutput,
} from './base-service-compaction';

const messages = [] as Message[];

function contextFor(stream: () => AsyncGenerator<string, void, void>) {
  return {
    streamChat: stream,
    isAborted: () => false,
    getProvider: () => AIServiceProvider.OpenAI,
  };
}

describe('compactMessages partial output', () => {
  it('attaches text received before a stream error', async () => {
    const failure = new Error('stream dropped');
    const context = contextFor(async function* () {
      yield JSON.stringify({ content: 'first half ' });
      yield JSON.stringify({ content: 'of the summary' });
      throw failure;
    });

    await expect(compactMessages(messages, context)).rejects.toBe(failure);
    expect(getCompactionPartialOutput(failure)).toBe(
      'first half of the summary',
    );
  });

  it('has no partial output when nothing was received', async () => {
    const failure = new Error('connect failed');
    const context = contextFor(async function* () {
      yield* [] as string[];
      throw failure;
    });

    await expect(compactMessages(messages, context)).rejects.toBe(failure);
    expect(getCompactionPartialOutput(failure)).toBeUndefined();
  });
});
