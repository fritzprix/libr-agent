import { describe, expect, it } from 'vitest';
import type { Message } from '@/models/chat';
import type { MCPTool } from '@/lib/mcp';
import { BaseAIService, stableStringify } from '../base-service';
import { buildCompactionInstruction } from '../base-service-context';
import { AIServiceError, AIServiceProvider } from '../types';

class TestBaseAIService extends BaseAIService<string, string> {
  private compactChunks: string[] = [''];
  private compactChunkBatches: string[][] = [];
  private compactCallCount = 0;

  getProvider(): AIServiceProvider {
    return AIServiceProvider.Empty;
  }

  convertTools(mcpTools: MCPTool[]): string[] {
    void mcpTools;
    return [];
  }

  sanitizeSingleMessage(message: Message): Message {
    return message;
  }

  supportsTools(modelName: string): boolean {
    void modelName;
    return false;
  }

  estimateContextWindow(modelName: string): number {
    void modelName;
    return 0;
  }

  protected convertMessages(
    messages: Message[],
    systemPrompt?: string,
  ): string[] {
    void messages;
    void systemPrompt;
    return [];
  }

  protected async *doStreamChat(
    messages?: Message[],
    options?: {
      modelName?: string;
      systemPrompt?: string;
      sessionContext?: string;
    },
  ): AsyncGenerator<string, void, void> {
    void messages;
    void options;
    this.compactCallCount += 1;
    const chunks =
      this.compactChunkBatches.shift() ?? this.compactChunks;
    for (const chunk of chunks) {
      yield chunk;
    }
  }

  dispose(): void {}

  public shouldRetryForTest(error: unknown): boolean {
    return this.shouldRetry(error);
  }

  public setCompactChunks(chunks: string[]): void {
    this.compactChunks = chunks;
  }

  public setCompactChunkBatches(batches: string[][]): void {
    this.compactChunkBatches = [...batches];
  }

  public getCompactCallCount(): number {
    return this.compactCallCount;
  }
}

describe('BaseAIService.shouldRetry', () => {
  const service = new TestBaseAIService('test-key');

  it('retries transient RESOURCE_EXHAUSTED 429 rate limits', () => {
    expect(
      service.shouldRetryForTest({
        status: 429,
        message: '429 RESOURCE_EXHAUSTED: Rate limit exceeded, please retry later',
      }),
    ).toBe(true);
  });

  it('does not retry spending cap 429 errors', () => {
    expect(
      service.shouldRetryForTest({
        status: 429,
        message:
          '429 RESOURCE_EXHAUSTED: spending cap reached for this project quota',
      }),
    ).toBe(false);
  });
});

describe('BaseAIService.sanitizeMessages', () => {
  const service = new TestBaseAIService('test-key');

  it('replaces malformed tool calls with repair guidance instead of resending them', () => {
    const sanitized = service.sanitizeMessages([
      {
        id: 'assistant-1',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'assistant',
        content: [],
        tool_calls: [
          {
            id: 'call-bad',
            type: 'function',
            function: {
              name: 'readFile',
              arguments: '{"path":"foo.txt"',
            },
          },
        ],
      },
    ]);

    expect(sanitized).toHaveLength(1);
    expect(sanitized[0].tool_calls).toBeUndefined();
    expect(sanitized[0].content).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          type: 'text',
          text: expect.stringContaining(
            'invalid tool call arguments were removed',
          ),
        }),
      ]),
    );
    expect(
      (sanitized[0].content[0] as { type: 'text'; text: string }).text,
    ).toContain('readFile');
  });

  it('keeps valid tool calls while repairing malformed siblings', () => {
    const sanitized = service.sanitizeMessages([
      {
        id: 'assistant-2',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'assistant',
        content: [{ type: 'text', text: 'Working...' }],
        tool_calls: [
          {
            id: 'call-good',
            type: 'function',
            function: {
              name: 'listFiles',
              arguments: '{"path":"src"}',
            },
          },
          {
            id: 'call-bad',
            type: 'function',
            function: {
              name: 'readFile',
              arguments: '{path:"broken"}',
            },
          },
        ],
      },
      {
        id: 'tool-good',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'tool',
        tool_call_id: 'call-good',
        content: [{ type: 'text', text: 'ok' }],
      },
    ]);

    expect(sanitized).toHaveLength(2);
    expect(sanitized[0].tool_calls).toHaveLength(1);
    expect(sanitized[0].tool_calls?.[0].id).toBe('call-good');
    expect(
      sanitized[0].content.some(
        (part) =>
          part.type === 'text' &&
          part.text.includes('Treat each omitted tool call as a failed attempt'),
      ),
    ).toBe(true);
    expect(sanitized[1].tool_call_id).toBe('call-good');
  });
});

