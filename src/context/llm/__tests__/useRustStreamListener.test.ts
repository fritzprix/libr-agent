import { renderHook, waitFor } from '@testing-library/react';
import { listen } from '@tauri-apps/api/event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  AgentEventPayload,
  LlmStreamDeltaPayload,
} from '@/context/agent-session/types';
import { REASONING_BUDGET_CHARS_PER_TOKEN } from '@/lib/ai-service/openai/reasoning-budget';
import type { Settings } from '@/lib/services/settings-service';
import type { Message } from '@/models/chat';
import {
  createRustStreamHandler,
  useRustStreamListener,
} from '../useRustStreamListener';

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
  return {
    handler,
    current: () => streaming.get(SESSION),
    hasEntry: () => streaming.has(SESSION),
  };
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

  it('clears the live message when the cancelled turn is dropped', () => {
    const { handler, hasEntry } = setup();

    handler.handleDelta(delta({ content: 'partial' }));
    expect(hasEntry()).toBe(true);
    handler.handleCancel(SESSION, RESPONSE);

    expect(hasEntry()).toBe(false);
  });

  it('keeps a newer turn when an old turn is cancelled', () => {
    const { handler, current } = setup();

    handler.handleDelta(delta({ content: 'new' }, { responseMessageId: 'r-2' }));
    handler.handleCancel(SESSION, RESPONSE);

    expect(current()?.id).toBe('r-2');
  });

  it('restarts the live message when the backend resets the turn (#3089)', () => {
    const { handler, current, hasEntry } = setup();

    handler.handleDelta(delta({ content: 'discarded' }));
    expect(current()).toBeDefined();

    handler.handleReset(SESSION, RESPONSE);
    expect(hasEntry()).toBe(false);

    // The retry answers under the same id; its text replaces the discarded
    // attempt instead of continuing it.
    handler.handleDelta(delta({ content: 'retried' }));
    expect(textOf(current())).toBe('retried');
  });

  it('keeps a newer turn when an old turn is reset', () => {
    const { handler, current } = setup();

    handler.handleDelta(delta({ content: 'new' }, { responseMessageId: 'r-2' }));
    handler.handleReset(SESSION, RESPONSE);

    expect(current()?.id).toBe('r-2');
  });

  it('clears the live message when the final message is saved', () => {
    const { handler, hasEntry } = setup();

    handler.handleDelta(delta({ content: 'done' }));
    handler.handleDelta(delta({ finishReason: 'stop' }));
    expect(hasEntry()).toBe(true);
    handler.handleMessageAdded(SESSION, RESPONSE);

    expect(hasEntry()).toBe(false);
  });

  it('keeps the live message when another message is saved', () => {
    const { handler, hasEntry } = setup();

    handler.handleDelta(delta({ content: 'streaming' }));
    handler.handleMessageAdded(SESSION, 'some-other-message');

    expect(hasEntry()).toBe(true);
  });

  it('publishes a usage-only delta that arrives right after finishReason', () => {
    const { handler, current } = setup();

    handler.handleDelta(delta({ content: 'Hello' }, { provider: 'openai' }));
    handler.handleDelta(
      delta({ finishReason: 'stop' }, { provider: 'openai' }),
    );
    expect(current()?.usage).toBeUndefined();

    // Arrives immediately (inside the 50ms throttle window) as OpenAI's trailing choices: [] chunk.
    handler.handleDelta(
      delta(
        {
          usage: {
            promptTokens: 120,
            completionTokens: 30,
            totalTokens: 150,
          },
        },
        { provider: 'openai' },
      ),
    );

    expect(current()?.usage?.promptTokens).toBe(120);
    expect(current()?.usage?.completionTokens).toBe(30);
    expect(current()?.usage?.totalTokens).toBe(150);
    expect(current()?.usage?.details?.evalDuration).toBeTypeOf('number');
  });
});

describe('useRustStreamListener', () => {
  it('clears the live message on the saved assistant message and on cancel', async () => {
    const handlers = new Map<string, (event: { payload: unknown }) => void>();
    vi.mocked(listen).mockImplementation(async (eventName, handler) => {
      handlers.set(
        eventName,
        handler as (event: { payload: unknown }) => void,
      );
      return () => {};
    });

    let streaming = new Map<string, Partial<Message>>();
    const setStreamingMessages = (
      update: React.SetStateAction<Map<string, Partial<Message>>>,
    ) => {
      streaming = typeof update === 'function' ? update(streaming) : update;
    };
    const settingsRef = { current: { advanced: {} } as Settings };
    renderHook(() =>
      useRustStreamListener({ settingsRef, setStreamingMessages }),
    );
    await waitFor(() => expect(handlers.size).toBe(2));

    const emitAgentEvent = (payload: AgentEventPayload) =>
      handlers.get('agent:event')?.({ payload });

    emitAgentEvent(delta({ content: 'hi' }));
    expect(streaming.has(SESSION)).toBe(true);
    emitAgentEvent({
      type: 'messageAdded',
      sessionId: SESSION,
      message: {
        id: RESPONSE,
        sessionId: SESSION,
        role: 'assistant',
        content: [{ type: 'text', text: 'hi' }],
        createdAt: 1,
        updatedAt: 1,
      },
    });
    expect(streaming.has(SESSION)).toBe(false);

    emitAgentEvent(
      delta({ content: 'again' }, { responseMessageId: 'response-2' }),
    );
    expect(streaming.has(SESSION)).toBe(true);
    handlers.get('llm:completion-cancel')?.({
      payload: {
        sessionId: SESSION,
        responseMessageId: 'response-2',
        reason: 'user',
      },
    });
    expect(streaming.has(SESSION)).toBe(false);
  });
});
