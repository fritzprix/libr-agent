/**
 * Regression tests for the TTFT gauge-flicker bug.
 *
 * Bug: OpenAI and Groq services yielded a fake usage chunk with
 * `promptTokens: 0, completionTokens: 0, totalTokens: 0` as the very first
 * streaming chunk in order to carry the TTFT (Time To First Token) metric.
 * This caused the context-window gauge in the UI to momentarily drop to 0%
 * before recovering when the real usage arrived at the end of the stream.
 *
 * Fix: The TTFT chunk now only carries `{ usage: { details: { timeToFirstToken } } }`.
 * Token count fields are omitted entirely so they remain undefined (falsy) — as a
 * result the UI keeps its previous gauge value rather than resetting to 0.
 *
 * These tests ensure the fix is preserved and cannot regress silently.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { Message } from '@/models/chat';

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/** Minimal valid user message fixture */
const makeMessage = (id: string, text: string): Message => ({
  id,
  sessionId: 'test-session',
  threadId: 'test-session',
  role: 'user',
  content: [{ type: 'text', text }],
  createdAt: new Date(),
});

/** Collect all JSON-parsed chunks yielded by a streamChat call */
async function collectChunks(
  generator: AsyncGenerator<string>,
): Promise<Array<Record<string, unknown>>> {
  const chunks: Array<Record<string, unknown>> = [];
  for await (const raw of generator) {
    try {
      chunks.push(JSON.parse(raw) as Record<string, unknown>);
    } catch {
      // non-JSON chunk – ignore for these tests
    }
  }
  return chunks;
}

/** Returns every usage chunk (chunks that carry a `usage` field) */
function usageChunks(
  chunks: Array<Record<string, unknown>>,
): Array<Record<string, unknown>> {
  return chunks.filter(
    (c): c is Record<string, unknown> => typeof c.usage === 'object',
  );
}

// ---------------------------------------------------------------------------
// Mock: OpenAI SDK
// ---------------------------------------------------------------------------

vi.mock('openai', () => ({
  default: vi.fn(),
}));