describe('stableStringify', () => {
  it('returns a string for undefined values', () => {
    expect(stableStringify(undefined)).toBe('"undefined"');
  });

  it('returns a stable string for bigint values', () => {
    expect(stableStringify(42n)).toBe('{"$bigint":"42"}');
  });
});

describe('BaseAIService.compact', () => {
  const createMessages = (): Message[] => [
    {
      id: 'user-1',
      sessionId: 'session-1',
      threadId: 'session-1',
      role: 'user',
      content: [{ type: 'text', text: 'Summarize this conversation.' }],
    },
  ];

  it('combines JSON and raw stream chunks into a trimmed summary', async () => {
    const service = new TestBaseAIService('test-key');
    service.setCompactChunks([
      JSON.stringify({ content: 'Hello ' }),
      'world',
      JSON.stringify({ content: '  ' }),
    ]);

    await expect(service.compact(createMessages())).resolves.toBe('Hello world');
  });

  it('throws when compaction returns only empty output', async () => {
    const service = new TestBaseAIService('test-key');
    service.setCompactChunks(['', JSON.stringify({ content: '   ' })]);

    await expect(service.compact(createMessages())).rejects.toBeInstanceOf(
      AIServiceError,
    );
  });

  it('performs one bounded recursive squeeze when the first summary exceeds target budget', async () => {
    const service = new TestBaseAIService('test-key');
    service.setCompactChunkBatches([
      [JSON.stringify({ content: 'x'.repeat(2400) })],
      [JSON.stringify({ content: 'tight summary' })],
    ]);

    await expect(
      service.compact(createMessages(), {
        targetMaxTokens: 200,
        hardMaxTokens: 800,
        maxRecursivePasses: 1,
      }),
    ).resolves.toBe('tight summary');
    expect(service.getCompactCallCount()).toBe(2);
  });

  it('fails when bounded recursive compaction still exceeds the hard cap', async () => {
    const service = new TestBaseAIService('test-key');
    service.setCompactChunkBatches([
      [JSON.stringify({ content: 'x'.repeat(2400) })],
      [JSON.stringify({ content: 'y'.repeat(2400) })],
    ]);

    await expect(
      service.compact(createMessages(), {
        targetMaxTokens: 200,
        hardMaxTokens: 300,
        maxRecursivePasses: 1,
      }),
    ).rejects.toThrow(/hard cap/);
    expect(service.getCompactCallCount()).toBe(2);
  });
});

describe('buildCompactionInstruction', () => {
  it('includes bounded recursive compaction guidance for prior summaries', () => {
    const instruction = buildCompactionInstruction(
      [
        {
          id: 'compact-summary-session-1',
          sessionId: 'session-1',
          threadId: 'session-1',
          role: 'user',
          content: [{ type: 'text', text: 'Previous compact summary' }],
        },
      ],
      {
        targetMaxTokens: 200,
        hardMaxTokens: 300,
        recursivePass: true,
      },
    );

    expect(instruction).toContain('Aim for <= 200 tokens');
    expect(instruction).toContain('Hard max: 300 tokens');
    expect(instruction).toContain('Preserve the facts, not the wording');
    expect(instruction).toContain(
      'This is a bounded recursive compaction pass',
    );
  });
});
