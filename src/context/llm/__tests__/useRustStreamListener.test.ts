import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { LlmStreamDeltaPayload } from '@/context/agent-session/types';
import { REASONING_BUDGET_CHARS_PER_TOKEN } from '@/lib/ai-service/openai/reasoning-budget';
import type { Settings } from '@/lib/services/settings-service';
import type { Message } from '@/models/chat';
import { createRustStreamHandler } from '../useRustStreamListener';

const reportLLMStreamingIssue = vi.fn().mockResolvedValue(undefined);

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock('@/lib/backend/agent-commands', () => ({
  reportLLMStreamingIssue: (...args: unknown[]) =>
    reportLLMStreamingIssue(...args),
}));

const SESSION = 'session-1';
const RESPONSE = 'response-1';

function setup() {
  let streaming = new Map<string, Partial<Message>>();
  const settingsRef = { current: { advanced: {} } as Settings };
  const handler = createRustStreamHandler(settingsRef, (update) => {
    streaming = typeof update === 'function' ? update(streaming) : update;
  });
  return { handler, current: () => streaming.get(SESSION) };
}

function delta(
  piece: LlmStreamDeltaPayload['delta'],
  overrides: Partial<LlmStreamDeltaPayload> = {},
): LlmStreamDeltaPayload {
  return {
    type: 'llmStreamDelta',
    sessionId: SESSION,
    responseMessageId: RESPONSE,
    provider: 'anthropic',
    maxTokens: null,
    delta: piece,
    ...overrides,
  };
}

function textOf(message: Partial<Message> | undefined): string {
  return (message?.content ?? [])
    .flatMap((item) => (item.type === 'text' ? [item.text] : []))
    .join('');
}

describe('Rust stream handler', () => {
  beforeEach(() => {
    reportLLMStreamingIssue.mockClear();
  });

  it('paints thinking, text and tool calls into one live message', () => {
    const { handler, current } = setup();

    handler.handleDelta(delta({ thinking: 'Plan it. ' }));
    handler.handleDelta(delta({ thinking: 'Then act.' }));
    handler.handleDelta(delta({ content: 'Hello, ' }));
    handler.handleDelta(
      delta({
        tool_calls: [
          {
            index: 0,
            id: 'call_1',
            function: { name: 'planning__create', arguments: '' },
          },
        ],
      }),
    );
    handler.handleDelta(
      delta({
        tool_calls: [{ index: 0, function: { arguments: '{"a":1}' } }],
      }),
    );
    handler.handleDelta(delta({ finishReason: 'tool_calls' }));

    const message = current();
    expect(message?.id).toBe(RESPONSE);
    expect(message?.isStreaming).toBe(true);
    expect(message?.thinking).toBe('Plan it. Then act.');
    expect(textOf(message)).toBe('Hello, ');
    expect(message?.tool_calls).toEqual([
      {
        id: 'call_1',
        type: 'function',
        function: { name: 'planning__create', arguments: '{"a":1}' },
      },
    ]);
    expect(reportLLMStreamingIssue).not.toHaveBeenCalled();
  });

  it('starts a fresh message when a new response begins', () => {
    const { handler, current } = setup();

    handler.handleDelta(delta({ content: 'first' }));
    handler.handleDelta(delta({ finishReason: 'stop' }));
    handler.handleDelta(
      delta({ content: 'second' }, { responseMessageId: 'response-2' }),
    );
    handler.handleDelta(
      delta({ finishReason: 'stop' }, { responseMessageId: 'response-2' }),
    );

    expect(current()?.id).toBe('response-2');
    expect(textOf(current())).toBe('second');
  });

  it('reports a repeated text loop from a scripted stream', () => {
    const { handler } = setup();
    const sentence =
      'The quick brown fox jumps over the lazy dog while the cat watches closely. ';

    for (let i = 0; i < 20; i += 1) {
      handler.handleDelta(delta({ content: sentence }));
    }

    expect(reportLLMStreamingIssue).toHaveBeenCalledTimes(1);
    expect(reportLLMStreamingIssue).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionId: SESSION,
        responseMessageId: RESPONSE,
        issueKind: 'REPEATED_TEXT_LOOP',
      }),
    );
  });

  it('reports the reasoning budget with the request cap and stops painting', () => {
    const { handler, current } = setup();
    const maxTokens = 40;
    const overBudget = 'x'.repeat(
      Math.floor(maxTokens * 0.9) * REASONING_BUDGET_CHARS_PER_TOKEN,
    );

    handler.handleDelta(
      delta({ content: 'ok' }, { provider: 'openai', maxTokens }),
    );
    const before = current();
    handler.handleDelta(
      delta({ thinking: overBudget }, { provider: 'openai', maxTokens }),
    );
    handler.handleDelta(
      delta({ thinking: 'late' }, { provider: 'openai', maxTokens }),
    );

    expect(reportLLMStreamingIssue).toHaveBeenCalledTimes(1);
    expect(reportLLMStreamingIssue).toHaveBeenCalledWith(
      expect.objectContaining({
        issueKind: 'REASONING_BUDGET_EXCEEDED',
        maxOutputTokens: maxTokens,
        maxOutputTokensSource: 'assistant',
      }),
    );
    expect(current()).toBe(before);
  });

  it('ignores a turn the backend cancelled', () => {
    const { handler, current } = setup();

    handler.handleDelta(delta({ content: 'partial' }));
    handler.handleCancel(SESSION, RESPONSE);
    const before = current();
    handler.handleDelta(delta({ content: ' late text' }));
    handler.handleDelta(delta({ finishReason: 'stop' }));

    expect(current()).toBe(before);
  });
});