vi.mock('../../logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

vi.mock('../../retry-utils', () => ({
  withRetry: vi.fn().mockImplementation((fn: () => unknown) => fn()),
  withTimeout: vi
    .fn()
    .mockImplementation((p: Promise<unknown>) => p),
}));

vi.mock('../../llm-config-manager', () => ({
  llmConfigManager: {
    getModelsForProvider: vi.fn().mockReturnValue({}),
    getModel: vi.fn().mockReturnValue(null),
  },
  ModelInfo: {},
}));

vi.mock('../message-normalizer', () => ({
  MessageNormalizer: {
    sanitizeMessagesForProvider: vi.fn().mockImplementation((msgs: Message[]) => msgs),
  },
}));

vi.mock('../model-capabilities', () => ({
  supportsThinking: vi.fn().mockResolvedValue(false),
  getContextWindow: vi.fn().mockResolvedValue(128000),
}));

// ---------------------------------------------------------------------------
// Helper: build a minimal fake OpenAI-style stream
// ---------------------------------------------------------------------------

/**
 * Builds an async iterable that mimics an OpenAI streaming response.
 * Emits a single text delta followed by a final usage chunk.
 */
async function* fakeOpenAIStream(usage: {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
}) {
  // First chunk: text delta only (no usage)
  yield {
    choices: [{ delta: { content: 'Hello' } }],
    usage: null,
  };
  // Last chunk: usage only (OpenAI's stream_options: include_usage pattern)
  yield {
    choices: [{ delta: {} }],
    usage,
  };
}

// ---------------------------------------------------------------------------
// Helper: build a minimal fake Groq-style stream
// ---------------------------------------------------------------------------

async function* fakeGroqStream() {
  yield { choices: [{ delta: { content: 'Hi' } }] };
  // Groq streams don't carry usage in chunks with the standard SDK;
  // the service relies on TTFT + stream end without per-chunk usage.
}

// ---------------------------------------------------------------------------
// Tests: OpenAIService
// ---------------------------------------------------------------------------

describe('OpenAIService – TTFT gauge-flicker regression', () => {
  let service: InstanceType<typeof import('../openai').OpenAIService>;

  beforeEach(async () => {
    vi.clearAllMocks();

    const OpenAI = (await import('openai')).default as unknown as ReturnType<typeof vi.fn>;
    OpenAI.mockImplementation(() => ({
      chat: {
        completions: {
          create: vi.fn().mockResolvedValue(
            fakeOpenAIStream({
              prompt_tokens: 42,
              completion_tokens: 10,
              total_tokens: 52,
            }),
          ),
        },
      },
    }));

    const { OpenAIService } = await import('../openai');
    service = new OpenAIService('sk-test-key', { maxTokens: 100 });
  });

  it('TTFT chunk carries only details — no token count fields (service layer)', async () => {
    const messages = [makeMessage('m1', 'Hello')];
    const chunks = await collectChunks(
      service.streamChat(messages, { modelName: 'gpt-4o' }),
    );

    const firstUsage = usageChunks(chunks)[0];
    expect(firstUsage).toBeDefined();

    // The TTFT chunk must carry timeToFirstToken in details.
    const details = (firstUsage.usage as Record<string, unknown>)
      ?.details as Record<string, unknown> | undefined;
    expect(details?.timeToFirstToken).toBeTypeOf('number');

    // The service intentionally omits token count fields from the TTFT chunk
    // so that useLLMExecution.ts can detect a "details-only" first chunk and
    // normalise them to 0 via `?? 0` before passing to the badge component.
    // If these fields are present with value 0 we regress back to the flicker bug.
    const usage = firstUsage.usage as Record<string, unknown>;
    expect('promptTokens' in usage).toBe(false);
    expect('completionTokens' in usage).toBe(false);
    expect('totalTokens' in usage).toBe(false);
  });

  it('real usage chunk arrives with non-zero token counts', async () => {
    const messages = [makeMessage('m1', 'Hello')];
    const chunks = await collectChunks(
      service.streamChat(messages, { modelName: 'gpt-4o' }),
    );

    const allUsage = usageChunks(chunks);
    const realUsage = allUsage.find((c) => {
      const u = c.usage as Record<string, unknown>;
      const pt = u.promptTokens as number | undefined;
      const tt = u.totalTokens as number | undefined;
      return (pt ?? 0) > 0 || (tt ?? 0) > 0;
    });

    expect(realUsage).toBeDefined();
    const u = realUsage!.usage as Record<string, unknown>;
    expect(u.promptTokens).toBe(42);
    expect(u.completionTokens).toBe(10);
    expect(u.totalTokens).toBe(52);
  });
});

// ---------------------------------------------------------------------------
// Tests: GroqService
// ---------------------------------------------------------------------------

describe('GroqService – TTFT gauge-flicker regression', () => {
  let service: InstanceType<typeof import('../groq').GroqService>;

  beforeEach(async () => {
    vi.clearAllMocks();

    vi.mock('groq-sdk', () => ({
      default: vi.fn().mockImplementation(() => ({
        chat: {
          completions: {
            create: vi.fn().mockResolvedValue(fakeGroqStream()),
          },
        },
      })),
    }));

    const { GroqService } = await import('../groq');
    service = new GroqService('gsk-test-key', { maxTokens: 100 });
  });

  it('TTFT chunk carries only details — no token count fields (service layer)', async () => {
    const messages = [makeMessage('m1', 'Hi')];
    const chunks = await collectChunks(
      service.streamChat(messages, { modelName: 'llama-3.1-8b-instant' }),
    );

    const firstUsage = usageChunks(chunks)[0];
    // Groq may not always yield a TTFT chunk if the stream is empty.
    if (!firstUsage) return;

    // The TTFT chunk must NOT include token count fields at the service layer.
    // The useLLMExecution hook normalises them to 0 via `?? 0` before the badge.
    const usage = firstUsage.usage as Record<string, unknown>;
    expect('promptTokens' in usage).toBe(false);
    expect('completionTokens' in usage).toBe(false);
    expect('totalTokens' in usage).toBe(false);
  });

  it('TTFT chunk must carry timeToFirstToken in details', async () => {
    const messages = [makeMessage('m1', 'Hi')];
    const chunks = await collectChunks(
      service.streamChat(messages, { modelName: 'llama-3.1-8b-instant' }),
    );

    const usageWithTTFT = usageChunks(chunks).find((c) => {
      const details = (c.usage as Record<string, unknown>)
        ?.details as Record<string, unknown> | undefined;
      return typeof details?.timeToFirstToken === 'number';
    });

    expect(usageWithTTFT).toBeDefined();
  });
});
